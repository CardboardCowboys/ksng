use std::{
  path::{Path, PathBuf},
  process::Command,
};

use ort::session::Session;
use regex::Regex;

macro_rules! check_path {
  ($base:expr, $rel:literal) => {{
    let r = $base.join($rel);
    if std::fs::exists(&r)? {
      return Ok(Some(r));
    }
  };};
}

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

fn find_cuda_cudnn(paths: &[PathBuf]) -> Result<Option<(PathBuf, PathBuf)>, anyhow::Error> {
  let cuda = find_cuda(paths)?;
  let cudnn = find_cudnn(paths)?;

  if let Some(cuda) = cuda
    && let Some(cudnn) = cudnn
  {
    return Ok(Some((cuda, cudnn)));
  }

  Ok(None)
}

fn find_ort_runtime(path: &Path) -> Result<Option<PathBuf>, anyhow::Error> {
  check_path!(path, "onnxruntime.dll");
  check_path!(path, "onnxruntime.so");
  check_path!(path, "onnxruntime.dylib");

  Ok(None)
}

fn find_ort(paths: &[PathBuf]) -> Result<Option<PathBuf>, anyhow::Error> {
  let cwd = std::env::current_dir()?;
  if let Some(ort) = find_ort_runtime(&cwd)? {
    return Ok(Some(ort));
  }

  let exe = std::env::current_exe()?;
  if let Some(dir) = exe.parent()
    && let Some(ort) = find_ort_runtime(dir)?
  {
    return Ok(Some(ort));
  }

  for path in paths {
    if let Some(ort) = find_ort_runtime(path)? {
      return Ok(Some(ort));
    }
  }

  Ok(None)
}

pub fn init_ort() -> Result<(), anyhow::Error> {
  let path = std::env::var("PATH")?;
  let paths = std::env::split_paths(&path);
  let mut paths_arr = Vec::new();
  for path in paths {
    paths_arr.push(path);
  }

  let mut providers = Vec::new();
  if let Some((cuda, cudnn)) = find_cuda_cudnn(&paths_arr)? {
    log::info!("using CUDA for onnxruntime, found CUDA at {cuda:?} and cuDNN at {cudnn:?}");
    paths_arr.push(cuda);
    paths_arr.push(cudnn);
    providers.push(ort::ep::CUDA::default().build());
  }

  let ort = find_ort(&paths_arr)?;

  let paths_env = std::env::join_paths(&paths_arr)?;
  unsafe {
    std::env::set_var("PATH", paths_env);
  }

  let builder = if let Some(ort) = ort {
    ort::init_from(ort)?
  } else {
    ort::init()
  };

  builder.with_execution_providers(providers).commit();

  Ok(())
}

pub fn session_with_model(model: &Path) -> Result<Session, anyhow::Error> {
  let model = Session::builder()?
    .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
    .map_err(|e| anyhow::Error::msg(e.message().to_string()))?
    .with_intra_threads(4)
    .map_err(|e| anyhow::Error::msg(e.message().to_string()))?
    .commit_from_file(model)?;
  Ok(model)
}
