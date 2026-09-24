use std::{io::Read, path::PathBuf};

use binary_rw::{BinaryReader, BinaryWriter};
use uuid::Uuid;

use crate::error::Error;

/// The source of the attachment
pub enum AttachmentSource {
  /// This attachment can be loaded from a file at the given relative path to
  /// the project file.
  File(PathBuf),
  /// The location of the attachment file or data is managed by the host. This
  /// is not portable.
  Managed,
}

pub struct Attachment {
  pub id: Uuid,
  pub name: String,
  pub mime_type: String,
  pub size: usize,
  pub source: AttachmentSource,
}

impl Attachment {
  pub fn read(reader: &mut BinaryReader) -> Result<Attachment, Error> {
    let (hi, lo) = (reader.read_u64()?, reader.read_u64()?);
    let id = Uuid::from_u64_pair(hi, lo);
    let name = reader.read_string()?;
    let mime = reader.read_string()?;
    let size = reader.read_u64()?;
    let source = match reader.read_u8()? {
      0 => AttachmentSource::File(PathBuf::from(reader.read_string()?)),
      1 => AttachmentSource::Managed,
      idx => return Err(Error::Io(format!("Unknown attachment source ID {idx}"))),
    };

    Ok(Attachment {
      id,
      name,
      mime_type: mime,
      size: size as usize,
      source,
    })
  }

  pub fn write(&self, writer: &mut BinaryWriter) -> Result<(), Error> {
    let (hi, lo) = self.id.as_u64_pair();
    writer.write_u64(hi)?;
    writer.write_u64(lo)?;
    writer.write_string(&self.name)?;
    writer.write_string(&self.mime_type)?;
    writer.write_u64(self.size as u64)?;
    match &self.source {
      AttachmentSource::File(path) => {
        writer.write_u8(0)?;
        let s = path
          .to_str()
          .map(|s| s.to_string())
          .ok_or(Error::Io(format!("Can't format path {path:?} as string")))?;
        writer.write_string(s)?;
      }
      AttachmentSource::Managed => {
        writer.write_u8(1)?;
      }
    }

    Ok(())
  }
}

pub enum AttachmentReader {
  Path(PathBuf),
  Stream(Box<dyn Read>),
}

pub trait AttachmentResolver {
  fn read(&self, attachment: &Attachment) -> AttachmentReader;
}
