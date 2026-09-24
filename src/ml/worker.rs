use std::{
  cell::RefCell,
  path::PathBuf,
  process::{Child, Command},
  sync::{
    Arc, Barrier, RwLock,
    atomic::{AtomicBool, Ordering},
  },
  task::Waker,
  thread,
};

use futures_util::FutureExt;
use interprocess::local_socket::{
  GenericFilePath, GenericNamespaced, ListenerOptions, NameType, ToFsName, ToNsName,
  traits::tokio::{Listener, Stream},
};
use ksng_ml_ipc::packet::{
  HostGetAvailableModelsList, HostGetDownloadableModelsList, HostPacket, host_packet::Contents,
  worker_packet,
};
use tokio::runtime::Runtime;

use crate::{
  ml::{models::ModelManager, tasks::TaskManager},
  util::{error::UiError, logger::Logger},
};

type Models = Arc<RwLock<ModelManager>>;
type Tasks = Arc<RwLock<TaskManager>>;
type SendQueue = Arc<deadqueue::unlimited::Queue<HostPacket>>;

pub struct WorkerManager {
  instance: RefCell<Option<WorkerInstance>>,
  logger: Logger,
  send_queue: SendQueue,
  pub models: Models,
  pub tasks: Tasks,
}

impl WorkerManager {
  pub fn new(logger: Logger) -> WorkerManager {
    let send_queue = Arc::new(deadqueue::unlimited::Queue::new());
    WorkerManager {
      instance: RefCell::new(None),
      models: Arc::new(RwLock::new(ModelManager::new(send_queue.clone()))),
      tasks: Arc::new(RwLock::new(TaskManager::new(send_queue.clone()))),
      send_queue,
      logger,
    }
  }

  pub fn start(&self) -> Result<(), UiError> {
    if let Some(inst) = &mut *self.instance.borrow_mut() {
      if inst.is_running() {
        return Ok(());
      }

      inst.stop();
    }

    let inst = WorkerInstance::start(
      self.logger.clone(),
      self.models.clone(),
      self.tasks.clone(),
      self.send_queue.clone(),
    )?;
    self.instance.borrow_mut().replace(inst);
    Ok(())
  }

  pub fn is_running(&self) -> bool {
    if let Some(inst) = &mut *self.instance.borrow_mut() {
      return inst.is_running();
    }

    false
  }

  pub fn is_installed() -> Result<bool, UiError> {
    WorkerInstance::is_installed()
  }

  pub fn send(&self, packet: HostPacket) {
    if let Some(inst) = self.instance.borrow().as_ref() {
      inst.send(packet);
    }
  }
}

struct WorkerInstance {
  process: Child,
  send_queue: SendQueue,
  closed: Arc<AtomicBool>,
  has_closed: Arc<Barrier>,
}

#[derive(Clone)]
struct ListenerParams {
  logger: Logger,
  send_queue: SendQueue,
  models: Models,
  tasks: Tasks,
  closed: Arc<AtomicBool>,
  has_closed: Arc<Barrier>,
}

impl WorkerInstance {
  pub fn start(
    logger: Logger,
    models: Models,
    tasks: Tasks,
    send_queue: SendQueue,
  ) -> Result<WorkerInstance, UiError> {
    let Some(worker_exe) = Self::find_worker_exe()? else {
      return Err(UiError::Worker(
        "Can't find path of ksng executable".to_string(),
      ));
    };

    let socket_name = "ksng.sock";

    let ns_name = if GenericNamespaced::is_supported() {
      socket_name.to_ns_name::<GenericNamespaced>()?
    } else {
      socket_name.to_fs_name::<GenericFilePath>()?
    };

    // Use a barrier to make sure that the listener is created before the worker
    // spawns
    let barrier = Arc::new(Barrier::new(2));
    let barrier_clone = barrier.clone();

    let closed = Arc::new(AtomicBool::new(false));
    let has_closed = Arc::new(Barrier::new(2));

    let params = ListenerParams {
      send_queue: send_queue.clone(),
      logger: logger.clone(),
      models,
      tasks,
      closed: closed.clone(),
      has_closed: has_closed.clone(),
    };

    thread::spawn(move || {
      let rt = logger.wrap(Runtime::new());
      if let Some(rt) = rt {
        rt.block_on(async move {
          let name_s = format!("{ns_name:?}");
          let socket = logger.wrap(ListenerOptions::new().name(ns_name).create_tokio());
          if let Some(socket) = socket {
            log::info!("host listening on socket {name_s}");
            let logger_clone = logger.clone();
            barrier_clone.wait();
            logger_clone.wrap(Self::listen_thread(socket, params).await);
          }
        });
      }
    });

    barrier.wait();
    log::info!("running {worker_exe:?} with arg {socket_name:?}");
    let process = Command::new(worker_exe).arg(socket_name).spawn()?;

    Ok(WorkerInstance {
      process,
      send_queue,
      closed,
      has_closed,
    })
  }

