//! JetBrains 风格暗 / 亮主题。`init_theme` 启动时初始化，主题按钮支持两态切换。

use std::sync::Arc;

use gpui_kit::component::{Theme, ThemeMode, highlighter::HighlightTheme};
use gpui_kit::{App, Global, Hsla, Pixels, hsla, px, rgb};
use ramag_domain::traits::Storage;

/// 让 UI 层切主题时访问 Storage 做持久化
pub struct StorageGlobal(pub Arc<dyn Storage>);
impl Global for StorageGlobal {}

/// main 可能没注入，None 时不持久化
pub fn storage_from_cx(cx: &App) -> Option<Arc<dyn Storage>> {
    cx.try_global::<StorageGlobal>().map(|g| g.0.clone())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Dark,
    Light,
}

/// Keep both scrollbar axes compact while preserving a usable drag target.
const SCROLLBAR_TRACK_WIDTH: Pixels = px(10.0);
const SCROLLBAR_THUMB_WIDTH: Pixels = px(6.0);
const SCROLLBAR_THUMB_HOVER_WIDTH: Pixels = px(7.0);
const SCROLLBAR_THUMB_ACTIVE_WIDTH: Pixels = px(8.0);
const SCROLLBAR_THUMB_INSET: Pixels = px(0.0);
const SCROLLBAR_THUMB_RADIUS: Pixels = px(3.0);

/// preference "dark" 用暗色；其余（缺省 / 旧版 "system" 残值）一律默认浅色
pub fn init_theme(preference: Option<&str>, cx: &mut App) {
    let mode = match preference {
        Some("dark") => Mode::Dark,
        _ => Mode::Light,
    };
    apply_theme(mode, cx);
}

pub fn apply_theme(mode: Mode, cx: &mut App) {
    crate::assets::register_monitor_font(cx);
    match mode {
        Mode::Dark => {
            Theme::change(ThemeMode::Dark, None, cx);
            apply_dark_palette(Theme::global_mut(cx));
        }
        Mode::Light => {
            Theme::change(ThemeMode::Light, None, cx);
            apply_light_palette(Theme::global_mut(cx));
        }
    }
    normalize_component_palette(Theme::global_mut(cx));
    // 命令编辑器背景对齐主背景：gpui 默认 editor.background 是纯黑，浮在主背景上显突兀
    normalize_editor_highlight_theme(mode, cx);
    // 结果表和其它需要明确可发现性的内容区域会自行挂载滚动条；统一恢复可见轨道和滑块。
    configure_scrollbar_paint(Theme::global_mut(cx));
    // gpui-component 的按钮读已解析令牌，Base 滚动条读独立主题副本；
    // 修改 colors 后必须同时同步，避免出现默认白按钮和旧色滑块。
    let theme = Theme::global_mut(cx);
    theme.tokens = (&theme.colors).into();
    crate::system_settings::apply_display_settings(cx);
}

/// 把原型的中性表面、选中状态和组件令牌映射到 GPUI 主题。
/// 工具选中背景使用 raised；键盘焦点继续使用独立的蓝色 ring。
fn normalize_component_palette(theme: &mut Theme) {
    theme.radius = gpui_kit::px(6.0);
    theme.radius_lg = gpui_kit::px(8.0);
    theme.caret = theme.foreground;
    theme.secondary_hover = theme.muted;
    theme.secondary_active = theme.list_active;
    theme.ring = theme.accent;
    theme.accordion = theme.secondary;
    theme.sidebar_foreground = theme.foreground;
    theme.sidebar_border = theme.border;
    theme.sidebar_accent = theme.list_active;
    theme.sidebar_accent_foreground = theme.foreground;
    theme.sidebar_primary = theme.primary;
    theme.sidebar_primary_foreground = theme.primary_foreground;
    theme.tab = theme.secondary;
    theme.tab_bar = theme.secondary;
    theme.tab_bar_segmented = theme.muted;
    theme.tab_active = theme.list_active;
    theme.tab_foreground = theme.muted_foreground;
    theme.tab_active_foreground = theme.foreground;
    theme.colors.list = theme.background;
    theme.list_even = theme.secondary;
    theme.list_head = theme.secondary;
    theme.table = theme.background;
    theme.table_even = theme.secondary;
    theme.table_head = theme.secondary;
    theme.table_head_foreground = theme.foreground;
    theme.table_foot = theme.secondary;
    theme.table_foot_foreground = theme.muted_foreground;
    theme.table_hover = theme.list_hover;
    theme.table_active = theme.list_active;
    theme.table_active_border = theme.list_active_border;
    theme.table_row_border = theme.border;
    normalize_button_palette(theme);
}

