use std::{path::Path, sync::Once};

use ffmpeg_next::{ChannelLayout, Dictionary, Packet, Rational, codec, encoder, format, frame};

use crate::{
  Error, PlanarAudioBuffer,
  chain::AudioInfo,
  encoders::{AudioCodec, AudioEncoder, AudioEncoderOptions},
};

static FFMPEG_INIT: Once = Once::new();

pub struct FfmpegAudioEncoder {
  sample_conv: ffmpeg_next::software::resampling::context::Context,
  encoder: ffmpeg_next::encoder::audio::Encoder,
  output: ffmpeg_next::format::context::Output,
  in_frame: ffmpeg_next::frame::audio::Audio,
  out_frame: ffmpeg_next::frame::audio::Audio,
  is_same: bool,
  waiting_frames: usize,
  frame_size: usize,
  finalized: bool,
  samples_written: usize,
}

impl FfmpegAudioEncoder {
  /// Creates a new encoder with the given options and output path.
  pub fn new<P: AsRef<Path>>(
    output: P,
    audio_codec: AudioCodec,
    options: AudioEncoderOptions,
    info: AudioInfo,
  ) -> Result<FfmpegAudioEncoder, Error> {
    FFMPEG_INIT.call_once(|| {
      ffmpeg_next::init().unwrap();
    });

    let output_path = audio_codec.set_extension(output.as_ref());

    let mut output = format::output(&output_path)?;
    let codec = match &audio_codec {
      AudioCodec::Mp3 => encoder::find(codec::Id::MP3),
      AudioCodec::Aac => encoder::find(codec::Id::AAC),
      AudioCodec::Wav => encoder::find(codec::Id::PCM_F32LE),
      _ => {
        return Err(Error::msg(format!(
          "FfmpegAudioEncoder does not support {:?} codec",
          audio_codec,
        )));
      }
    };

    let Some(codec) = codec else {
      return Err(Error::msg(format!(
        "Could not find FFmpeg encoder for {:?}",
        audio_codec
      )));
    };

    let mut sample_rate = info.sample_rate as i32;
    if let Some(iter) = codec.audio()?.rates() {
      let mut rates: Vec<i32> = iter.collect();
      sample_rate = Self::select_sample_rate(&mut rates, info.sample_rate as i32);
    }

    let mut format = format::Sample::F32(format::sample::Type::Planar);
    if let Some(iter) = codec.audio()?.formats() {
      for f in iter {
        if matches!(f, format::Sample::F32(..)) {
          format = f;
          break;
        }

        if matches!(f, format::Sample::I16(..)) {
          format = f;
        }
      }
    }

    let layout = ChannelLayout::default(info.num_channels as i32);
    let mut ost = output.add_stream(codec)?;
    let mut encoder = codec::context::Context::new_with_codec(codec)
      .encoder()
      .audio()?;
    encoder.set_format(format);
    encoder.set_channel_layout(layout);
    encoder.set_rate(sample_rate);
    encoder.set_time_base(Rational::new(1, sample_rate));
    if !matches!(audio_codec, AudioCodec::Wav) {
      encoder.set_bit_rate(options.bit_rate);
    }
    ost.set_parameters(&encoder);

    let opts = Self::parse_opts(&options.options)
      .ok_or(Error::msg("Could not parse options for FFmpeg encoder"))?;
    let encoder = encoder.open_with(opts)?;

    let frame_size = if encoder.frame_size() > 0 {
      encoder.frame_size() as usize
    } else {
      1024
    };
    log::info!("frame_size: {frame_size}");

    let sample_conv = ffmpeg_next::software::resampler(
      (
        format::Sample::F32(format::sample::Type::Planar),
        layout,
        info.sample_rate as u32,
      ),
      (encoder.format(), layout, sample_rate as u32),
    )?;

    let mut in_frame = frame::audio::Audio::new(
      format::Sample::F32(format::sample::Type::Planar),
      frame_size,
      layout,
    );
    in_frame.set_rate(info.sample_rate as u32);

    let mut out_frame = frame::audio::Audio::new(encoder.format(), frame_size, layout);
    out_frame.set_rate(sample_rate as u32);

    let is_same =
      in_frame.format() == out_frame.format() && info.sample_rate == sample_rate as usize;

    output.write_header()?;

    Ok(FfmpegAudioEncoder {
      sample_conv,
      encoder,
      output,
      in_frame,
      out_frame,
      waiting_frames: 0,
      frame_size,
      is_same,
      finalized: false,
      samples_written: 0,
    })
  }

