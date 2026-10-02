//! AssetSource：优先 ramag-ui 内嵌 svg（assets/icons），未命中回退 `gpui_kit::assets::Assets`

use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString};

/// 编译期内嵌 svg
#[derive(rust_embed::RustEmbed)]
#[folder = "assets"]
#[include = "icons/**/*.svg"]
struct LocalAssets;

#[derive(Default, Clone, Copy)]
pub struct RamagAssets;

struct MonitorFontLoaded;
impl gpui_kit::Global for MonitorFontLoaded {}

/// Registers the small bundled monitor heading font once per app, including isolated previews.
/// Its OFL notice is embedded alongside the font so packaged builds retain the license.
pub(crate) fn register_monitor_font(cx: &mut gpui_kit::App) {
    if cx.try_global::<MonitorFontLoaded>().is_some() {
        return;
    }
    let result = cx
        .text_system()
        .add_fonts(vec![Cow::Borrowed(include_bytes!(
            "../assets/Michroma-Regular.ttf"
        ))]);
    if let Err(error) = result {
        tracing::warn!(%error, "monitor heading font could not be loaded");
    }
    cx.set_global(MonitorFontLoaded);
}

impl AssetSource for RamagAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }
        if path == "fonts/Michroma-OFL.txt" {
            return Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/Michroma-OFL.txt"
            ))));
        }
        if let Some(file) = LocalAssets::get(path) {
            return Ok(Some(file.data));
        }
        // Markdown 预览会把仓库内相对图片解析为绝对路径；GPUI 的 `img` 将其作为
        // Embedded 资源交给 AssetSource，因此这里补充本地文件回退。
        if std::path::Path::new(path).is_absolute()
            && let Ok(data) = std::fs::read(path)
        {
            return Ok(Some(Cow::Owned(data)));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut out: Vec<SharedString> = LocalAssets::iter()
            .filter_map(|p| p.starts_with(path).then(|| p.into()))
            .collect();
        if let Ok(upstream) = gpui_kit::assets::Assets.list(path) {
            out.extend(upstream);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_absolute_files_for_markdown_images() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/screenshots/v0.0.5/settings-system-tray-windows.png");
        let loaded = RamagAssets
            .load(path.to_string_lossy().as_ref())
            .ok()
            .flatten();

        assert!(loaded.is_some());
    }

    #[test]
    fn embeds_result_pagination_icons_used_by_upstream_icon_names() {
        for path in ["icons/skip-back.svg", "icons/skip-forward.svg"] {
            let loaded = RamagAssets.load(path).ok().flatten();
            assert!(
                loaded.is_some(),
                "pagination asset should be embedded: {path}"
            );
        }
    }

    #[test]
    fn embeds_all_builtin_tool_icons() {
        for path in [
            "icons/database.svg",
            "icons/git-branch.svg",
            "icons/clipboard.svg",
            "icons/terminal.svg",
            "icons/gauge.svg",
            "icons/container.svg",
            "icons/kafka.svg",
            "icons/mqtt.svg",
            "icons/api.svg",
            "icons/json.svg",
            "icons/cloud.svg",
            "icons/users.svg",
            "icons/plugin.svg",
            "icons/globe.svg",
            "icons/hash.svg",
            "icons/toolbox.svg",
        ] {
            assert!(
                RamagAssets.load(path).ok().flatten().is_some(),
                "missing asset: {path}"
            );
        }
    }
}