/// 主按钮使用原型的中性色；状态按钮保留各自的状态色。
fn normalize_button_palette(theme: &mut Theme) {
    theme.button = theme.secondary;
    theme.button_hover = theme.muted;
    theme.button_active = theme.list_active;
    theme.button_foreground = theme.foreground;
    theme.button_primary = theme.primary;
    theme.button_primary_hover = theme.primary_hover;
    theme.button_primary_active = theme.primary_active;
    theme.button_primary_foreground = theme.primary_foreground;
    theme.button_secondary = theme.secondary;
    theme.button_secondary_hover = theme.secondary_hover;
    theme.button_secondary_active = theme.secondary_active;
    theme.button_secondary_foreground = theme.secondary_foreground;
    theme.button_danger = theme.danger;
    theme.button_danger_hover = theme.danger_hover;
    theme.button_danger_active = theme.danger_active;
    theme.button_danger_foreground = theme.danger_foreground;
    theme.button_success = theme.success;
    theme.button_success_hover = theme.success_hover;
    theme.button_success_active = theme.success_active;
    theme.button_success_foreground = theme.success_foreground;
    theme.button_warning = theme.warning;
    theme.button_warning_hover = theme.warning_hover;
    theme.button_warning_active = theme.warning_active;
    theme.button_warning_foreground = theme.warning_foreground;
    theme.button_info = theme.info;
    theme.button_info_hover = theme.info_hover;
    theme.button_info_active = theme.info_active;
    theme.button_info_foreground = theme.info_foreground;
}

/// 切换浅色 / 深色主题，立即刷新全部窗口并把最终选择异步写入 Storage。
pub fn toggle_theme(cx: &mut App) {
    let next = match current_mode(cx) {
        Mode::Light => Mode::Dark,
        Mode::Dark => Mode::Light,
    };
    set_theme_preference(next, cx);
}

/// 设置页与快捷主题按钮共用同一存储键，避免保存两个互相覆盖的主题偏好。
pub fn set_theme_preference(next: Mode, cx: &mut App) {
    apply_theme(next, cx);
    cx.refresh_windows();
    let preference = match next {
        Mode::Dark => "dark",
        Mode::Light => "light",
    };
    crate::preferences::persist_preference_latest("theme_mode", preference.to_string(), cx);
}

/// 为可拖拽滚动条提供稳定的滑块对比度；轨道保持透明以移除外圈边界和阴影。
fn configure_scrollbar_paint(theme: &mut Theme) {
    let track = hsla(0.0, 0.0, 0.0, 0.0);
    let mut thumb = theme.muted_foreground;
    thumb.a = 0.68;
    let mut thumb_hover = theme.foreground;
    thumb_hover.a = 0.82;
    theme.scrollbar = track;
    theme.scrollbar_thumb = thumb;
    theme.scrollbar_thumb_hover = thumb_hover;
}

/// Project compact, marginless geometry onto GPUI Base after every theme sync.
pub(crate) fn configure_base_scrollbar(cx: &mut App) {
    let base_theme = gpui_kit::base::Theme::global_mut(cx);
    let mode = base_theme.scrollbar.mode();
    let motion = base_theme.scrollbar.motion();
    let transparent = hsla(0.0, 0.0, 0.0, 0.0);
    let styles = gpui_kit::base::ScrollbarStyles::default()
        .track(|style| {
            style
                .width(SCROLLBAR_TRACK_WIDTH)
                .bg(transparent)
                .border_color(transparent)
        })
        .track_hover(|style| style.bg(transparent).border_color(transparent))
        .track_active(|style| style.bg(transparent).border_color(transparent))
        .thumb(|style| {
            style
                .width(SCROLLBAR_THUMB_WIDTH)
                .inset(SCROLLBAR_THUMB_INSET)
                .radius(SCROLLBAR_THUMB_RADIUS)
        })
        .thumb_hover(|style| {
            style
                .width(SCROLLBAR_THUMB_HOVER_WIDTH)
                .inset(SCROLLBAR_THUMB_INSET)
                .radius(SCROLLBAR_THUMB_RADIUS)
        })
        .thumb_active(|style| {
            style
                .width(SCROLLBAR_THUMB_ACTIVE_WIDTH)
                .inset(SCROLLBAR_THUMB_INSET)
                .radius(SCROLLBAR_THUMB_RADIUS)
        });
    base_theme.scrollbar = gpui_kit::base::ScrollbarTheme::new()
        .with_mode(mode)
        .with_motion(motion)
        .with_styles(styles);
}

