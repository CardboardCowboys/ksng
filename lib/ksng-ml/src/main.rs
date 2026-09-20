use {
  crate::{models::ModelManager, tasks::TaskManager},
  futures_util::FutureExt,
  interprocess::local_socket::{
    GenericFilePath, GenericNamespaced,
    tokio::{Stream, prelude::*},
  },
  ksng_ml_ipc::packet::host_packet,
  std::task::Waker,
  tokio::io::BufReader,
  uuid::Uuid,
};

mod audio;
mod libs;
mod models;
mod tasks;

#[tokio::main]
async fn main() {
  colog::init();
  libs::ort::init_ort().unwrap();

  let args: Vec<String> = std::env::args().collect();
  if args.len() < 2 {
    panic!("Must provide name of local socket in second argument.");
  }

  run(&args[1]).await.unwrap()
}

async fn run(name: &str) -> Result<(), anyhow::Error> {
  let name = if GenericNamespaced::is_supported() {
    name.to_ns_name::<GenericNamespaced>().unwrap()
  } else {
    name.to_fs_name::<GenericFilePath>().unwrap()
  };

  let mut tasks = TaskManager::new();
  let models = ModelManager::new().unwrap();

  let name_s = format!("{name:?}");
  let conn = Stream::connect(name).await.unwrap();
  let (mut recv, mut send) = conn.split();
  let mut recv_reader = BufReader::new(&mut recv);

  log::info!("worker listening on socket {name_s}");

  let mut recv_future = Box::pin(ksng_ml_ipc::read_next_packet::<
    ksng_ml_ipc::packet::HostPacket,
  >(&mut recv_reader));
  let mut ctx = std::task::Context::from_waker(Waker::noop());

  loop {
    match recv_future.poll_unpin(&mut ctx) {
      std::task::Poll::Ready(Ok(packet)) => {
        log::info!("received packet: {packet:?}");
        match packet.contents.unwrap() {
          host_packet::Contents::DlModelsList(_) => {
            models.list_dl_models(&mut send).await?;
          }
          host_packet::Contents::ModelsList(_) => {
            models.list_models(&mut send).await?;
          }
          host_packet::Contents::DlListedModel(dl_model) => {
            let job_id: Uuid = dl_model.job_id.unwrap().into();
            let model_id: Uuid = dl_model.model_id.unwrap().into();
            models.download_model_id(model_id, job_id).await?;
          }
          host_packet::Contents::Kill(_) => {
            log::info!("received kill");
            break;
          }
          host_packet::Contents::StartTask(task) => {
            tasks.start_task(&models, task).await?;
          }
          host_packet::Contents::CancelTask(cancel) => {
            if let Some(id) = cancel.id {
              tasks.cancel_task(id.into()).await?;
            }
          }
        }
        drop(recv_future);
        recv_future = Box::pin(ksng_ml_ipc::read_next_packet::<
          ksng_ml_ipc::packet::HostPacket,
        >(&mut recv_reader));
      }
      std::task::Poll::Ready(Err(err)) => {
        log::error!("Failed to read host packet from IPC: {err:?}");
        break;
      }
      std::task::Poll::Pending => {}
    }

    models.poll_jobs(&mut send).await?;
    tasks.poll_tasks(&mut send).await?;
  }

  Ok(())
}
