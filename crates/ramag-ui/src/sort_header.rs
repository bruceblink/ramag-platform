//! Shared visual treatment for interactive table-column sorting.

use gpui_kit::component::{Icon, IconName, Sizable as _};
use gpui_kit::{Div, Hsla, IntoElement, ParentElement as _, Styled as _, px};

/// The visible direction represented by an active sorted column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection {
    /// Values appear from lowest to highest (or alphabetically).
    Ascending,
    /// Values appear from highest to lowest (or reverse alphabetically).
    Descending,
}

/// Returns the common icon used for inactive, ascending, and descending headers.
pub const fn sort_icon_name(direction: Option<SortDirection>) -> IconName {
    match direction {
        Some(SortDirection::Ascending) => IconName::ArrowUp,
        Some(SortDirection::Descending) => IconName::ArrowDown,
        None => IconName::ChevronsUpDown,
    }
}

/// Builds the same small sort indicator for every sortable table header.
pub fn sort_indicator(direction: Option<SortDirection>, muted: Hsla, active: Hsla) -> Icon {
    Icon::new(sort_icon_name(direction))
        .xsmall()
        .text_color(if direction.is_some() { active } else { muted })
}

/// Places a caller-styled column label beside the shared sort indicator.
///
/// Each table retains its own accessible click target and width rules while
/// this wrapper keeps icon sizing and label spacing consistent.
pub fn sortable_header_content(
    label: impl IntoElement,
    direction: Option<SortDirection>,
    muted: Hsla,
    active: Hsla,
) -> Div {
    gpui_kit::component::h_flex()
        .w_full()
        .h_full()
        .min_w_0()
        .items_center()
        .gap(px(4.0))
        .text_xs()
        .font_weight(gpui_kit::FontWeight::MEDIUM)
        .text_color(muted)
        .child(label)
        .child(sort_indicator(direction, muted, active).flex_none())
}
