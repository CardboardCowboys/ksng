use std::path::Path;

mod cuda;
pub mod ort;

macro_rules! check_path {
  ($base:expr, $rel:literal) => {{
    let r = $base.join($rel);
    if std::fs::exists(&r)? {
      return Ok(Some(r));
    }
  };};
}

pub(crate) use check_path;

#[derive(PartialEq)]
pub struct Version(u32, u32, String);

impl Version {
  pub fn new(major: u32, minor: u32, rest: &str) -> Version {
    Version(major, minor, rest.to_owned())
  }

  #[allow(dead_code)]
  pub fn from_str(s: &str) -> Option<Version> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() < 2 {
      return None;
    }

    let major = parts[0].parse::<u32>().ok()?;
    let minor = parts[1].parse::<u32>().ok()?;
    if parts.len() > 2 {
      Some(Version(major, minor, parts[2..].join(".")))
    } else {
      Some(Version(major, minor, String::default()))
    }
  }
}

impl PartialOrd for Version {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    match self.0.partial_cmp(&other.0) {
      Some(core::cmp::Ordering::Equal) => {}
      ord => return ord,
    }
    self.1.partial_cmp(&other.1)
  }
}

pub fn get_dll_metadata(_path: &Path) -> Option<Version> {
  #[cfg(target_os = "windows")]
  {
    match get_dll_metadata_windows(_path) {
      Ok(res) => res,
      Err(e) => {
        log::error!("error getting dll metadata for {_path:?}: {e:?}");
        None
      }
    }
  }
  #[cfg(not(target_os = "windows"))]
  {
    None
  }
}

#[cfg(target_os = "windows")]
fn get_dll_metadata_windows(path: &Path) -> Result<Option<Version>, anyhow::Error> {
  let image = editpe::Image::parse_file(path)?;
  let Some(resources) = image.resource_directory() else {
    return Ok(None);
  };

  let Some(info) = resources.get_version_info()? else {
    return Ok(None);
  };

  let Some(table) = info.strings.first() else {
    return Ok(None);
  };

  for (k, v) in &table.strings {
    if k == "FileVersion"
      && let Some(version) = Version::from_str(v.as_str())
    {
      return Ok(Some(version));
    }
  }

  Ok(None)
}
