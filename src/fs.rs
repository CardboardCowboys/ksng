use std::path::{Path, PathBuf};

use klib::objects::file::File;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{preferences::Preferences, project::Project, util::error::UiError};

#[derive(Serialize, Deserialize)]
pub struct ProjectEntry {
  pub id: Uuid,
  pub name: String,
  pub last_modified: chrono::NaiveDateTime,
}

#[derive(Serialize, Deserialize, Default)]
pub struct ProjectManifest {
  pub entries: Vec<ProjectEntry>,
}

pub struct Data {}

impl Data {
  pub fn load_preferences() -> Result<Preferences, UiError> {
    use std::io::Read;
    let root_dir = Data::root_dir()?;

    std::fs::File::open(root_dir.join("config.json"))
      .map_err(UiError::from)
      .and_then(|mut f| {
        let mut s = String::new();
        f.read_to_string(&mut s)?;
        Ok(s)
      })
      .and_then(|s| serde_json::from_str::<Preferences>(&s).map_err(|e| e.into()))
      .or(Ok(Preferences::default()))
  }

  pub fn save_preferences(preferences: &Preferences) -> Result<(), UiError> {
    let root_dir = Data::root_dir()?;
    std::fs::File::create(root_dir.join("config.json"))
      .map_err(|e| e.into())
      .and_then(|mut f| {
        use std::io::Write;

        let str = serde_json::to_string(&preferences)?;
        f.write_all(str.as_bytes())?;
        Ok(())
      })
  }

  pub fn list_projects() -> Result<ProjectManifest, UiError> {
    let root_dir = Data::root_dir()?;
    let manifest = Data::load_or_create_manifest(&root_dir)?;
    Ok(manifest)
  }

  pub fn save_project(project: &Project) -> Result<(), UiError> {
    let root_dir = Data::root_dir()?;

    let mut manifest = Data::load_or_create_manifest(&root_dir)?;
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
      .sort_by_key(|b| std::cmp::Reverse(b.last_modified));

    Data::save_manifest(&root_dir, &manifest)?;

    let project_dir = Data::get_or_create_project_dir(&root_dir, project.id)?;
    let project_file = std::fs::File::create(project_dir.join("project.kpj"))?;
    project.file.write_to_file(project_file)?;

    Ok(())
  }

  pub fn load_project(id: Uuid, manifest: &ProjectManifest) -> Result<Project, UiError> {
    let entry = manifest.entries.iter().find(|f| f.id == id);
    if entry.is_none() {
      return Err(UiError::Io(format!("Missing project file {id}")));
    }

    let entry = entry.unwrap();

    let root_dir = Data::root_dir()?;
    let project_dir = Data::get_or_create_project_dir(&root_dir, entry.id)?;
    let project_file = std::fs::File::open(project_dir.join("project.kpj"))?;
    let kfile = File::read_from_file(project_file)?;

    Ok(Project::from_file(entry.id, entry.name.clone(), kfile))
  }

  pub fn delete_project(id: Uuid) -> Result<(), UiError> {
    let root_dir = Data::root_dir()?;
    let dir = root_dir.join(id.to_string());

    if std::fs::exists(&dir)? {
      std::fs::remove_dir_all(&dir)?;
    }

    let mut manifest = Data::load_or_create_manifest(&root_dir)?;
    manifest.entries.retain(|f| f.id != id);

    Data::save_manifest(&root_dir, &manifest)?;
    Ok(())
  }

  fn load_or_create_manifest(root_dir: &Path) -> Result<ProjectManifest, UiError> {
    use std::io::Read;

    std::fs::File::open(root_dir.join("manifest.json"))
      .map_err(UiError::from)
      .and_then(|mut f| {
        let mut s = String::new();
        f.read_to_string(&mut s)?;
        Ok(s)
      })
      .and_then(|s| serde_json::from_str::<ProjectManifest>(&s).map_err(|e| e.into()))
      .or(Ok(ProjectManifest::default()))
  }

  fn save_manifest(root_dir: &Path, manifest: &ProjectManifest) -> Result<(), UiError> {
    std::fs::File::create(root_dir.join("manifest.json"))
      .map_err(|e| e.into())
      .and_then(|mut f| {
        use std::io::Write;

        let str = serde_json::to_string(&manifest)?;
        f.write_all(str.as_bytes())?;
        Ok(())
      })
  }

  fn get_or_create_project_dir(root_dir: &Path, id: Uuid) -> Result<PathBuf, UiError> {
    let path = root_dir.join(id.to_string());
    if !std::fs::exists(&path)? {
      std::fs::create_dir(&path)?;
    }
    Ok(path)
  }

  fn root_dir() -> Result<PathBuf, UiError> {
    let root_dir = directories::ProjectDirs::from("com", "Cardboard Cowboys", "ksng")
      .ok_or(UiError::Io(
        "Couldn't obtain path to data directory".to_string(),
      ))?
      .data_dir()
      .to_path_buf();
    if !std::fs::exists(&root_dir)? {
      std::fs::create_dir_all(&root_dir)?;
    }

    Ok(root_dir)
  }
}

pub struct Cache;

impl Cache {
  pub fn get_file_path(id: Uuid, ext: &str) -> Result<PathBuf, UiError> {
    Self::cache_dir().map(|p| p.join(format!("{id}.{ext}")))
  }

  fn cache_dir() -> Result<PathBuf, UiError> {
    let cache_dir = directories::ProjectDirs::from("com", "Cardboard Cowboys", "ksng")
      .ok_or(UiError::Io(
        "Couldn't obtain path to cache directory".to_string(),
      ))?
      .cache_dir()
      .to_path_buf();

    if !std::fs::exists(&cache_dir)? {
      std::fs::create_dir_all(&cache_dir)?;
    }

    Ok(cache_dir)
  }
}
