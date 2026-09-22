use std::{env, error::Error, path::PathBuf};

use protox::prost::Message;

fn main() -> Result<(), Box<dyn Error>> {
    // 预研使用同一份 .proto 同时生成测试服务和 DescriptorSet，验证运行时动态消息路径。
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").ok_or("OUT_DIR must be set by Cargo")?);
    println!("cargo:rerun-if-changed=proto/api_spike.proto");

    let descriptor_set = protox::Compiler::new(["proto"])?
        .include_imports(true)
        .include_source_info(false)
        .open_file("api_spike.proto")?
        .file_descriptor_set();
    std::fs::write(
        out_dir.join("api_spike.bin"),
        descriptor_set.encode_to_vec(),
    )?;

    tonic_prost_build::configure()
        .build_client(false)
        .build_server(true)
        .compile_fds(descriptor_set)?;

    Ok(())
}
