//! 防止主题切换后组件保留默认配色，以及高饱和按钮文字不可读。
use super::*;

/// 把 sRGB 转成相对亮度，用于检查真实前景和背景的文字对比。
fn luminance(color: Hsla) -> f32 {
    let rgb = color.to_rgb();
    let linear = |channel: f32| {
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(rgb.r) + 0.7152 * linear(rgb.g) + 0.0722 * linear(rgb.b)
}

#[gpui_kit::test]
fn component_tokens_follow_palette_after_theme_round_trip(cx: &mut gpui_kit::TestAppContext) {
    cx.update(gpui_kit::component::init);
    for mode in [Mode::Dark, Mode::Light, Mode::Dark] {
        cx.update(|app| {
            apply_theme(mode, app);
            let theme = Theme::global(app);
            assert_eq!(theme.tokens.button_primary.color, theme.primary);
            assert_eq!(
                theme.tokens.button_primary_foreground.color,
                theme.primary_foreground
            );
            assert_eq!(theme.tokens.list_active.color, theme.list_active);
            assert_eq!(theme.tokens.table_hover.color, theme.table_hover);
            assert_eq!(theme.tokens.scrollbar_thumb.color, theme.scrollbar_thumb);
            assert_eq!(theme.tokens.warning.color, theme.warning);
            for (name, foreground, background) in [
                ("正文", theme.foreground, theme.background),
                ("辅助文字", theme.muted_foreground, theme.secondary),
                (
                    "主要按钮",
                    theme.button_primary_foreground,
                    theme.button_primary,
                ),
                (
                    "成功按钮",
                    theme.button_success_foreground,
                    theme.button_success,
                ),
                (
                    "警告按钮",
                    theme.button_warning_foreground,
                    theme.button_warning,
                ),
                (
                    "危险按钮",
                    theme.button_danger_foreground,
                    theme.button_danger,
                ),
            ] {
                let fg = luminance(foreground);
                let bg = luminance(background);
                let ratio = (fg.max(bg) + 0.05) / (fg.min(bg) + 0.05);
                assert!(ratio >= 4.5, "{mode:?} {name} 文字对比不足：{ratio}");
            }
        });
    }
}
