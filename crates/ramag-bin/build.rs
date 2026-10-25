//! 构建脚本：Windows 下嵌入 ramag.ico 并设置 GUI 主线程栈空间。
//! 其它平台（macOS 走 .icns bundle）不应用 Windows 专属资源和链接参数。

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=../../scripts/icons/ramag.ico");
    println!("cargo:rerun-if-env-changed=RAMAG_PACKAGE_PUBLISHER");
    // build.rs 的 cfg 反映宿主，判目标平台须用 CARGO_CFG_TARGET_OS 环境变量
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // GPUI 数据库结果视图的布局和绘制调用链会超过默认主线程栈；4 MiB 已通过原生结果集浏览验证。
        match std::env::var("CARGO_CFG_TARGET_ENV").as_deref() {
            Ok("msvc") => println!("cargo:rustc-link-arg-bin=ramag=/STACK:4194304"),
            Ok("gnu") => println!("cargo:rustc-link-arg-bin=ramag=-Wl,--stack,4194304"),
            _ => {}
        }

        // CI supplies the repository owner; local builds fall back to Cargo package metadata.
        let publisher = std::env::var("RAMAG_PACKAGE_PUBLISHER")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| std::env::var("CARGO_PKG_AUTHORS").ok())
            .unwrap_or_else(|| "Ramag contributors".into());
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../scripts/icons/ramag.ico")
            .set("ProductName", "Ramag")
            .set("FileDescription", "Ramag Developer Toolbox")
            .set("InternalName", "ramag")
            .set("OriginalFilename", "ramag.exe")
            .set("CompanyName", &publisher)
            .set("LegalCopyright", "Copyright (c) Ramag contributors");
        res.compile()?;
    }
    Ok(())
}
