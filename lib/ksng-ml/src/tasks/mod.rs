use std::{collections::HashSet, path::PathBuf, sync::Arc};

use anyhow::Error;
use ksng_ml_ipc::packet::{
  self, HostStartTask, WorkerPacket, WorkerTaskCancelled, WorkerTaskComplete, WorkerTaskFailed,
  WorkerTaskResult, WorkerTaskRunning, worker_task_info, worker_task_result,
};
use spectrasonic::encoders::{AudioCodec, AudioEncoderOptions};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
  models::ModelManager,
  tasks::htdemucs::{HtdemucsParameters, HtdemucsSeparator},
};

pub mod htdemucs;

fn packet_codec_to_codec(packet: ksng_ml_ipc::packet::AudioCodec) -> AudioCodec {
  match packet {
    packet::AudioCodec::Mp3 => AudioCodec::Mp3,
    packet::AudioCodec::Aac => AudioCodec::Aac,
    packet::AudioCodec::Wav => AudioCodec::Wav,
    packet::AudioCodec::Flac => AudioCodec::Flac,
  }
}

fn packet_options_to_options(packet: ksng_ml_ipc::packet::AudioOptions) -> AudioEncoderOptions {
  AudioEncoderOptions {
    bit_rate: packet.bit_rate as usize,
    options: packet.options_str,
  }
}

pub trait TaskImpl {
  async fn process(
    &self,
    monitor: Arc<TaskMonitor>,
  ) -> Result<Option<worker_task_result::Result>, anyhow::Error>;
}

pub enum TaskStatus {
  Running,
  Completed,
  Failed(anyhow::Error),
  Cancelled,
}

pub struct TaskMonitor {
  cancelled: RwLock<bool>,
  progress: RwLock<f32>,
  status: RwLock<TaskStatus>,
}

pub struct Task {
  id: Uuid,
  _title: String,
  monitor: Arc<TaskMonitor>,
}

pub struct TaskResult {
  id: Uuid,
  result: worker_task_result::Result,
}

pub struct TaskManager {
  tasks: Vec<Task>,
  pending_task_results: Arc<deadqueue::unlimited::Queue<TaskResult>>,
}

impl TaskManager {
  pub fn new() -> TaskManager {
    TaskManager {
      tasks: Vec::new(),
      pending_task_results: Default::default(),
    }
  }

  pub async fn start_task(
    &mut self,
    models: &ModelManager,
    task: HostStartTask,
  ) -> Result<(), Error> {
    let Some(task_id) = task.id else {
      return Err(Error::msg("No task ID provided"));
    };

    let task_id: Uuid = task_id.into();

    let Some(model_id) = task.model_id else {
      return Err(Error::msg("No model ID provided"));
    };

    let model_id: Uuid = model_id.into();

    let Some((path, params)) = models.find_model_path_params(model_id).await else {
      return Err(Error::msg(format!(
        "Could not find model for ID {:?}",
        model_id
      )));
    };

    let Some(task) = task.task else {
      return Err(Error::msg("No task provided"));
    };

    let (title, task) = match task {
      packet::host_start_task::Task::Htdemucs(htdemucs) => {
        let params = serde_json::from_value::<HtdemucsParameters>(params)?;
        let codec = packet_codec_to_codec(htdemucs.codec());
        (
          "Htdemucs stem separation",
          HtdemucsSeparator {
            model_path: path,
            input_path: PathBuf::from(htdemucs.input_path),
            parameters: params,
            output_path_drums: PathBuf::from(htdemucs.output_path_drums),
            output_path_bass: PathBuf::from(htdemucs.output_path_bass),
            output_path_other: PathBuf::from(htdemucs.output_path_other),
            output_path_vocals: PathBuf::from(htdemucs.output_path_vocals),
            codec,
            audio_options: packet_options_to_options(htdemucs.audio_options.unwrap()),
          },
        )
      }
    };

    let task_id_ret = task_id;
    let monitor = Arc::new(TaskMonitor {
      cancelled: RwLock::new(false),
      progress: RwLock::new(0.0),
      status: RwLock::new(TaskStatus::Running),
    });
    let monitor_ret = monitor.clone();
    let queue = self.pending_task_results.clone();

    tokio::spawn(async move {
      let monitor_clone = monitor.clone();
      let result = task.process(monitor).await;
      match result {
        Err(e) => *monitor_clone.status.write().await = TaskStatus::Failed(e),
        Ok(Some(result)) => {
          *monitor_clone.status.write().await = TaskStatus::Completed;
          queue.push(TaskResult {
            id: task_id,
            result,
          });
        }
        Ok(None) => {
          if !matches!(*monitor_clone.status.read().await, TaskStatus::Cancelled) {
            *monitor_clone.status.write().await =
              TaskStatus::Failed(Error::msg("Task completed but no result was returned."));
          }
        }
      }
    });

    self.tasks.push(Task {
      id: task_id_ret,
      _title: title.to_string(),
      monitor: monitor_ret,
    });

    Ok(())
  }

  pub async fn poll_tasks(
    &mut self,
    stream: &mut interprocess::local_socket::tokio::SendHalf,
  ) -> Result<(), Error> {
    let mut tasks_to_remove = HashSet::new();

    for task in &self.tasks {
      let status = &*task.monitor.status.read().await;
      let progress = *task.monitor.progress.read().await;

      let status_packet = match status {
        TaskStatus::Running => worker_task_info::Status::Running(WorkerTaskRunning {}),
        TaskStatus::Completed => worker_task_info::Status::Complete(WorkerTaskComplete {}),
        TaskStatus::Failed(error) => worker_task_info::Status::Failed(WorkerTaskFailed {
          error: error.to_string(),
        }),
        TaskStatus::Cancelled => worker_task_info::Status::Cancelled(WorkerTaskCancelled {}),
      };

      let packet = WorkerPacket {
        contents: Some(packet::worker_packet::Contents::TaskInfo(
          packet::WorkerTaskInfo {
            id: Some(task.id.into()),
            progress,
            status: Some(status_packet),
          },
        )),
      };

      ksng_ml_ipc::write_packet(packet, stream).await?;

      if matches!(status, TaskStatus::Completed) {
        tasks_to_remove.insert(task.id);
      }
    }

    self.tasks.retain(|t| !tasks_to_remove.contains(&t.id));

    while let Some(result) = self.pending_task_results.try_pop() {
      let packet = WorkerPacket {
        contents: Some(packet::worker_packet::Contents::TaskResult(
          WorkerTaskResult {
            id: Some(result.id.into()),
            result: Some(result.result),
          },
        )),
      };

      ksng_ml_ipc::write_packet(packet, stream).await?;
    }

    Ok(())
  }

  pub async fn cancel_task(&self, id: Uuid) -> Result<(), Error> {
    for task in &self.tasks {
      if task.id == id {
        *task.monitor.status.write().await = TaskStatus::Cancelled;
        *task.monitor.cancelled.write().await = true;
        return Ok(());
      }
    }

    Err(Error::msg(format!("Could not find task with ID {id:?}")))
  }
}
