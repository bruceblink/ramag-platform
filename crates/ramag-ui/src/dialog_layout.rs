//! Bounds shared dialogs against the viewport, including short desktop windows.

use gpui::{Pixels, Window, px};

pub(crate) struct DialogLayout {
    pub width: Pixels,
    pub top: Pixels,
    pub height: Pixels,
    pub body_height: Pixels,
}

impl DialogLayout {
    /// Reserve space for the title, padding and bottom margin before sizing scrollable content.
    pub fn new(window: &Window, preferred_width: f32) -> Self {
        let viewport = window.viewport_size();
        let top = crate::responsive_dialog_top(window);
        let height = (viewport.height - top - px(16.0)).max(px(0.0));
        Self {
            width: crate::responsive_dialog_width(window, preferred_width),
            top,
            height,
            body_height: (height - px(80.0)).max(px(0.0)),
        }
    }
}
