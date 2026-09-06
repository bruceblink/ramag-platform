fn main() {
    // librdkafka's Windows SSPI implementation calls secur32 APIs when the
    // GNU linker builds Kafka tests and the desktop binary. Keep this as a
    // trailing linker argument so it resolves symbols from librdkafka's
    // static archive after Cargo has listed that archive.
    if cfg!(target_os = "windows") {
        println!("cargo:rustc-link-arg=-lsecur32");
    }
}
