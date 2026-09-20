use futures_util::StreamExt;
use interprocess::local_socket::tokio::SendHalf;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use tokio::{io::AsyncWriteExt, sync::RwLock};
use uuid::Uuid;

// TODO: download json from github
const DOWNLOADABLE_MODELS_JSON: &str = include_str!("models.json");

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ModelType {
  Htdemucs,
}

#[derive(Serialize, Deserialize)]
struct DownloadableModelDefinition {
  id: Uuid,
  name: String,
  model_type: ModelType,
  url: String,
  size: usize,
  description: String,
  parameters: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
struct DownloadableModels {
  models: Vec<DownloadableModelDefinition>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Model {
  id: Uuid,
  name: String,
  size: usize,
  model_type: ModelType,
  description: String,
  parameters: serde_json::Value,
}

enum ModelDownloadStatus {
  Downloading,
  Success,
  Failed(anyhow::Error),
}

struct ModelDownloadJob {
  id: Uuid,
  model_id: Uuid,
  size: Option<usize>,
  downloaded: usize,
  status: ModelDownloadStatus,
  finalized: bool,
  model: Model,
}

type JobHandle = Arc<RwLock<ModelDownloadJob>>;

#[derive(Serialize, Deserialize)]
struct ModelCache {
  models: Vec<Model>,
}

pub struct ModelManager {
  models_dir: PathBuf,
  models: RwLock<Vec<Model>>,
  jobs: RwLock<Vec<JobHandle>>,
  dl_models: DownloadableModels,
}

impl ModelManager {
  pub fn new() -> Result<ModelManager, anyhow::Error> {
    let cache_dir = directories::ProjectDirs::from("com", "Cardboard Cowboys", "ksng-ml")
      .unwrap()
      .cache_dir()
      .to_path_buf();
    let models_dir = cache_dir.join("models");
    if !std::fs::exists(&models_dir)? {
      std::fs::create_dir_all(&models_dir)?;
    }

    let mut models = Vec::new();
    let cache_file = models_dir.join("_models.json");
    if std::fs::exists(&cache_file)? {
      let file = std::fs::File::open(&cache_file)?;
      let model_cache: ModelCache = serde_json::from_reader(file)?;
      models = model_cache.models;
    }

    let dl_models = serde_json::from_str(DOWNLOADABLE_MODELS_JSON)?;

    Ok(ModelManager {
      models_dir,
      models: RwLock::new(models),
      jobs: RwLock::new(Vec::new()),
      dl_models,
    })
  }

  async fn update_model_cache(&self) -> Result<(), anyhow::Error> {
    let cache_file = self.models_dir.join("_models.json");
    let mut file = tokio::fs::File::create(&cache_file).await?;
    let models = self.models.read().await;
    let model_cache = ModelCache {
      models: models.clone(),
    };
    let s = serde_json::to_vec(&model_cache)?;
    file.write_all(&s).await?;
    Ok(())
  }

  pub async fn list_models(&self, writer: &mut SendHalf) -> Result<(), anyhow::Error> {
    use ksng_ml_ipc::packet;

    let mut models = Vec::new();
    for model in &*self.models.read().await {
      models.push(packet::Model {
        name: model.name.to_string(),
        size: model.size as u64,
        r#type: format!("{:?}", model.model_type),
        id: Some(model.id.into()),
        description: model.description.clone(),
      });
    }

    let packet = packet::WorkerPacket {
      contents: Some(packet::worker_packet::Contents::ModelsListResponse(
        packet::WorkerGetAvailableModelsListResponse { models },
      )),
    };

    ksng_ml_ipc::write_packet(packet, writer).await?;
    Ok(())
  }

  pub async fn list_dl_models(&self, writer: &mut SendHalf) -> Result<(), anyhow::Error> {
    use ksng_ml_ipc::packet;

    let mut models = Vec::new();
    for model in &self.dl_models.models {
      models.push(packet::Model {
        name: model.name.clone(),
        size: model.size as u64,
        r#type: format!("{:?}", model.model_type),
        id: Some(model.id.into()),
        description: model.description.clone(),
      });
    }

    let packet = packet::WorkerPacket {
      contents: Some(packet::worker_packet::Contents::DlModelsListResponse(
        packet::WorkerGetDownloadableModelsListResponse { models },
      )),
    };

    ksng_ml_ipc::write_packet(packet, writer).await?;
    Ok(())
  }

  pub async fn download_model_id(&self, model_id: Uuid, job_id: Uuid) -> Result<(), anyhow::Error> {
    for model in &self.dl_models.models {
      if model.id == model_id {
        return self.download_model(model, job_id).await;
      }
    }

    Err(anyhow::Error::msg(format!(
      "No model definition found for ID {model_id}"
    )))
  }

