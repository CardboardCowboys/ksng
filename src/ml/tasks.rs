use std::{collections::HashMap, sync::Arc};

use ksng_ml_ipc::packet::{
  HostCancelTask, HostPacket, HostStartTask, WorkerTaskInfo, WorkerTaskResult, host_start_task,
  worker_task_info, worker_task_result,
};
use uuid::Uuid;

use crate::util::error::UiError;

pub enum TaskResult {
  Htdemucs {
    output_path_vocals: String,
    output_path_drums: String,
    output_path_bass: String,
    output_path_other: String,
  },
}

pub enum TaskStatus {
  Running { progress: f32 },
  Complete(TaskResult),
  Failed(String),
  Cancelled,
}

pub struct Task {
  id: Uuid,
  name: String,
  status: TaskStatus,
}

pub struct TaskManager {
  tasks: Vec<Task>,
  send_queue: Arc<deadqueue::unlimited::Queue<HostPacket>>,
}

impl TaskManager {
  pub fn new(send_queue: Arc<deadqueue::unlimited::Queue<HostPacket>>) -> TaskManager {
    TaskManager {
      tasks: Vec::new(),
      send_queue,
    }
  }

  pub fn start_task(&mut self, name: String, model_id: Uuid, task: host_start_task::Task) {
    let task_id = Uuid::new_v4();
    let packet = HostPacket {
      contents: Some(ksng_ml_ipc::packet::host_packet::Contents::StartTask(
        HostStartTask {
          id: Some(task_id.into()),
          model_id: Some(model_id.into()),
          task: Some(task),
        },
      )),
    };

    self.tasks.push(Task {
      id: task_id,
      name,
      status: TaskStatus::Running { progress: 0.0 },
    });

    self.send_queue.push(packet);
  }

  pub fn cancel_task(&mut self, task_id: Uuid) {
    let packet = HostPacket {
      contents: Some(ksng_ml_ipc::packet::host_packet::Contents::CancelTask(
        HostCancelTask {
          id: Some(task_id.into()),
        },
      )),
    };
    self.send_queue.push(packet);
  }

  pub fn recv_task_info(&mut self, packet: WorkerTaskInfo) -> Result<(), UiError> {
    let task_id: Uuid = packet.id.unwrap().into();
    let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) else {
      return Err(UiError::Worker(format!(
        "Could not find task for ID {:?}",
        task_id
      )));
    };

    match packet.status.unwrap() {
      worker_task_info::Status::Running(_) => {
        task.status = TaskStatus::Running {
          progress: packet.progress,
        };
      }
      worker_task_info::Status::Complete(_) => { /* ignore - TaskResult will update us */ }
      worker_task_info::Status::Failed(err) => {
        task.status = TaskStatus::Failed(err.error);
      }
      worker_task_info::Status::Cancelled(_) => {
        task.status = TaskStatus::Cancelled;
      }
    }

    Ok(())
  }

  pub fn recv_task_result(&mut self, packet: WorkerTaskResult) -> Result<(), UiError> {
    let task_id: Uuid = packet.id.unwrap().into();
    let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) else {
      return Err(UiError::Worker(format!(
        "Could not find task for ID {:?}",
        task_id
      )));
    };

    match packet.result.unwrap() {
      worker_task_result::Result::Htdemucs(result) => {
        task.status = TaskStatus::Complete(TaskResult::Htdemucs {
          output_path_vocals: result.output_path_vocals,
          output_path_drums: result.output_path_drums,
          output_path_bass: result.output_path_bass,
          output_path_other: result.output_path_other,
        });
      }
    }

    Ok(())
  }
}