/// 编辑器背景对齐主背景；浅色默认主题的亮蓝色注释改为更常见的绿色。
fn normalize_editor_highlight_theme(mode: Mode, cx: &mut App) {
    let theme = Theme::global_mut(cx);
    let bg = theme.background;
    let hl = normalized_editor_highlight_theme(mode, bg, &theme.highlight_theme);
    theme.highlight_theme = Arc::new(hl);
}

fn normalized_editor_highlight_theme(
    mode: Mode,
    bg: Hsla,
    source: &HighlightTheme,
) -> HighlightTheme {
    let mut hl = source.clone();
    hl.style.editor_background = Some(bg);
    if matches!(mode, Mode::Light) {
        let comment = hl.style.syntax.string;
        hl.style.syntax.comment = comment;
        hl.style.syntax.comment_doc = comment;
    }
    hl
}

pub fn current_mode(cx: &App) -> Mode {
    if matches!(Theme::global(cx).mode, ThemeMode::Light) {
        Mode::Light
    } else {
        Mode::Dark
    }
}

/// JetBrains 风格深色工作区配色：中性深灰承载层次，蓝色只用于焦点、选中和动作。
fn apply_dark_palette(theme: &mut Theme) {
    let pulse = crate::pulse_ui::pulse_palette_for_mode(Mode::Dark);
    let accent = rgb(0x82b9ff).into();
    let accent_hover = hsl(209.0, 100.0, 78.0);
    let accent_active = hsl(209.0, 100.0, 72.0);
    let primary: Hsla = rgb(0xededed).into();
    let primary_hover = primary.opacity(0.88);
    let row_hover = rgb(0x202020).into();

    theme.accent = accent;
    theme.accent_foreground = rgb(0x111111).into();
    theme.primary = primary;
    theme.primary_hover = primary_hover;
    theme.primary_active = primary_hover;
    theme.primary_foreground = rgb(0x111111).into();

    theme.link = accent_hover;
    theme.link_hover = hsl(209.0, 100.0, 78.0);
    theme.link_active = accent_active;

    theme.background = pulse.background;
    theme.secondary = pulse.surface;
    theme.sidebar = pulse.background;
    theme.title_bar = pulse.background;
    theme.title_bar_border = pulse.border;

    theme.border = pulse.border;
    theme.input = pulse.background;

    theme.foreground = pulse.text;
    theme.muted = row_hover;
    theme.muted_foreground = pulse.muted;
    theme.secondary_foreground = pulse.text;

    theme.danger = rgb(0xff9b9b).into();
    theme.danger_hover = rgb(0xffaaaa).into();
    theme.danger_active = rgb(0xffbaba).into();
    theme.danger_foreground = theme.primary_foreground;

    theme.success = rgb(0x74d5a2).into();
    theme.success_hover = rgb(0x84dcb0).into();
    theme.success_active = rgb(0x9be6c2).into();
    theme.success_foreground = theme.primary_foreground;

    theme.warning = rgb(0xe9bf75).into();
    theme.warning_hover = rgb(0xedcc8e).into();
    theme.warning_active = rgb(0xf1d7a8).into();
    theme.warning_foreground = theme.primary_foreground;

    theme.info = accent;
    theme.info_hover = accent_hover;
    theme.info_active = accent_active;
    theme.info_foreground = theme.primary_foreground;

    theme.selection = pulse.selected;

    theme.list_active = pulse.selected;
    theme.list_active_border = pulse.border;
    theme.list_hover = row_hover;

    theme.popover = pulse.surface;
    theme.popover_foreground = pulse.text;

    // 补全前缀高亮：暗色下浅蓝可见于选中态深蓝 bg
    theme.blue = hsl(209.0, 90.0, 75.0);
    theme.blue_light = hsl(209.0, 90.0, 84.0);
}

