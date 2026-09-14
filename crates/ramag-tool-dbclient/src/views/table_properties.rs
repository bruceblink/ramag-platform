//! 单表 DDL 预览与触发器只读信息。

use std::sync::Arc;

use gpui::{
    AnyElement, AppContext as _, ClickEvent, Context, DragMoveEvent, EventEmitter, FocusHandle,
    IntoElement, MouseButton, ParentElement, Point, Render, ScrollHandle, Styled, Window, div,
    point, prelude::*, px,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, Theme,
    button::ButtonVariants as _, h_flex, v_flex,
};
use ramag_app::ConnectionService;
use ramag_domain::entities::{ConnectionConfig, Query, Trigger, Value};
use tracing::error;

mod ddl;
mod triggers;

use self::{ddl::render_ddl, triggers::render as render_triggers};

const MODAL_WIDTH: f32 = 1160.0;
const MODAL_HEIGHT: f32 = 650.0;
const MODAL_MARGIN: f32 = 16.0;

#[derive(Clone, Debug)]
pub(crate) enum TablePropertiesEvent {
    CloseRequested,
}

impl EventEmitter<TablePropertiesEvent> for TablePropertiesDialog {}

#[derive(Clone, Copy, Debug)]
struct TablePropertiesDrag;

#[derive(Clone, Copy, Debug)]
struct DragState {
    cursor: Point<gpui::Pixels>,
    position: Point<gpui::Pixels>,
}

pub(crate) struct TablePropertiesDialog {
    service: Arc<ConnectionService>,
    connection: ConnectionConfig,
    schema: String,
    table: String,
    is_view: bool,
    ddl_loading: bool,
    ddl_text: Option<String>,
    ddl_error: Option<String>,
    triggers_loading: bool,
    triggers: Option<Vec<Trigger>>,
    triggers_error: Option<String>,
    request_generation: u64,
    position: Option<Point<gpui::Pixels>>,
    drag_state: Option<DragState>,
    focus_handle: FocusHandle,
    ddl_vertical_scroll: ScrollHandle,
    ddl_horizontal_scroll: ScrollHandle,
    triggers_vertical_scroll: ScrollHandle,
}

impl TablePropertiesDialog {
    pub(crate) fn new(
        service: Arc<ConnectionService>,
        connection: ConnectionConfig,
        schema: String,
        table: String,
        is_view: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut this = Self {
            service,
            connection,
            schema,
            table,
            is_view,
            ddl_loading: false,
            ddl_text: None,
            ddl_error: None,
            triggers_loading: false,
            triggers: None,
            triggers_error: None,
            request_generation: 0,
            position: None,
            drag_state: None,
            focus_handle: cx.focus_handle(),
            ddl_vertical_scroll: ScrollHandle::new(),
            ddl_horizontal_scroll: ScrollHandle::new(),
            triggers_vertical_scroll: ScrollHandle::new(),
        };
        this.refresh(cx);
        this
    }

