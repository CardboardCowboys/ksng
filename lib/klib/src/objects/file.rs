use binary_rw::{BinaryReader, BinaryWriter, Endian, FileStream, ReadStream, WriteStream};

use crate::{config::Config, error::Error, objects::track::Track};

const MAGIC_NUMBER: u32 = 0x474E534B;
const FILE_VERSION: u16 = 0;

#[derive(Default)]
pub struct File {
  pub config: Config,
  pub metadata: serde_json::Value,
  pub tracks: Vec<Track>,
}

impl File {
  pub fn write(&self, stream: &mut impl WriteStream) -> Result<(), Error> {
    let mut writer = BinaryWriter::new(stream, Endian::Little);
    writer.write_u32(MAGIC_NUMBER)?;
    writer.write_u16(FILE_VERSION)?;

    writer.write_string(serde_json::to_string(&self.config)?)?;
    writer.write_string(serde_json::to_string(&self.metadata)?)?;

    writer.write_usize(self.tracks.len())?;
    for track in &self.tracks {
      track.write(&mut writer)?;
    }

    Ok(())
  }

  pub fn write_to_file(&self, file: std::fs::File) -> Result<(), Error> {
    let mut stream = FileStream::new(file);
    self.write(&mut stream)
  }

  pub fn read(stream: &mut impl ReadStream) -> Result<File, Error> {
    let mut file = File::default();
    let mut reader = BinaryReader::new(stream, Endian::Little);

    let read_magic = reader.read_u32()?;
    if read_magic != MAGIC_NUMBER {
      return Err(Error::Unsupported(format!("Magic number {read_magic:x}")));
    }

    let read_version = reader.read_u16()?;
    if read_version > FILE_VERSION {
      return Err(Error::Unsupported(format!(
        "KSNG file version {read_version} not supported, only {FILE_VERSION} and lower."
      )));
    }

    file.config = serde_json::from_str(&reader.read_string()?)?;
    file.metadata = serde_json::from_str(&reader.read_string()?)?;

    let len = reader.read_usize()?;
    file.tracks.reserve(len);
    for _i in 0..len {
      let track = Track::read(&mut reader)?;
      file.tracks.push(track);
    }

    Ok(file)
  }

  pub fn read_from_file(file: std::fs::File) -> Result<File, Error> {
    let mut stream = FileStream::new(file);
    File::read(&mut stream)
  }
}