  pub fn is_running(&mut self) -> bool {
    match self.process.try_wait() {
      Ok(Some(_)) => false,
      Ok(None) => true,
      Err(_) => false,
    }
  }

  pub fn stop(&mut self) {
    self.closed.store(true, Ordering::Relaxed);
    self.has_closed.wait();
  }

  pub fn is_installed() -> Result<bool, UiError> {
    Ok(Self::find_worker_exe()?.is_some())
  }

  pub fn send(&self, packet: HostPacket) {
    self.send_queue.push(packet);
  }

  async fn listen_thread(
    socket: interprocess::local_socket::tokio::Listener,
    params: ListenerParams,
  ) -> Result<(), UiError> {
    let has_closed = params.has_closed.clone();
    loop {
      let closed = params.closed.clone();
      let conn = socket.accept().await?;
      log::info!("accepted connection from worker");
      params
        .logger
        .wrap(Self::listen_client(conn, params.clone()).await);
      if closed.load(Ordering::Relaxed) {
        break;
      }
    }

    has_closed.wait();

    Ok(())
  }

  async fn listen_client(
    conn: interprocess::local_socket::tokio::Stream,
    params: ListenerParams,
  ) -> Result<(), UiError> {
    let (mut recv, mut send) = conn.split();
    let mut recv_reader = tokio::io::BufReader::new(&mut recv);
    let mut recv_future = Box::pin(ksng_ml_ipc::read_next_packet::<
      ksng_ml_ipc::packet::WorkerPacket,
    >(&mut recv_reader));
    let mut ctx = std::task::Context::from_waker(Waker::noop());

    ksng_ml_ipc::write_packet(
      HostPacket {
        contents: Some(Contents::DlModelsList(HostGetDownloadableModelsList {})),
      },
      &mut send,
    )
    .await?;

    ksng_ml_ipc::write_packet(
      HostPacket {
        contents: Some(Contents::ModelsList(HostGetAvailableModelsList {})),
      },
      &mut send,
    )
    .await?;

    let models = params.models.as_ref();
    let tasks = params.tasks.as_ref();

    loop {
      if params.closed.load(Ordering::Relaxed) {
        break;
      }
      match recv_future.poll_unpin(&mut ctx) {
        std::task::Poll::Ready(Ok(packet)) => {
          log::debug!("received packet: {packet:?}");
          match packet.contents.unwrap() {
            worker_packet::Contents::DlModelsListResponse(response) => {
              models.write().unwrap().recv_dl_models_list(response);
            }
            worker_packet::Contents::JobStatusResponse(response) => {
              params
                .logger
                .wrap(models.write().unwrap().recv_job_status(response));
            }
            worker_packet::Contents::ModelsListResponse(response) => {
              models.write().unwrap().recv_models_list(response);
            }
            worker_packet::Contents::TaskResult(result) => {
              params
                .logger
                .wrap(tasks.write().unwrap().recv_task_result(result));
            }
            worker_packet::Contents::TaskInfo(info) => {
              params
                .logger
                .wrap(tasks.write().unwrap().recv_task_info(info));
            }
          }
          drop(recv_future);
          recv_future = Box::pin(ksng_ml_ipc::read_next_packet::<
            ksng_ml_ipc::packet::WorkerPacket,
          >(&mut recv_reader));
        }
        std::task::Poll::Ready(Err(err)) => {
          params.logger.log(
            crate::util::logger::LogType::Error,
            format!("Failed to read host packet from IPC: {err:?}"),
          );
        }
        std::task::Poll::Pending => {}
      }

      while let Some(packet) = params.send_queue.try_pop() {
        ksng_ml_ipc::write_packet(packet, &mut send).await?;
      }
    }

    Ok(())
  }

  fn find_worker_exe() -> Result<Option<PathBuf>, UiError> {
    let name = Self::exe_name();
    let this_exe = std::env::current_exe()?;
    let this_dir = this_exe.parent();
    if let Some(this_dir) = this_dir
      && std::fs::exists(this_dir.join(name))?
    {
      return Ok(Some(this_dir.join(name)));
    }

    if let Some(dirs) = directories::ProjectDirs::from("com", "Cardboard Cowboys", "ksng-ml") {
      let file = dirs.data_dir().join(name);
      if std::fs::exists(&file)? {
        return Ok(Some(file));
      }
    }

    Ok(None)
  }

  fn exe_name() -> &'static str {
    #[cfg(target_os = "windows")]
    {
      "ksng-ml.exe"
    }
    #[cfg(not(target_os = "windows"))]
    {
      "ksng-ml"
    }
  }
}