    pub(crate) fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle.focus(window, cx);
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.request_generation = self.request_generation.wrapping_add(1);
        let request_generation = self.request_generation;
        let service = self.service.clone();
        let connection = self.connection.clone();
        let schema = self.schema.clone();
        let table = self.table.clone();
        let is_view = self.is_view;
        let trigger_service = service.clone();
        let trigger_connection = connection.clone();
        let trigger_schema = schema.clone();
        let trigger_table = table.clone();
        let trigger_is_view = is_view;
        self.ddl_loading = true;
        self.ddl_text = None;
        self.ddl_error = None;
        self.triggers_loading = true;
        self.triggers = None;
        self.triggers_error = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let (ddl, triggers) = futures::join!(
                load_table_ddl(service, connection.clone(), schema, table, is_view),
                load_table_triggers(
                    trigger_service,
                    trigger_connection,
                    trigger_schema,
                    trigger_table,
                    trigger_is_view,
                ),
            );
            let _ = this.update(cx, |this, cx| {
                if this.request_generation != request_generation
                    || this.connection.id != connection.id
                {
                    return;
                }
                this.ddl_loading = false;
                match ddl {
                    Ok(ddl) => this.ddl_text = Some(ddl),
                    Err(error) => {
                        error!(
                            operation = "table_properties_ddl_load",
                            connection_id = %this.connection.id,
                            schema = %this.schema,
                            table = %this.table,
                            error = %error,
                            "load table properties DDL failed"
                        );
                        this.ddl_error = Some(format!("加载建表语句失败：{error:#}"));
                    }
                }
                this.triggers_loading = false;
                match triggers {
                    Ok(triggers) => this.triggers = Some(triggers),
                    Err(error) => {
                        error!(
                            operation = "table_properties_triggers_load",
                            connection_id = %this.connection.id,
                            schema = %this.schema,
                            table = %this.table,
                            error = %error,
                            "load table properties triggers failed"
                        );
                        this.triggers_error = Some(format!("加载触发器失败：{error:#}"));
                    }
                }
                this.ddl_vertical_scroll
                    .set_offset(gpui::Point::new(px(0.0), px(0.0)));
                this.ddl_horizontal_scroll
                    .set_offset(gpui::Point::new(px(0.0), px(0.0)));
                this.triggers_vertical_scroll
                    .set_offset(gpui::Point::new(px(0.0), px(0.0)));
                cx.notify();
            });
        })
        .detach();
    }

    fn request_close(&mut self, cx: &mut Context<Self>) {
        cx.emit(TablePropertiesEvent::CloseRequested);
    }

    fn begin_drag(&mut self, cursor: Point<gpui::Pixels>) {
        let position = self
            .position
            .unwrap_or_else(|| point(px(MODAL_MARGIN), px(MODAL_MARGIN)));
        self.drag_state = Some(DragState { cursor, position });
    }

    fn update_drag(&mut self, cursor: Point<gpui::Pixels>, viewport: gpui::Size<gpui::Pixels>) {
        let Some(drag) = self.drag_state else {
            return;
        };
        let next = point(
            drag.position.x + cursor.x - drag.cursor.x,
            drag.position.y + cursor.y - drag.cursor.y,
        );
        self.position = Some(clamp_position(next, viewport, modal_size(viewport)));
    }

    fn end_drag(&mut self) {
        self.drag_state = None;
    }

    fn render_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let close_button = ramag_ui::clickable_button("table-properties-close")
            .ghost()
            .small()
            .icon(IconName::Close)
            .tooltip("关闭")
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.request_close(cx)));
        let refresh_button = ramag_ui::clickable_button("table-properties-refresh")
            .ghost()
            .small()
            .icon(ramag_ui::icons::refresh_cw())
            .tooltip("重新加载 DDL 和触发器")
            .disabled(self.ddl_loading || self.triggers_loading)
            .on_click(cx.listener(|this, _, _, cx| this.refresh(cx)));

        h_flex()
            .id("table-properties-drag-handle")
            .w_full()
            .h(px(48.0))
            .flex_none()
            .items_center()
            .gap(px(8.0))
            .px(px(12.0))
            .border_b_1()
            .border_color(theme.border)
            .bg(theme.secondary)
            .cursor_move()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                    this.begin_drag(event.position);
                    cx.stop_propagation();
                }),
            )
            .on_drag(TablePropertiesDrag, |_, _, _, cx| {
                cx.stop_propagation();
                cx.new(|_| gpui::Empty)
            })
            .on_drag_move::<TablePropertiesDrag>(cx.listener(
                |this, event: &DragMoveEvent<TablePropertiesDrag>, window, cx| {
                    this.update_drag(event.event.position, window.viewport_size());
                    cx.notify();
                },
            ))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.end_drag();
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.end_drag();
                    cx.notify();
                }),
            )
            .child(
                Icon::new(if self.is_view {
                    IconName::Frame
                } else {
                    IconName::MemoryStick
                })
                .small()
                .text_color(theme.accent),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(1.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(format!("{}.{}", self.schema, self.table)),
                    )
                    .child(div().text_xs().text_color(theme.muted_foreground).child(
                        if self.is_view {
                            "只读视图定义"
                        } else {
                            "只读建表语句"
                        },
                    )),
            )
            .child(
                div()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(refresh_button),
            )
            .child(
                div()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(close_button),
            )
            .into_any_element()
    }

    fn render_modal(
        &self,
        position: Point<gpui::Pixels>,
        size: gpui::Size<gpui::Pixels>,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        v_flex()
            .id("table-properties-modal")
            .absolute()
            .left(position.x)
            .top(position.y)
            .w(size.width)
            .h(size.height)
            .min_h_0()
            .overflow_hidden()
            .bg(theme.background)
            .border_1()
            .border_color(theme.border)
            .rounded(px(7.0))
            .occlude()
            .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
            .key_context("TablePropertiesDialog")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    this.request_close(cx);
                    cx.stop_propagation();
                }
            }))
            .child(self.render_header(cx))
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .p(px(12.0))
                    .gap(px(8.0))
                    .child(render_triggers(self, theme))
                    .child(div().flex_1().min_h_0().child(render_ddl(
                        self.ddl_loading,
                        self.ddl_text.clone(),
                        self.ddl_error.clone(),
                        &self.ddl_vertical_scroll,
                        &self.ddl_horizontal_scroll,
                        theme,
                    ))),
            )
            .into_any_element()
    }
}

