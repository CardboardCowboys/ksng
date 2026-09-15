use std::path::PathBuf;

use ndarray::{ArrayView4, ArrayViewD, NewAxis, s};
use ort::{
  session::Session,
  value::{Tensor, TensorRef},
};

use crate::audio::{AudioChunkProvider, AudioChunkWriter};

const SOURCES: [&'static str; 4] = ["drums", "bass", "other", "vocals"];

struct HtdemucsSeparator {
  model_path: PathBuf,
  input_path: PathBuf,
  output_paths: [PathBuf; 4],
}

impl HtdemucsSeparator {
  pub fn separate(&self) -> Result<(), anyhow::Error> {
    log::info!("loading model file from {:?}", self.model_path);
    let mut model = Session::builder()?
      .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
      .map_err(|e| anyhow::Error::msg(e.message().to_string()))?
      .with_intra_threads(4)
      .map_err(|e| anyhow::Error::msg(e.message().to_string()))?
      .with_execution_providers([
        ort::ep::NVRTX::default().build(),
        ort::ep::TensorRT::default().build(),
        ort::ep::DirectML::default().build(),
        ort::ep::CUDA::default().build(),
      ])
      .map_err(|e| anyhow::Error::msg(e.message().to_string()))?
      .commit_from_file(&self.model_path)?;

    log::info!("loading audio file from {:?}", self.input_path);
    let mut audio = AudioChunkProvider::new(&self.input_path, 44100, 7.8)?;
    let mut audio_out = AudioChunkWriter::new(
      &self.output_paths,
      audio.total_frames(),
      2,
      audio.chunk_size(),
    )?;

    for inp in model.inputs() {
      log::info!("- {}: {:?}", inp.name(), inp.dtype());
    }

    for outp in model.outputs() {
      log::info!("- {}: {:?}", outp.name(), outp.dtype());
    }

    log::info!("running chunks");
    let mut stems = Vec::with_capacity(SOURCES.len());
    for _ in SOURCES {
      let size = audio.total_samples();
      let mut buf = Vec::with_capacity(size);
      buf.resize(size, 0.0_f32);
      stems.push(buf);
    }

    let mut chunk = ndarray::Array2::default((2, audio.chunk_size()));
    loop {
      let num_read = audio.next_chunk(&mut chunk)?;
      if num_read == 0 {
        break;
      }

      let x = chunk.slice(s![NewAxis, .., ..]);

      let output = model.run(ort::inputs![ "mix" => TensorRef::from_array_view(x)? ])?;
      let Some(stems) = output.get("stems") else {
        return Err(anyhow::Error::msg("No 'stems' key found in output"));
      };

      let arr: ArrayViewD<f32> = stems.try_extract_array()?;
      let arr: ArrayView4<f32> = arr.into_dimensionality()?;

      audio_out.write_chunk(&arr)?;

      if num_read < audio.chunk_size() {
        break;
      }
    }

    log::info!("writing outputs");

    audio_out.finalize()?;

    Ok(())
  }
}

#[test]
pub fn test_htdemucs() -> Result<(), anyhow::Error> {
  colog::init();

  ort::init_from("C:/Tools/ort/onnxruntime.dll")
    .unwrap()
    .with_execution_providers([ort::ep::CUDA::default().build()])
    .commit();
  let sep = HtdemucsSeparator {
    input_path: PathBuf::from("C:/Users/Ashley/Music/Burning Airlines - Outside The Aviary.mp3"),
    model_path: PathBuf::from("C:/Users/Ashley/Downloads/htdemucs.onnx"),
    output_paths: [
      PathBuf::from("C:/Users/Ashley/Downloads/aviary-drums.bin"),
      PathBuf::from("C:/Users/Ashley/Downloads/aviary-bass.bin"),
      PathBuf::from("C:/Users/Ashley/Downloads/aviary-other.bin"),
      PathBuf::from("C:/Users/Ashley/Downloads/aviary-vocals.bin"),
    ],
  };

  sep.separate()?;

  Ok(())
}
