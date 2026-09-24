use std::sync::Arc;

use ksng_ml_ipc::packet::{
  HostDownloadListedModel, HostPacket, WorkerGetAvailableModelsListResponse,
  WorkerGetDownloadableModelsListResponse, WorkerJobStatusResponse, host_packet::Contents,
  worker_job_status_response,
};
use uuid::Uuid;

use crate::util::error::UiError;

pub enum ModelDownloadStatus {
  NotDownloaded,
  Downloaded,
  Downloading { progress: f32 },
  Failed(String),
}

pub struct Model {
  pub id: Uuid,
  pub name: String,
  pub model_type: String,
  pub size: usize,
  pub description: String,
  pub download_status: ModelDownloadStatus,
}

pub struct ModelManager {
  models: Vec<Model>,
  send_queue: Arc<deadqueue::unlimited::Queue<HostPacket>>,
  is_loading_models_list: bool,
  is_loading_dl_models_list: bool,
}

impl ModelManager {
  pub fn new(send_queue: Arc<deadqueue::unlimited::Queue<HostPacket>>) -> ModelManager {
    ModelManager {
      models: Vec::new(),
      is_loading_models_list: true,
      is_loading_dl_models_list: true,
      send_queue,
    }
  }

  pub fn models(&self) -> Option<&[Model]> {
    if self.is_loading_models_list || self.is_loading_dl_models_list {
      return None;
    }

    Some(&self.models)
  }

  pub fn download_model(&self, model_id: Uuid) {
    self.send_queue.push(HostPacket {
      contents: Some(Contents::DlListedModel(HostDownloadListedModel {
        model_id: Some(model_id.into()),
        job_id: Some(Uuid::new_v4().into()),
      })),
    });
  }

  pub fn recv_models_list(&mut self, list: WorkerGetAvailableModelsListResponse) {
    for model in list.models {
      self.add_or_update_model(model, true);
    }
    self.is_loading_models_list = false;
  }

  pub fn recv_dl_models_list(&mut self, list: WorkerGetDownloadableModelsListResponse) {
    for model in list.models {
      self.add_or_update_model(model, false);
    }
    self.is_loading_dl_models_list = false;
  }

  pub fn recv_job_status(&mut self, update: WorkerJobStatusResponse) -> Result<(), UiError> {
    let model_id: Uuid = update.model_id.unwrap().into();
    let Some(model) = self.models.iter_mut().find(|m| m.id == model_id) else {
      return Err(UiError::Worker(format!(
        "Could not find model with ID {:?}",
        model_id
      )));
    };
    let status = update.status.unwrap();
    match status {
      worker_job_status_response::Status::Downloading(_) => {
        let progress = update.downloaded_size as f32 / update.total_size as f32;
        model.download_status = ModelDownloadStatus::Downloading {
          progress: progress.clamp(0.0, 1.0),
        };
      }
      worker_job_status_response::Status::Complete(_) => {
        model.download_status = ModelDownloadStatus::Downloaded;
      }
      worker_job_status_response::Status::Error(error) => {
        model.download_status = ModelDownloadStatus::Failed(error.error);
      }
    }

    Ok(())
  }

  fn add_or_update_model(&mut self, model: ksng_ml_ipc::packet::Model, downloaded: bool) {
    let id: Uuid = model.id.unwrap().into();
    for existing_model in &mut self.models {
      if existing_model.id == id {
        if matches!(
          existing_model.download_status,
          ModelDownloadStatus::NotDownloaded | ModelDownloadStatus::Failed(..)
        ) && downloaded
        {
          existing_model.download_status = ModelDownloadStatus::Downloaded;
        }
        return;
      }
    }

    self.models.push(Model {
      id,
      name: model.name,
      model_type: model.r#type,
      size: model.size as usize,
      description: model.description,
      download_status: if downloaded {
        ModelDownloadStatus::Downloaded
      } else {
        ModelDownloadStatus::NotDownloaded
      },
    })
  }
}