/// VSCode Light+ 配色
fn apply_light_palette(theme: &mut Theme) {
    let pulse = crate::pulse_ui::pulse_palette_for_mode(Mode::Light);
    let accent = rgb(0x175cbb).into();
    let accent_hover = rgb(0x124b98).into();
    let accent_active = rgb(0x0e3c7a).into();
    let primary: Hsla = rgb(0x171717).into();
    let primary_hover = primary.opacity(0.88);
    let row_hover = rgb(0xeeeeee).into();

    theme.accent = accent;
    theme.accent_foreground = hsl(0.0, 0.0, 100.0);
    theme.primary = primary;
    theme.primary_hover = primary_hover;
    theme.primary_active = primary_hover;
    theme.primary_foreground = hsl(0.0, 0.0, 100.0);

    theme.link = accent;
    theme.link_hover = accent_hover;
    theme.link_active = accent_active;

    theme.background = pulse.background;
    theme.secondary = pulse.surface;
    theme.sidebar = pulse.background;
    theme.title_bar = pulse.background;
    theme.title_bar_border = pulse.border;

    theme.border = pulse.border;
    theme.input = pulse.background;

    theme.foreground = pulse.text;
    theme.muted = row_hover;
    theme.muted_foreground = pulse.muted;
    theme.secondary_foreground = pulse.text;

    theme.danger = rgb(0xb92525).into();
    theme.danger_hover = rgb(0xa71f1f).into();
    theme.danger_active = rgb(0x921a1a).into();
    theme.danger_foreground = theme.primary_foreground;

    // 原型浅色 r-green 在状态底色上不足 4.5:1，轻微加深后保留绿色语义。
    theme.success = rgb(0x176f46).into();
    theme.success_hover = rgb(0x14653f).into();
    theme.success_active = rgb(0x105336).into();
    theme.success_foreground = theme.primary_foreground;

    theme.warning = rgb(0x885913).into();
    theme.warning_hover = rgb(0x754d10).into();
    theme.warning_active = rgb(0x66420e).into();
    theme.warning_foreground = theme.primary_foreground;

    theme.info = accent;
    theme.info_hover = accent_hover;
    theme.info_active = accent_active;
    theme.info_foreground = theme.primary_foreground;

    theme.selection = pulse.selected;

    theme.list_active = pulse.selected;
    theme.list_active_border = pulse.border;
    theme.list_hover = row_hover;

    theme.popover = pulse.surface;
    theme.popover_foreground = pulse.text;

    // 补全前缀高亮：浅色下 blue 须比 accent 更亮才能看清
    theme.blue = hsl(207.0, 100.0, 65.0);
    theme.blue_light = hsl(207.0, 100.0, 75.0);
}

/// 将 HSL 数值范围转换为 Hsla。
fn hsl(h: f32, s: f32, l: f32) -> Hsla {
    hsla(h / 360.0, s / 100.0, l / 100.0, 1.0)
}

trait Opacity {
    fn opacity(self, alpha: f32) -> Self;
}

impl Opacity for Hsla {
    fn opacity(mut self, alpha: f32) -> Self {
        self.a = alpha.clamp(0.0, 1.0);
        self
    }
}

#[cfg(test)]
#[path = "theme_palette_tests.rs"]
mod palette_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrollbar_geometry_is_compact_and_marginless() {
        assert_eq!(SCROLLBAR_TRACK_WIDTH, px(10.0));
        assert_eq!(SCROLLBAR_THUMB_WIDTH, px(6.0));
        assert_eq!(SCROLLBAR_THUMB_INSET, px(0.0));
        assert_eq!(SCROLLBAR_THUMB_RADIUS, px(3.0));
    }

    #[test]
    fn light_editor_comments_are_readable_and_background_matches_app() {
        let source = HighlightTheme::default_light();
        let bg = hsl(0.0, 0.0, 100.0);

        let normalized = normalized_editor_highlight_theme(Mode::Light, bg, &source);

        assert_eq!(normalized.style.editor_background, Some(bg));
        assert_eq!(
            normalized.style.syntax.comment,
            normalized.style.syntax.string
        );
        assert_eq!(
            normalized.style.syntax.comment_doc,
            normalized.style.syntax.string
        );
    }

    #[test]
    fn dark_editor_keeps_existing_comment_style() {
        let source = HighlightTheme::default_dark();
        let normalized =
            normalized_editor_highlight_theme(Mode::Dark, hsl(0.0, 0.0, 12.0), &source);

        assert_eq!(normalized.style.syntax.comment, source.style.syntax.comment);
        assert_eq!(
            normalized.style.syntax.comment_doc,
            source.style.syntax.comment_doc
        );
    }
}
