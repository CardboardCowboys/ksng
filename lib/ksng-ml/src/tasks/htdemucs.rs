use std::{path::PathBuf, sync::Arc};

use ksng_ml_ipc::packet::{HtdemucsResult, worker_task_result};
use ndarray::{ArrayView4, ArrayViewD, NewAxis, s};
use ort::value::TensorRef;
use serde::{Deserialize, Serialize};
use spectrasonic::encoders::{AudioCodec, AudioEncoderOptions};

use crate::{
  audio::{AudioChunkProvider, AudioChunkWriter},
  tasks::{TaskImpl, TaskMonitor},
};

//const SOURCES: [&'static str; 4] = ["drums", "bass", "other", "vocals"];

#[derive(Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum OutputType {
  Drums,
  Bass,
  Other,
  Vocals,
}

#[derive(Serialize, Deserialize)]
pub struct HtdemucsParameters {
  outputs: Vec<OutputType>,
  chunk_size: usize,
  sample_rate: usize,
  channels: usize,
}

pub struct HtdemucsSeparator {
  pub model_path: PathBuf,
  pub input_path: PathBuf,
  pub parameters: HtdemucsParameters,
  pub output_path_drums: PathBuf,
  pub output_path_bass: PathBuf,
  pub output_path_other: PathBuf,
  pub output_path_vocals: PathBuf,
  pub codec: AudioCodec,
  pub audio_options: AudioEncoderOptions,
}

impl HtdemucsSeparator {
  fn output_path_for(
    codec: &AudioCodec,
    outputs: &[OutputType],
    paths: &[PathBuf],
    output_type: OutputType,
  ) -> String {
    for (i, output) in outputs.iter().enumerate() {
      if *output == output_type {
        return codec.set_extension(&paths[i]).to_str().unwrap().to_string();
      }
    }

    String::default()
  }
}

impl TaskImpl for HtdemucsSeparator {
  async fn process(
    &self,
    monitor: Arc<TaskMonitor>,
  ) -> Result<Option<worker_task_result::Result>, anyhow::Error> {
    log::info!("loading audio file from {:?}", self.input_path);
    let mut audio = AudioChunkProvider::new(
      &self.input_path,
      self.parameters.sample_rate,
      self.parameters.channels,
      self.parameters.chunk_size,
    )?;
    let mut output_paths = Vec::new();
    for output in &self.parameters.outputs {
      output_paths.push(match output {
        OutputType::Drums => self.output_path_drums.clone(),
        OutputType::Bass => self.output_path_bass.clone(),
        OutputType::Other => self.output_path_other.clone(),
        OutputType::Vocals => self.output_path_vocals.clone(),
      });
    }
    log::info!("output paths: {output_paths:?}");
    let mut audio_out = AudioChunkWriter::new(
      &output_paths,
      self.codec.clone(),
      self.audio_options.clone(),
      audio.total_frames(),
      2,
      44100,
      audio.chunk_size(),
    )?;

    log::info!("loading model file from {:?}", self.model_path);
    let mut model = crate::libs::ort::session_with_model(&self.model_path)?;

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
      output_path_drums: Self::output_path_for(
        &self.codec,
        &self.parameters.outputs,
        &output_paths,
        OutputType::Drums,
      ),
      output_path_bass: Self::output_path_for(
        &self.codec,
        &self.parameters.outputs,
        &output_paths,
        OutputType::Bass,
      ),
      output_path_other: Self::output_path_for(
        &self.codec,
        &self.parameters.outputs,
        &output_paths,
        OutputType::Other,
      ),
      output_path_vocals: Self::output_path_for(
        &self.codec,
        &self.parameters.outputs,
        &output_paths,
        OutputType::Vocals,
      ),
    })))
  }
}
