//! 防止主题切换后组件保留默认配色，以及高饱和按钮文字不可读。
use super::*;

/// 把 sRGB 转成相对亮度，用于检查真实前景和背景的文字对比。
fn luminance(color: Hsla) -> f32 {
    let rgb = color.to_rgb();
    luminance_rgb([rgb.r, rgb.g, rgb.b])
}

fn luminance_rgb(channels: [f32; 3]) -> f32 {
    let linear = |channel: f32| {
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(channels[0]) + 0.7152 * linear(channels[1]) + 0.0722 * linear(channels[2])
}

/// 先把半透明前景画到实际底色上，再计算文字对比度。
fn contrast_ratio(foreground: Hsla, background: Hsla) -> f32 {
    let fg = foreground.to_rgb();
    let bg = background.to_rgb();
    let alpha = foreground.a;
    let visible_foreground = [
        fg.r * alpha + bg.r * (1.0 - alpha),
        fg.g * alpha + bg.g * (1.0 - alpha),
        fg.b * alpha + bg.b * (1.0 - alpha),
    ];
    let fg_luminance = luminance_rgb(visible_foreground);
    let bg_luminance = luminance(background);
    (fg_luminance.max(bg_luminance) + 0.05) / (fg_luminance.min(bg_luminance) + 0.05)
}

/// 把半透明按钮背景叠到卡片或弹层底色上，再检查不透明按钮文字。
fn contrast_ratio_over_backdrop(
    foreground: Hsla,
    translucent_background: Hsla,
    backdrop: Hsla,
) -> f32 {
    let layer = translucent_background.to_rgb();
    let underneath = backdrop.to_rgb();
    let alpha = translucent_background.a;
    let visible_background = luminance_rgb([
        layer.r * alpha + underneath.r * (1.0 - alpha),
        layer.g * alpha + underneath.g * (1.0 - alpha),
        layer.b * alpha + underneath.b * (1.0 - alpha),
    ]);
    let foreground_luminance = luminance(foreground);
    (foreground_luminance.max(visible_background) + 0.05)
        / (foreground_luminance.min(visible_background) + 0.05)
}

#[gpui_kit::test]
fn component_tokens_follow_palette_after_theme_round_trip(cx: &mut gpui_kit::TestAppContext) {
    cx.update(gpui_kit::component::init);
    for mode in [Mode::Dark, Mode::Light, Mode::Dark] {
        cx.update(|app| {
            apply_theme(mode, app);
            let theme = Theme::global(app);
            assert_eq!(theme.tokens.button_primary.color, theme.button_primary);
            assert_eq!(
                theme.tokens.button_primary_foreground.color,
                theme.button_primary_foreground
            );
            assert_eq!(theme.tokens.list_active.color, theme.list_active);
            assert_eq!(theme.tokens.table_hover.color, theme.table_hover);
            assert_eq!(theme.tokens.scrollbar_thumb.color, theme.scrollbar_thumb);
            assert_eq!(theme.tokens.warning.color, theme.warning);
            assert_eq!(theme.button_primary, theme.foreground);
            assert_eq!(theme.button_primary_foreground, theme.primary_foreground);
            assert_eq!(theme.button_primary, theme.primary);
            assert_eq!(theme.ring, theme.accent);
            assert_eq!(theme.table_row_border, theme.border);
            assert_eq!(theme.table_row_border.a, 1.0);
            let table_line_ratio = contrast_ratio(theme.table_row_border, theme.secondary);
            assert!(
                table_line_ratio >= 1.2,
                "{mode:?} 表格横线与表面差异不足：{table_line_ratio}"
            );

            for background in [theme.background, theme.secondary, theme.popover] {
                for (name, foreground) in [
                    ("正文", theme.foreground),
                    ("辅助文字", theme.muted_foreground),
                ] {
                    let ratio = contrast_ratio(foreground, background);
                    assert!(
                        ratio >= 4.5,
                        "{mode:?} {name} 在 {background:?} 上的对比不足：{ratio}"
                    );
                }

                for (name, color) in [
                    ("成功状态", theme.success),
                    ("警告状态", theme.warning),
                    ("不可用状态", theme.muted_foreground),
                    ("失败状态", theme.danger),
                ] {
                    let ratio =
                        contrast_ratio_over_backdrop(color, color.opacity(0.12), background);
                    assert!(
                        ratio >= 4.5,
                        "{mode:?} {name} 在 {background:?} 上的对比不足：{ratio}"
                    );
                }
            }

            for (name, foreground, button_background, backdrop) in [
                (
                    "主要按钮",
                    theme.button_primary_foreground,
                    theme.button_primary,
                    theme.background,
                ),
                (
                    "主要按钮悬停",
                    theme.button_primary_foreground,
                    theme.button_primary_hover,
                    theme.secondary,
                ),
                (
                    "主要按钮按下",
                    theme.button_primary_foreground,
                    theme.button_primary_active,
                    theme.popover,
                ),
            ] {
                let ratio = contrast_ratio_over_backdrop(foreground, button_background, backdrop);
                assert!(ratio >= 4.5, "{mode:?} {name} 文字对比不足：{ratio}");
            }

            for (name, foreground, colors) in [
                (
                    "成功按钮",
                    theme.button_success_foreground,
                    [
                        theme.button_success,
                        theme.button_success_hover,
                        theme.button_success_active,
                    ],
                ),
                (
                    "警告按钮",
                    theme.button_warning_foreground,
                    [
                        theme.button_warning,
                        theme.button_warning_hover,
                        theme.button_warning_active,
                    ],
                ),
                (
                    "危险按钮",
                    theme.button_danger_foreground,
                    [
                        theme.button_danger,
                        theme.button_danger_hover,
                        theme.button_danger_active,
                    ],
                ),
            ] {
                for (state, button_background) in [
                    ("默认", colors[0]),
                    ("悬停", colors[1]),
                    ("按下", colors[2]),
                ] {
                    for backdrop in [theme.background, theme.secondary, theme.popover] {
                        let ratio =
                            contrast_ratio_over_backdrop(foreground, button_background, backdrop);
                        assert!(
                            ratio >= 4.5,
                            "{mode:?} {name}{state}在{backdrop:?}上文字对比不足：{ratio}"
                        );
                    }
                }
            }
        });
    }
}

#[gpui_kit::test]
fn neutral_surface_tokens_match_the_prototype_in_both_modes(cx: &mut gpui_kit::TestAppContext) {
    cx.update(gpui_kit::component::init);
    for (mode, expected) in [
        (
            Mode::Light,
            [0xffffff, 0xfafafa, 0xf4f4f4, 0x171717, 0x656565],
        ),
        (
            Mode::Dark,
            [0x090909, 0x101010, 0x171717, 0xededed, 0xa1a1a1],
        ),
    ] {
        cx.update(|app| {
            apply_theme(mode, app);
            let theme = Theme::global(app);
            let palette = crate::pulse_ui::pulse_palette(app);
            assert_eq!(palette.background, rgb(expected[0]).into());
            assert_eq!(palette.surface, rgb(expected[1]).into());
            assert_eq!(palette.raised, rgb(expected[2]).into());
            assert_eq!(palette.text, rgb(expected[3]).into());
            assert_eq!(palette.muted, rgb(expected[4]).into());
            let (border_rgb, accent, primary, hover, selected) = if mode == Mode::Light {
                (0xe4e4e4, 0x175cbb, 0x171717, 0xeeeeee, 0xf4f4f4)
            } else {
                (0x282828, 0x82b9ff, 0xededed, 0x202020, 0x171717)
            };
            let mut primary_hover: Hsla = rgb(primary).into();
            primary_hover.a = 0.88;
            let (success, warning, danger) = if mode == Mode::Light {
                (0x176f46, 0x885913, 0xb92525)
            } else {
                (0x74d5a2, 0xe9bf75, 0xff9b9b)
            };
            let border = palette.border.to_rgb();
            let expected_border = rgb(border_rgb);
            assert_eq!(
                [border.r, border.g, border.b],
                [expected_border.r, expected_border.g, expected_border.b]
            );
            assert_eq!(palette.border.a, 1.0);
            assert_eq!(palette.selected, rgb(selected).into());
            assert_eq!(theme.primary, rgb(primary).into());
            assert_eq!(theme.accent, rgb(accent).into());
            assert_eq!(theme.ring, theme.accent);
            assert_eq!(theme.input, palette.background);
            assert_eq!(theme.popover, palette.surface);
            assert_eq!(theme.muted, rgb(hover).into());
            assert_eq!(theme.list_active, palette.raised);
            assert_eq!(theme.list_active_border, palette.border);
            assert_eq!(theme.button_primary, palette.text);
            assert_eq!(theme.success, rgb(success).into());
            assert_eq!(theme.warning, rgb(warning).into());
            assert_eq!(theme.danger, rgb(danger).into());
            assert_eq!(theme.button_primary_hover, primary_hover);
            assert_eq!(theme.button_primary_active, theme.button_primary_hover);
            assert_eq!(
                theme.button_primary_foreground,
                rgb(if mode == Mode::Light {
                    0xffffff
                } else {
                    0x111111
                })
                .into()
            );
        });
    }
}

#[gpui_kit::test]
fn shared_error_text_color_follows_the_active_theme(cx: &mut gpui_kit::TestAppContext) {
    cx.update(gpui_kit::component::init);
    for mode in [Mode::Light, Mode::Dark, Mode::Light] {
        cx.update(|app| {
            apply_theme(mode, app);
            let theme = Theme::global(app);
            assert_eq!(crate::error_text_color(theme), theme.danger);
        });
    }
}
