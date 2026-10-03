use super::{
    render_session_context_header, render_session_tab_title, session_pulse_status,
    session_tab_status_colors, session_tab_title_max_width,
};
use gpui_kit::{Context, IntoElement, Render, TestAppContext, Window, px, size};

struct SessionContextHeaderPreview;

impl Render for SessionContextHeaderPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_session_context_header(
            "一个很长的数据库连接名称用于页头边界验证".into(),
            "PostgreSQL · db.internal.example:5432".into(),
            ramag_ui::pulse_ui::PulseStatus::Current,
            "已连接",
            cx,
        )
    }
}

struct SessionTabTitlePreview {
    title: String,
}

impl Render for SessionTabTitlePreview {
    fn render(&mut self, window: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        render_session_tab_title(
            self.title.clone(),
            "session-tab-title".into(),
            session_tab_title_max_width(f32::from(window.viewport_size().width)),
            gpui_kit::hsla(0.0, 0.0, 0.9, 1.0),
        )
    }
}

#[gpui_kit::test]
fn session_tab_title_stays_within_its_responsive_width(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (_, cx) = cx.add_window_view(|_, _| SessionTabTitlePreview {
        title: "a-very-long-connection-name-that-must-remain-discoverable".into(),
    });

    for width in [360.0, 1024.0, 1440.0] {
        let max_width = session_tab_title_max_width(width);
        cx.simulate_resize(size(px(width), px(240.0)));
        cx.run_until_parked();

        let bounds = cx
            .debug_bounds("session-tab-title-bounds")
            .expect("会话标签标题应渲染");
        assert!(
            bounds.size.width <= px(max_width),
            "标题宽度不能超过响应式上限：bounds={bounds:?}, max={max_width}"
        );
    }
}

#[test]
fn session_tab_title_width_leaves_room_for_context_actions() {
    assert_eq!(session_tab_title_max_width(360.0), 140.0);
    assert_eq!(session_tab_title_max_width(1024.0), 180.0);
    assert_eq!(session_tab_title_max_width(1440.0), 240.0);
    assert!(session_tab_title_max_width(360.0) < session_tab_title_max_width(1440.0));
}

#[test]
fn session_tab_statuses_follow_theme_colors() {
    let warning = gpui_kit::hsla(0.1, 0.2, 0.3, 1.0);
    let danger = gpui_kit::hsla(0.2, 0.3, 0.4, 1.0);
    let success = gpui_kit::hsla(0.3, 0.4, 0.5, 1.0);
    let muted = gpui_kit::hsla(0.4, 0.5, 0.6, 1.0);

    assert_eq!(
        session_tab_status_colors(true, Some((false, true)), warning, danger, success, muted),
        (warning, "需重连", warning)
    );
    assert_eq!(
        session_tab_status_colors(false, None, warning, danger, success, muted),
        (muted, "未连接", muted)
    );
    assert_eq!(
        session_tab_status_colors(false, Some((true, false)), warning, danger, success, muted),
        (warning, "连接中", warning)
    );
    assert_eq!(
        session_tab_status_colors(false, Some((false, true)), warning, danger, success, muted),
        (danger, "连接失败", danger)
    );
    assert_eq!(
        session_tab_status_colors(false, Some((false, false)), warning, danger, success, muted),
        (success, "已连接", success)
    );
}

#[test]
fn session_pulse_status_preserves_connection_semantics() {
    assert_eq!(
        session_pulse_status(false, None),
        (ramag_ui::pulse_ui::PulseStatus::Unavailable, "未连接")
    );
    assert_eq!(
        session_pulse_status(false, Some((true, false))),
        (ramag_ui::pulse_ui::PulseStatus::Warming, "连接中")
    );
    assert_eq!(
        session_pulse_status(false, Some((false, true))),
        (ramag_ui::pulse_ui::PulseStatus::Failed, "连接失败")
    );
    assert_eq!(
        session_pulse_status(true, Some((false, false))),
        (ramag_ui::pulse_ui::PulseStatus::Stale, "需重连")
    );
}

#[gpui_kit::test]
fn session_context_header_stays_inside_supported_window_sizes(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (preview, cx) = cx.add_window_view(|_, _| SessionContextHeaderPreview);

    for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
        cx.simulate_resize(size(px(width), px(height)));
        preview.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();

        let header = cx
            .debug_bounds("dbclient-session-header")
            .expect("数据库会话上下文栏应渲染");
        let title = cx
            .debug_bounds("dbclient-session-header-title")
            .expect("数据库会话上下文标题应渲染");
        let status = cx
            .debug_bounds("dbclient-session-header-status")
            .expect("数据库会话状态应渲染");
        for (label, bounds) in [("上下文栏", header), ("标题", title), ("状态", status)] {
            assert!(
                bounds.origin.x >= px(0.0)
                    && bounds.origin.y >= px(0.0)
                    && bounds.right() <= px(width)
                    && bounds.bottom() <= px(height),
                "{label}不能越出窗口：{bounds:?} at {width}x{height}"
            );
        }
        assert!(title.size.width > px(0.0));
        assert!(status.size.width > px(0.0));
    }
}