  async fn download_model(
    &self,
    model: &DownloadableModelDefinition,
    job_id: Uuid,
  ) -> Result<(), anyhow::Error> {
    let job = Arc::new(RwLock::new(ModelDownloadJob {
      id: job_id,
      model_id: model.id,
      size: Some(model.size),
      downloaded: 0,
      status: ModelDownloadStatus::Downloading,
      finalized: false,
      model: Model {
        id: model.id,
        name: model.name.clone(),
        size: model.size,
        model_type: model.model_type,
        description: model.description.clone(),
        parameters: model.parameters.clone(),
      },
    }));

    let url = model.url.to_string();
    let out_path = self
      .models_dir
      .join(model.id.to_string())
      .with_extension("onnx");

    self.jobs.write().await.push(job.clone());

    tokio::spawn(async move {
      let job_clone = job.clone();
      if let Err(err) = Self::download_model_impl(job, url, out_path).await {
        job_clone.write().await.status = ModelDownloadStatus::Failed(err);
      } else {
        job_clone.write().await.status = ModelDownloadStatus::Success;
      }
    });

    Ok(())
  }

  async fn download_model_impl(
    job: JobHandle,
    url: String,
    out_path: PathBuf,
  ) -> Result<(), anyhow::Error> {
    let mut file = tokio::fs::File::create(&out_path).await?;
    let response = reqwest::Client::new().get(url).send().await?;
    let size = response.content_length().unwrap_or(0_u64) as usize;
    let size = if job.read().await.size.is_none() {
      job.write().await.size = Some(size);
      size
    } else {
      job.read().await.size.unwrap()
    };

    let mut stream = response.bytes_stream();

    let mut downloaded = 0;
    while let Some(item) = stream.next().await {
      let item = item?;
      file.write_all(&item).await?;
      let new = (downloaded + item.len()).min(size);
      job.write().await.downloaded = new;
      downloaded = new;
    }

    Ok(())
  }

  pub async fn poll_jobs(&self, stream: &mut SendHalf) -> Result<(), anyhow::Error> {
    use ksng_ml_ipc::packet;
    let mut has_new_models = false;

    for job_handle in &*self.jobs.read().await {
      let job = job_handle.read().await;
      if job.finalized {
        continue;
      }

      match &job.status {
        ModelDownloadStatus::Downloading => {
          let packet = packet::WorkerPacket {
            contents: Some(packet::worker_packet::Contents::JobStatusResponse(
              packet::WorkerJobStatusResponse {
                model_id: Some(job.model_id.into()),
                job_id: Some(job.id.into()),
                total_size: job.size.map(|s| s as u64).unwrap_or(u64::MAX),
                downloaded_size: job.downloaded as u64,
                status: Some(packet::worker_job_status_response::Status::Downloading(
                  packet::JobStatusDownloading {},
                )),
              },
            )),
          };
          ksng_ml_ipc::write_packet(packet, stream).await?;
        }
        ModelDownloadStatus::Success => {
          let packet = packet::WorkerPacket {
            contents: Some(packet::worker_packet::Contents::JobStatusResponse(
              packet::WorkerJobStatusResponse {
                model_id: Some(job.model_id.into()),
                job_id: Some(job.id.into()),
                total_size: job.size.map(|s| s as u64).unwrap_or(u64::MAX),
                downloaded_size: job.downloaded as u64,
                status: Some(packet::worker_job_status_response::Status::Complete(
                  packet::JobStatusComplete {},
                )),
              },
            )),
          };
          ksng_ml_ipc::write_packet(packet, stream).await?;
          self.models.write().await.push(job.model.clone());
          has_new_models = true;
          drop(job);
          job_handle.write().await.finalized = true;
        }
        ModelDownloadStatus::Failed(err) => {
          let packet = packet::WorkerPacket {
            contents: Some(packet::worker_packet::Contents::JobStatusResponse(
              packet::WorkerJobStatusResponse {
                model_id: Some(job.model_id.into()),
                job_id: Some(job.id.into()),
                total_size: job.size.map(|s| s as u64).unwrap_or(u64::MAX),
                downloaded_size: job.downloaded as u64,
                status: Some(packet::worker_job_status_response::Status::Error(
                  packet::JobStatusError {
                    error: err.to_string(),
                  },
                )),
              },
            )),
          };
          ksng_ml_ipc::write_packet(packet, stream).await?;
          drop(job);
          job_handle.write().await.finalized = true;
        }
      }
    }

    if has_new_models {
      self.update_model_cache().await?;
    }

    Ok(())
  }

  pub async fn find_model_path_params(&self, id: Uuid) -> Option<(PathBuf, serde_json::Value)> {
    let models = self.models.read().await;
    for model in models.iter() {
      if model.id == id {
        let path = self
          .models_dir
          .join(model.id.to_string())
          .with_extension("onnx");
        return Some((path, model.parameters.clone()));
      }
    }

    None
  }
}
