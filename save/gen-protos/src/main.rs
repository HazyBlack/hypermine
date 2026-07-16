use std::{error::Error, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("src");

    prost_build::Config::new()
        .protoc_executable(protoc_bin_vendored::protoc_bin_path()?)
        .out_dir(&dir)
        .compile_protos(&[dir.join("protos.proto")], &[dir])?;
    Ok(())
}
