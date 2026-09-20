fn main() -> std::io::Result<()> {
  println!("cargo:rerun-if-changed=proto/packet.proto");
  prost_build::compile_protos(&["proto/packet.proto"], &["proto/"])?;
  Ok(())
}
