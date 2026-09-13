#[allow(unused_imports)]
use std::{
  path::{Path, PathBuf},
  str::FromStr,
};

use glob::glob;

fn main() {
  #[cfg(not(feature = "build-ffmpeg"))]
  {
    let out_dir = PathBuf::from_str(&std::env::var("OUT_DIR").unwrap())
      .unwrap()
      .join("../../../")
      .canonicalize()
      .unwrap();
    let Some(ffmpeg_dir) = std::env::var("FFMPEG_DIR").ok() else {
      panic!("You must set your `FFMPEG_DIR` to a path containing the FFMPEG dll/so/dylib files");
    };

    let ffmpeg_dir = PathBuf::from_str(&ffmpeg_dir).unwrap();
    let libs = vec![
      "avcodec",
      "avdevice",
      "avformat",
      "avutil",
      "swresample",
      "swscale",
    ];

    let Some(libs) = find_libs(&ffmpeg_dir, &libs) else {
      panic!("Could not find FFmpeg shared libraries in FFMPEG_DIR");
    };

    for lib in libs {
      let out_path = out_dir.join(lib.file_name().unwrap());
      if !std::fs::exists(&out_path).unwrap() {
        std::fs::copy(&lib, &out_path).unwrap();
      }
    }
  }
}

#[allow(dead_code)]
fn find_libs(base_path: &Path, names: &[&str]) -> Option<Vec<PathBuf>> {
  let found: Vec<PathBuf> = names
    .iter()
    .map(|n| find_lib(base_path, n))
    .filter(|n| n.is_some())
    .flatten()
    .collect();

  if found.len() == names.len() {
    return Some(found);
  }

  let found: Vec<PathBuf> = names
    .iter()
    .map(|n| find_lib(&base_path.join("lib"), n))
    .filter(|n| n.is_some())
    .flatten()
    .collect();

  if found.len() == names.len() {
    return Some(found);
  }

  let found: Vec<PathBuf> = names
    .iter()
    .map(|n| find_lib(&base_path.join("bin"), n))
    .filter(|n| n.is_some())
    .flatten()
    .collect();

  if found.len() == names.len() {
    Some(found)
  } else {
    None
  }
}

#[allow(dead_code)]
fn find_lib(in_path: &Path, name: &str) -> Option<PathBuf> {
  let path = in_path.to_str().unwrap();
  let ext = if cfg!(target_os = "windows") {
    "dll"
  } else if cfg!(target_os = "macos") {
    "dylib"
  } else {
    "so"
  };
  let pattern = format!("{path}/*{name}-*.{ext}");

  glob(&pattern).unwrap().next().map(|e| e.unwrap())
}
