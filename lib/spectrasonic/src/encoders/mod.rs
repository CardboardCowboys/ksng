pub mod ffmpeg;

#[derive(Debug)]
pub enum AudioCodec {
  Mp3,
  Aac,
  Vorbis,
  Opus,
  Wav,
  Flac,
}