impl Render for TablePropertiesDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let viewport = window.viewport_size();
        let size = modal_size(viewport);
        let initial_position = self.position.unwrap_or_else(|| {
            point(
                ((viewport.width - size.width) / 2.0).max(px(MODAL_MARGIN)),
                ((viewport.height - size.height) / 2.0).max(px(MODAL_MARGIN)),
            )
        });
        let position = clamp_position(initial_position, viewport, size);
        self.position = Some(position);

        let backdrop = div()
            .id("table-properties-backdrop")
            .absolute()
            .inset_0()
            .bg(theme.overlay)
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.request_close(cx)),
            );

        div().absolute().inset_0().children([
            backdrop.into_any_element(),
            self.render_modal(position, size, &theme, cx),
        ])
    }
}

fn modal_size(viewport: gpui::Size<gpui::Pixels>) -> gpui::Size<gpui::Pixels> {
    let available_width = (viewport.width - px(MODAL_MARGIN * 2.0)).max(px(1.0));
    let available_height = (viewport.height - px(MODAL_MARGIN * 2.0)).max(px(1.0));
    gpui::Size::new(
        available_width.min(px(MODAL_WIDTH)),
        available_height.min(px(MODAL_HEIGHT)),
    )
}

fn clamp_position(
    position: Point<gpui::Pixels>,
    viewport: gpui::Size<gpui::Pixels>,
    size: gpui::Size<gpui::Pixels>,
) -> Point<gpui::Pixels> {
    let min = px(MODAL_MARGIN);
    let max_x = (viewport.width - size.width - min).max(min);
    let max_y = (viewport.height - size.height - min).max(min);
    point(position.x.clamp(min, max_x), position.y.clamp(min, max_y))
}

/// Loads the database-native definition used by the read-only DDL preview.
/// The result keeps the driver's original SQL text so users can inspect or copy it without edits.
async fn load_table_ddl(
    service: Arc<ConnectionService>,
    connection: ConnectionConfig,
    schema: String,
    table: String,
    is_view: bool,
) -> anyhow::Result<String> {
    let sql = ramag_domain::entities::build_ddl_query(connection.driver, &schema, &table, is_view);
    if sql.is_empty() {
        return Err(anyhow::anyhow!("当前数据库类型不支持 DDL 查看"));
    }
    let result = service.execute(&connection, &Query::new(sql)).await?;
    result
        .rows
        .first()
        .and_then(|row| row.values.iter().rev().find_map(value_as_ddl))
        .filter(|ddl| !ddl.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("数据库未返回 {schema}.{table} 的定义"))
}

/// Loads table-level trigger metadata through the driver's bounded metadata API.
/// Views return an empty list because they do not own table triggers in the supported drivers.
async fn load_table_triggers(
    service: Arc<ConnectionService>,
    connection: ConnectionConfig,
    schema: String,
    table: String,
    is_view: bool,
) -> ramag_domain::error::Result<Vec<Trigger>> {
    if is_view {
        return Ok(Vec::new());
    }
    service.list_triggers(&connection, &schema, &table).await
}

fn value_as_ddl(value: &Value) -> Option<String> {
    match value {
        Value::Text(value) => Some(value.clone()),
        Value::Json(value) => Some(value.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
