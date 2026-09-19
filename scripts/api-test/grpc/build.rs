use std::{env, error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").ok_or("OUT_DIR must be set")?);
    println!("cargo:rerun-if-changed=proto/api_docker.proto");
    tonic_prost_build::configure()
        .build_client(false)
        .build_server(true)
        .file_descriptor_set_path(out_dir.join("api_docker.bin"))
        .compile_protos(&["proto/api_docker.proto"], &["proto"])?;
    Ok(())
}
