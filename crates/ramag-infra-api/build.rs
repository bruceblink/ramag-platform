use std::{env, error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    // 预研使用同一份 .proto 同时生成测试服务和 DescriptorSet，验证运行时动态消息路径。
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").ok_or("OUT_DIR must be set by Cargo")?);
    println!("cargo:rerun-if-changed=proto/api_spike.proto");

    tonic_prost_build::configure()
        .build_client(false)
        .build_server(true)
        .file_descriptor_set_path(out_dir.join("api_spike.bin"))
        .compile_protos(&["proto/api_spike.proto"], &["proto"])?;

    Ok(())
}
