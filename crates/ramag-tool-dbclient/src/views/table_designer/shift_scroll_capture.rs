//! Shift + 滚轮在字段视口的捕获阶段转为横向滚动。

use gpui_kit::{
    App, Bounds, DispatchPhase, Element, ElementId, GlobalElementId, Hitbox, HitboxBehavior,
    InspectorElementId, IntoElement, LayoutId, ScrollEvent, Style, Window, px, relative,
};

use super::TableDesigner;

pub(super) struct ShiftScrollCapture {
    designer: gpui_kit::Entity<TableDesigner>,
}

impl ShiftScrollCapture {
    pub(super) fn new(designer: gpui_kit::Entity<TableDesigner>) -> Self {
        Self { designer }
    }
}

impl IntoElement for ShiftScrollCapture {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ShiftScrollCapture {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some("table-designer-shift-scroll-capture".into())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let style = Style {
            position: gpui_kit::Position::Absolute,
            inset: gpui_kit::Edges {
                top: px(0.0).into(),
                left: px(0.0).into(),
                ..Default::default()
            },
            flex_grow: 1.0,
            flex_shrink: 1.0,
            size: gpui_kit::Size {
                width: relative(1.0).into(),
                height: relative(1.0).into(),
            },
            ..Default::default()
        };
        (window.request_layout(style, None, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<gpui_kit::Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        _: &mut App,
    ) -> Self::PrepaintState {
        window.insert_hitbox(bounds, HitboxBehavior::Normal)
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<gpui_kit::Pixels>,
        _: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        _: &mut App,
    ) {
        let hitbox = hitbox.id;
        let designer = self.designer.clone();
        window.on_mouse_event(move |event: &ScrollEvent, phase, window, app| {
            if phase != DispatchPhase::Capture
                || !event.modifiers.shift
                || !hitbox.should_handle_scroll(window)
            {
                return;
            }

            designer.update(app, |this, cx| this.on_field_scroll(event, window, cx));
        });
    }
}
