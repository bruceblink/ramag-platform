fn main() {
    // librdkafka's Windows SSPI implementation calls secur32 APIs. Ask Cargo
    // for the platform library so Rust emits the correct MSVC library form
    // instead of passing the GNU-only `-l` spelling to link.exe.
    if cfg!(target_os = "windows") {
        println!("cargo:rustc-link-lib=dylib=secur32");
    }
}
