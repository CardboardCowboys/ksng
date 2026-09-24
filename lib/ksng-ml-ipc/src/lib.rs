#![feature(bufreader_peek)]
#![feature(read_le)]

use std::io::{Read, Write};

const PACKET_MAGIC: u32 = 0x544B4350;

pub mod packet {
  include! { concat!(env!("OUT_DIR"), "/proto.packet.rs") }
}

impl From<uuid::Uuid> for packet::Uuid {
  fn from(value: uuid::Uuid) -> Self {
    let (hi, lo) = value.as_u64_pair();
    packet::Uuid { hi, lo }
  }
}

impl From<packet::Uuid> for uuid::Uuid {
  fn from(value: packet::Uuid) -> Self {
    uuid::Uuid::from_u64_pair(value.hi, value.lo)
  }
}

/// Reads the next packet from the stream asynchronously.
#[cfg(feature = "tokio")]
pub async fn read_next_packet<T>(
  reader: &mut tokio::io::BufReader<&mut interprocess::local_socket::tokio::RecvHalf>,
) -> Result<T, anyhow::Error>
where
  T: prost::Message + Default,
{
  use prost::bytes::BytesMut;
  use tokio::io::AsyncReadExt;

  let magic = reader.read_u32().await?;
  if magic != PACKET_MAGIC {
    return Err(anyhow::Error::msg(format!(
      "Expected magic value {PACKET_MAGIC}, got {magic}"
    )));
  }

  let len = reader.read_u32().await?;
  let mut buf = BytesMut::with_capacity(len as usize);
  reader.read_buf(&mut buf).await?;
  Ok(T::decode(&mut buf)?)
}

/// Reads the next packet from the stream if one is waiting, otherwise returns
/// None.
pub fn read_next_packet_sync<T>(
  reader: &mut std::io::BufReader<interprocess::local_socket::RecvHalf>,
) -> Result<Option<T>, anyhow::Error>
where
  T: prost::Message + Default,
{
  let magic_size = size_of::<u32>();
  let magic = reader.peek(magic_size)?;
  if magic.len() < magic_size {
    return Ok(None);
  }

  let magic = u32::from_le_bytes([magic[0], magic[1], magic[2], magic[3]]);
  if magic != PACKET_MAGIC {
    return Err(anyhow::Error::msg(format!(
      "Expected magic value {PACKET_MAGIC}, got {magic}"
    )));
  }

  let len = reader.read_le::<u32>()?;
  let mut buf = vec![0; len as usize];
  reader.read_exact(&mut buf)?;

  Ok(Some(T::decode(buf.as_slice())?))
}

/// Writes the given packet to the stream asynchronously.
#[cfg(feature = "tokio")]
pub async fn write_packet(
  packet: impl prost::Message,
  writer: &mut interprocess::local_socket::tokio::SendHalf,
) -> Result<(), anyhow::Error> {
  use tokio::io::AsyncWriteExt;

  writer.write_u32(PACKET_MAGIC).await?;

  let bytes = packet.encode_to_vec();
  writer.write_u32(bytes.len() as u32).await?;
  writer.write_all(&bytes).await?;

  Ok(())
}

/// Writes the given packet to the stream.
pub fn write_packet_sync(
  packet: impl prost::Message,
  writer: &mut interprocess::local_socket::SendHalf,
) -> Result<(), anyhow::Error> {
  writer.write_all(&PACKET_MAGIC.to_le_bytes())?;

  let bytes = packet.encode_to_vec();
  writer.write_all(&(bytes.len() as u32).to_le_bytes())?;
  writer.write_all(&bytes)?;

  Ok(())
}
