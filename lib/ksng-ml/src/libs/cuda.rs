use std::{
  path::{Path, PathBuf},
  process::Command,
};

use regex::Regex;

use crate::libs::check_path;

fn find_nvcc(path: &Path) -> Result<Option<PathBuf>, anyhow::Error> {
  check_path!(path, "nvcc");
  check_path!(path, "nvcc.exe");
  check_path!(path, "bin/nvcc");
  check_path!(path, "bin/nvcc.exe");

  Ok(None)
}

fn check_cuda_version(path: &Path) -> Result<bool, anyhow::Error> {
  let regex = Regex::new(r"Cuda compilation tools, release (\d+)\.(\d+),")?;
  let output = Command::new(path).arg("--version").output()?;
  if !output.status.success() {
    return Ok(false);
  }

  let s = String::from_utf8(output.stdout)?;
  if let Some(captures) = regex.captures(&s) {
    let Some(major) = captures.get(1) else {
      return Ok(false);
    };

    let Some(major) = major.as_str().parse::<i32>().ok() else {
      return Ok(false);
    };

    let Some(minor) = captures.get(2) else {
      return Ok(false);
    };

    let Some(minor) = minor.as_str().parse::<i32>().ok() else {
      return Ok(false);
    };

    if (major == 13 && minor >= 2) || major > 13 {
      return Ok(true);
    }
  }

  Ok(false)
}

fn find_cuda(paths: &[PathBuf]) -> Result<Option<PathBuf>, anyhow::Error> {
  for path in paths {
    let Some(nvcc) = find_nvcc(path)? else {
      continue;
    };

    if check_cuda_version(&nvcc)? {
      return Ok(Some(path.clone()));
    }
  }

  #[cfg(target_os = "windows")]
  {
    let base = PathBuf::from("C:/Program Files/NVIDIA GPU Computing Toolkit/CUDA");
    if std::fs::exists(&base)? {
      for dir in std::fs::read_dir(&base)? {
        let Some(entry) = dir.ok() else {
          continue;
        };

        let name = entry.file_name();
        let Some(s) = name.to_str().and_then(|n| n.strip_prefix('v')) else {
          continue;
        };

        let parts: Vec<i32> = s.split('.').flat_map(|i| i.parse::<i32>()).collect();
        if parts.len() >= 2 && (parts[0] == 13 && parts[1] >= 2) || parts[0] > 13 {
          return Ok(Some(entry.path()));
        }
      }
    }
  }

  Ok(None)
}

fn find_cudnn64(path: &Path) -> Result<Option<PathBuf>, anyhow::Error> {
  check_path!(path, "cudnn64_9.dll");
  check_path!(path, "cudnn64_9.so");
  check_path!(path, "cudnn64_9.dylib");

  Ok(None)
}

fn find_cudnn(paths: &[PathBuf]) -> Result<Option<PathBuf>, anyhow::Error> {
  for path in paths {
    if find_cudnn64(path)?.is_some() {
      return Ok(Some(path.clone()));
    }

    let rel = path.join("bin");
    if std::fs::exists(&rel)? {
      for entry in std::fs::read_dir(&rel)? {
        let Some(entry) = entry.ok() else {
          continue;
        };

        let rel = entry.path().join("x64");
        if find_cudnn64(&rel)?.is_some() {
          return Ok(Some(rel));
        }
      }
    }
  }

  Ok(None)
}

pub fn find_cuda_cudnn(paths: &[PathBuf]) -> Result<Option<(PathBuf, PathBuf)>, anyhow::Error> {
  let cuda = find_cuda(paths)?;
  let cudnn = find_cudnn(paths)?;

  if let Some(cuda) = cuda
    && let Some(cudnn) = cudnn
  {
    return Ok(Some((cuda, cudnn)));
  }

  Ok(None)
}
