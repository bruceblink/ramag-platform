use gpui_kit::component::ActiveTheme;
use gpui_kit::*;
use system_pulse_model::Screen;

pub(crate) fn palette(cx: &App) -> Palette {
    ramag_ui::pulse_ui::pulse_palette(cx)
}

type Palette = ramag_ui::pulse_ui::PulsePalette;

pub(crate) fn accent(screen: Screen, cx: &App) -> Hsla {
    let dark = cx.theme().is_dark();
    rgb(match screen {
        Screen::Cpu | Screen::Disks => {
            if dark {
                0x87c966
            } else {
                0x437d2c
            }
        }
        Screen::Memory => {
            if dark {
                0xb764e8
            } else {
                0x8535b8
            }
        }
        Screen::Energy => {
            if dark {
                0xe8d64b
            } else {
                0x887000
            }
        }
        Screen::Thermals => {
            if dark {
                0xe6a54a
            } else {
                0x9c5d15
            }
        }
        _ => {
            if dark {
                0x6aacf0
            } else {
                0x326ead
            }
        }
    })
    .into()
}

pub(crate) fn heading(text: impl Into<SharedString>, size: f32, cx: &App) -> Div {
    ramag_ui::pulse_ui::pulse_display_heading(text, size, cx)
}

pub(crate) fn section(cx: &App) -> Div {
    let colors = palette(cx);
    div()
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .rounded(px(8.))
        .border_1()
        .border_color(colors.border)
        .bg(colors.surface)
        .min_w_0()
}

pub(crate) fn empty(title: &str, detail: &str, cx: &App) -> AnyElement {
    section(cx)
        .min_h(px(180.))
        .justify_center()
        .child(heading(title.to_owned(), 22., cx))
        .child(
            div()
                .text_sm()
                .text_color(palette(cx).muted)
                .child(detail.to_owned()),
        )
        .into_any_element()
}