  fn send_frame(&mut self) -> Result<(), Error> {
    self.out_frame.set_pts(Some(self.samples_written as i64));
    self.encoder.send_frame(&self.out_frame)?;
    self.samples_written += self.out_frame.samples();
    let mut packet = Packet::empty();
    while self.encoder.receive_packet(&mut packet).is_ok() {
      packet.set_stream(0);
      packet.write_interleaved(&mut self.output)?;
    }

    Ok(())
  }

  fn parse_opts<'a>(s: &str) -> Option<Dictionary<'a>> {
    let mut dict = Dictionary::new();
    for keyval in s.split_terminator(',') {
      let tokens: Vec<&str> = keyval.split('=').collect();
      match tokens[..] {
        [key, val] => dict.set(key, val),
        _ => return None,
      }
    }
    Some(dict)
  }

  fn select_sample_rate(rates: &mut [i32], target_rate: i32) -> i32 {
    if rates.contains(&target_rate) {
      return target_rate;
    }

    rates.sort();
    for r in rates.iter() {
      if *r > target_rate {
        return *r;
      }
    }

    rates.sort_by_key(|s| i32::max(target_rate, *s) - i32::min(target_rate, *s));
    if let Some(rate) = rates.first() {
      return *rate;
    }

    target_rate
  }
}

impl AudioEncoder for FfmpegAudioEncoder {
  fn write(&mut self, buffer: &dyn PlanarAudioBuffer) -> Result<(), Error> {
    if self.finalized {
      return Err(Error::msg("Trying to write frames to a finalized encoder"));
    }

    let mut num_read = 0;
    loop {
      let num_expected = self.frame_size - self.waiting_frames;
      let num = num_expected.min(buffer.num_frames() - num_read);
      self
        .in_frame
        .copy_from_buffer(buffer, num_read, self.waiting_frames, num);
      self.waiting_frames += num;
      num_read += num;

      if num < num_expected {
        // Not enough to fill the frame - come back to it next call.
        break;
      }

      if self.is_same {
        self
          .out_frame
          .copy_from_buffer(&self.in_frame, 0, 0, self.in_frame.num_frames());
      } else {
        self.out_frame.set_samples(self.frame_size);
        self.sample_conv.run(&self.in_frame, &mut self.out_frame)?;
      }

      self.send_frame()?;

      self.waiting_frames = 0;
    }

    Ok(())
  }

  /// Finalizes the encoder, writing any remaining frames and closing the
  /// stream.
  fn finalize(&mut self) -> Result<(), Error> {
    if self.waiting_frames > 0 {
      self.in_frame.set_samples(self.waiting_frames);
      if self.is_same {
        self
          .out_frame
          .copy_from_buffer(&self.in_frame, 0, 0, self.in_frame.num_frames());
        self.out_frame.set_samples(self.waiting_frames);
      } else {
        self.out_frame.set_samples(self.frame_size);
        self.sample_conv.run(&self.in_frame, &mut self.out_frame)?;
      }

      if self.out_frame.samples() > 0 {
        self.send_frame()?;
      }

      while let Some(_delay) = self.sample_conv.flush(&mut self.out_frame)? {
        if self.out_frame.samples() > 0 {
          self.send_frame()?;
        } else {
          break;
        }
      }
    }

    self.encoder.send_eof()?;
    let mut packet = Packet::empty();
    while self.encoder.receive_packet(&mut packet).is_ok() {
      packet.set_stream(0);
      packet.write_interleaved(&mut self.output)?;
    }
    self.output.write_trailer()?;

    self.finalized = true;

    Ok(())
  }
}
