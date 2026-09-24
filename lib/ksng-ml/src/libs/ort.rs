use std::path::{Path, PathBuf};

use ort::session::Session;

use crate::libs::{Version, check_path, cuda::find_cuda_cudnn, get_dll_metadata};

fn find_ort_runtime(path: &Path) -> Result<Option<PathBuf>, anyhow::Error> {
  check_path!(path, "onnxruntime.dll");
  check_path!(path, "onnxruntime.so");
  check_path!(path, "onnxruntime.dylib");

  Ok(None)
}

fn find_ort(paths: &[PathBuf]) -> Result<Option<PathBuf>, anyhow::Error> {
  let target_version = Version::new(1, 27, "");

  let cwd = std::env::current_dir()?;
  if let Some(ort) = find_ort_runtime(&cwd)? {
    if let Some(version) = get_dll_metadata(&ort) {
      if version >= target_version {
        return Ok(Some(ort));
      }
    } else {
      return Ok(Some(ort));
    }
  }

  let exe = std::env::current_exe()?;
  if let Some(dir) = exe.parent()
    && let Some(ort) = find_ort_runtime(dir)?
  {
    if let Some(version) = get_dll_metadata(&ort) {
      if version >= target_version {
        return Ok(Some(ort));
      }
    } else {
      return Ok(Some(ort));
    }
  }

  for path in paths {
    if let Some(ort) = find_ort_runtime(path)? {
      if let Some(version) = get_dll_metadata(&ort) {
        if version >= target_version {
          return Ok(Some(ort));
        }
      } else {
        return Ok(Some(ort));
      }
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
  let (cuda, cudnn) = find_cuda_cudnn(&paths_arr)?;
  if let Some(cuda) = &cuda {
    log::info!("using TensorRT & TensorRT RTX for onnxruntime, found CUDA at {cuda:?}");
    paths_arr.push(cuda.clone());
    providers.push(ort::ep::TensorRT::default().build());
    providers.push(ort::ep::NVRTX::default().build());
  }

  if let Some(cuda) = &cuda
    && let Some(cudnn) = &cudnn
  {
    log::info!("using CUDA for onnxruntime, found CUDA at {cuda:?} and cuDNN at {cudnn:?}");
    paths_arr.push(cuda.clone());
    paths_arr.push(cudnn.clone());
    providers.push(ort::ep::CUDA::default().build());
  }

  if providers.is_empty() {
    log::info!("running ort with CPU inference - this is slow!");
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
