use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use klib::objects::file::File;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
  async_handler::{AsyncValue, AsyncValueState},
  error::UiError,
  logger::Logger,
  project::Project,
};

#[derive(Serialize, Deserialize, Clone)]
pub struct ProjectEntry {
  pub id: Uuid,
  pub name: String,
  pub last_modified: chrono::NaiveDateTime,
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct ProjectManifest {
  pub entries: Vec<ProjectEntry>,
}

pub struct Data {
  logger: Arc<Logger>,
  manifest: AsyncValue<ProjectManifest>,
}

impl Data {
  pub fn new(logger: Arc<Logger>) -> Data {
    Data {
      manifest: AsyncValue::new(logger.clone()),
      logger,
    }
  }

  pub fn list_projects(&self) -> AsyncValue<ProjectManifest> {
    self.load_or_create_manifest()
  }

  pub async fn save_project(&self, project: &Project) -> Result<(), UiError> {
    let manifest_ptr = self.load_or_create_manifest().get();
    let mut manifest = (*manifest_ptr)
      .as_ref()
      .ok_or(UiError::Io("Project manifest not yet loaded".to_string()))?
      .clone();
    let manifest_entry = manifest.entries.iter_mut().find(|e| e.id == project.id);
    if let Some(entry) = manifest_entry {
      entry.name = project.name.clone().unwrap_or_default();
      entry.last_modified = chrono::offset::Local::now().naive_local();
    } else {
      let entry = ProjectEntry {
        id: project.id,
        name: project.name.clone().unwrap_or_default(),
        last_modified: chrono::offset::Local::now().naive_local(),
      };
      manifest.entries.push(entry);
    }

    manifest
      .entries
      .sort_by(|a, b| b.last_modified.cmp(&a.last_modified));

    let project_dir = self.get_or_create_project_dir(project.id).await?;
    Filesystem::write_bytes_to_file(
      &project_dir.join("project.kpj"),
      &project.file.write_to_bytes()?,
    )
    .await?;

    self.save_manifest(manifest).await?;

    Ok(())
  }

  pub async fn load_project(&self, id: Uuid) -> Result<Project, UiError> {
    let manifest_ptr = self.load_or_create_manifest().get();
    let manifest = (*manifest_ptr)
      .as_ref()
      .ok_or(UiError::Io("Project manifest not yet loaded".to_string()))?;
    let entry = manifest.entries.iter().find(|f| f.id == id);
    if entry.is_none() {
      return Err(UiError::Io(format!("Missing project file {id}")));
    }

    let entry = entry.unwrap();

    let project_dir = self.get_or_create_project_dir(entry.id).await?;
    let project_file = std::fs::File::open(project_dir.join("project.kpj"))?;
    let kfile = File::read_from_file(project_file)?;

    Ok(Project {
      id: entry.id,
      name: Some(entry.name.clone()),
      file: kfile,
      dirty: false,
    })
  }

  pub async fn delete_project(&self, id: Uuid) -> Result<(), UiError> {
    let dir = PathBuf::from(id.to_string());

    if Filesystem::exists(&dir).await? {
      Filesystem::remove_dir(&dir).await?;
    }

    let manifest_ptr = self.load_or_create_manifest().get();
    let mut manifest = (*manifest_ptr)
      .as_ref()
      .ok_or(UiError::Io("Project manifest not yet loaded".to_string()))?
      .clone();
    manifest.entries.retain(|f| f.id != id);

    self.save_manifest(manifest).await?;
    Ok(())
  }

  fn load_or_create_manifest(&self) -> AsyncValue<ProjectManifest> {
    if self.manifest.state() == AsyncValueState::Unloaded {
      self.manifest.load(async || {
        Filesystem::read_file_to_bytes(&PathBuf::from("manifest.json"))
          .await
          .and_then(|bytes| {
            String::from_utf8(bytes)
              .map_err(|e| UiError::Io(format!("Can't create string from bytes: {e:?}")))
          })
          .and_then(|s| serde_json::from_str::<ProjectManifest>(&s).map_err(|e| e.into()))
          .or(Ok(ProjectManifest::default()))
      });
    }

    self.manifest.clone()
  }

  async fn save_manifest(&self, manifest: ProjectManifest) -> Result<(), UiError> {
    let string = serde_json::to_string(&manifest)?;
    self.manifest.set(Some(manifest));
    Filesystem::write_bytes_to_file(&PathBuf::from("manifest.json"), string.as_bytes()).await
  }

  async fn get_or_create_project_dir(&self, id: Uuid) -> Result<PathBuf, UiError> {
    let path = PathBuf::from(id.to_string());
    if !Filesystem::exists(&path).await? {
      Filesystem::create_dir(&path).await?;
    }
    Ok(path)
  }
}

struct Filesystem;

impl Filesystem {
  pub async fn exists(path: &Path) -> Result<bool, UiError> {
    let root_dir = Self::root_dir().await?;
    tokio::fs::try_exists(root_dir.join(path))
      .await
      .map_err(UiError::from)
  }

  pub async fn create_dir(path: &Path) -> Result<(), UiError> {
    let root_dir = Self::root_dir().await?;
    tokio::fs::create_dir_all(root_dir.join(path))
      .await
      .map_err(UiError::from)
  }

  pub async fn remove_dir(path: &Path) -> Result<(), UiError> {
    let root_dir = Self::root_dir().await?;
    tokio::fs::remove_dir_all(root_dir.join(path))
      .await
      .map_err(UiError::from)
  }

  pub async fn read_file_to_bytes(path: &Path) -> Result<Vec<u8>, UiError> {
    use tokio::io::AsyncReadExt;

    let root_dir = Self::root_dir().await?;
    let mut file = tokio::fs::File::open(root_dir.join(path)).await?;

    let mut buf = Vec::new();
    file.read_to_end(&mut buf).await?;
    Ok(buf)
  }

  pub async fn write_bytes_to_file(path: &Path, bytes: &[u8]) -> Result<(), UiError> {
    use tokio::io::AsyncWriteExt;

    let root_dir = Self::root_dir().await?;
    let mut file = tokio::fs::File::create(root_dir.join(path)).await?;
    file.write_all(bytes).await.map_err(UiError::from)
  }

  async fn root_dir() -> Result<PathBuf, UiError> {
    let root_dir = directories::ProjectDirs::from("com", "Cardboard Cowboys", "ksng")
      .ok_or(UiError::Io(
        "Couldn't obtain path to data directory".to_string(),
      ))?
      .data_dir()
      .to_path_buf();
    if !tokio::fs::try_exists(&root_dir).await? {
      tokio::fs::create_dir_all(&root_dir).await?;
    }

    Ok(root_dir)
  }
}
