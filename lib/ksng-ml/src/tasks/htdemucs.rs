use std::{path::PathBuf, sync::Arc};

use ksng_ml_ipc::packet::{HtdemucsResult, worker_task_result};
use ndarray::{ArrayView4, ArrayViewD, NewAxis, s};
use ort::value::TensorRef;
use spectrasonic::encoders::AudioEncoderOptions;

use crate::{
  audio::{AudioChunkProvider, AudioChunkWriter},
  tasks::{TaskImpl, TaskMonitor},
};

//const SOURCES: [&'static str; 4] = ["drums", "bass", "other", "vocals"];

pub struct HtdemucsSeparator {
  pub model_path: PathBuf,
  pub input_path: PathBuf,
  pub output_paths: [PathBuf; 4],
}

impl TaskImpl for HtdemucsSeparator {
  async fn process(
    &self,
    monitor: Arc<TaskMonitor>,
  ) -> Result<Option<worker_task_result::Result>, anyhow::Error> {
    log::info!("loading audio file from {:?}", self.input_path);
    let mut audio = AudioChunkProvider::new(&self.input_path, 44100, 2, 7.8)?;
    let mut audio_out = AudioChunkWriter::new(
      &self.output_paths,
      spectrasonic::encoders::AudioCodec::Flac,
      AudioEncoderOptions {
        options: String::default(),
        bit_rate: 128000,
      },
      audio.total_frames(),
      2,
      44100,
      audio.chunk_size(),
    )?;

    log::info!("loading model file from {:?}", self.model_path);
    let mut model = crate::ort::session_with_model(&self.model_path)?;

    for inp in model.inputs() {
      log::info!("- {}: {:?}", inp.name(), inp.dtype());
    }

    for outp in model.outputs() {
      log::info!("- {}: {:?}", outp.name(), outp.dtype());
    }

    log::info!("running chunks");
    let mut chunk = ndarray::Array2::default((2, audio.chunk_size()));
    let total_chunks = audio.total_chunks();
    loop {
      if *monitor.cancelled.read().await {
        return Ok(None);
      }

      let num_read = audio.next_chunk(&mut chunk)?;
      if num_read == 0 {
        log::info!("num_read: {num_read}");
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

      let progress = audio.current_chunk() as f32 / total_chunks as f32;
      *monitor.progress.write().await = progress.clamp(0.0, 1.0);

      if num_read < audio.chunk_size() {
        log::info!("num_read: {num_read}");
        break;
      }
    }

    log::info!("writing outputs");

    let paths = audio_out.finalize()?;
    assert!(paths.len() == 4);

    Ok(Some(worker_task_result::Result::Htdemucs(HtdemucsResult {
      output_path_drums: paths[0].to_str().unwrap().to_string(),
      output_path_bass: paths[1].to_str().unwrap().to_string(),
      output_path_other: paths[2].to_str().unwrap().to_string(),
      output_path_vocals: paths[3].to_str().unwrap().to_string(),
    })))
  }
}
