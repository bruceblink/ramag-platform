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
        let top = (viewport.height * 0.05).min(px(42.0));
        let height = (viewport.height - top - px(16.0)).max(px(0.0));
        Self {
            width: (viewport.width - px(32.0))
                .max(px(0.0))
                .min(px(preferred_width)),
            top,
            height,
            body_height: (height - px(80.0)).max(px(0.0)),
        }
    }
}
