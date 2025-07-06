use binary_rw::{BinaryReader, SeekStream};

use crate::{error::Error, objects::file::File};

const MAGIC_NUMBER: u32 = 0x4650534B;
const VERSION: u8 = 1;

/// Importer for the Karaoke Studio format.
///
/// This format was used by the predecessor to ksng (what ksng is the "next generation" to).
/// It is very similar to the ksng format, for probably obvious reasons.
/// This importer does not support importing the Karaoke Studio config.
///
/// You can find Karaoke Studio here: https://github.com/azrogers/KaraokeStudio
pub struct KaraokeStudioFormat;

fn read_null_terminated_string(reader: &mut BinaryReader) -> Result<String, Error> {
  let mut bytes = Vec::new();
  loop {
    let byte = reader.read_u8()?;
    if byte == 0 {
      return Ok(String::from_utf8(bytes).map_err(|e| {
        format!("Failed to read null-terminated UTF-8 string: {e:?}");
      })?);
    }

    bytes.push(byte);
  }
}

impl KaraokeStudioFormat {
  pub fn import(reader: &mut BinaryReader) -> Result<File, Error> {
    let mut file = File::default();

    let magic = reader.read_u32()?;
    if magic != MAGIC_NUMBER {
      return Err(Error::Io(format!("Invalid KSF magic number {magic:x}")));
    }

    let version = reader.read_u8()?;
    if version != VERSION {
      return Err(Error::Io(format!(
        "Only KSF version 1 is supported, found version {version}"
      )));
    }

    // skip info byte
    reader.read_u8()?;

    let value_count = reader.read_i32()?;
    for _i in 0..value_count {
      let key = read_null_terminated_string(reader)?;
      if key == "_tracks" {}
    }

    Ok(file)
  }
}
