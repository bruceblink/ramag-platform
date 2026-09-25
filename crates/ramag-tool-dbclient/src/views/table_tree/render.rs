use std::ops::Range;

use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Selectable as _, Sizable as _, WindowExt as _,
    button::ButtonVariants as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    ClickEvent, Context, InteractiveElement, IntoElement, ParentElement, Render, Styled, Window,
    div, img, px, uniform_list,
};
use ramag_domain::entities::DriverKind;
use ramag_ui::PointerDropdownMenu as _;

use super::{
    TableTreeFilter, TableTreePanel, TreeEvent, ops::TableDdlNotification,
    show_fullscreen_schema_error, show_fullscreen_schema_loading,
};
use crate::sql_completion::is_system_schema;

impl Render for TableTreePanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.clear_ddl_notification) {
            window.remove_notification::<TableDdlNotification>(cx);
        }
        if let Some(n) = self.pending_notification.take() {
            ramag_ui::push_responsive_notification(window, n, cx);
        }
        let muted_fg = cx.theme().muted_foreground;
        let red = gpui_kit::red();

        if self.connection.is_none() {
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .text_color(muted_fg)
                .text_xs()
                .child("从左侧选一个连接")
                .into_any_element();
        }

        if show_fullscreen_schema_loading(&self.schemas, self.loading_schemas) {
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .text_color(muted_fg)
                .text_xs()
                .child("正在加载数据库…")
                .into_any_element();
        }

        if show_fullscreen_schema_error(&self.schemas, self.error.as_deref()) {
            let err = self.error.clone().unwrap_or_default();
            return v_flex()
                .size_full()
                .p_2()
                .gap_2()
                .child(
                    div()
                        .text_xs()
                        .text_color(red)
                        .child(format!("加载失败：{err}")),
                )
                .child(
                    ramag_ui::clickable_button("retry")
                        .small()
                        .label("重试")
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.load_schemas(cx);
                        })),
                )
                .into_any_element();
        }

        let show_system = self.show_system;
        let filter = self.current_filter(cx);
        let has_filter = !filter.is_empty();
        let tree_view = self.tree_rows_view(&filter);
        let total_schemas = self.schemas.len();
        let visible_schemas = tree_view.visible_schemas;
        let mut header_text = if total_schemas == visible_schemas {
            format!("数据库 ({total_schemas})")
        } else {
            format!("数据库 ({visible_schemas}/{total_schemas})")
        };
        let searchable_schemas = tree_view.searchable_schemas;
        let failed_schemas = tree_view.failed_schemas;
        // 搜索范围必须与“显示系统库”筛选一致，否则隐藏的系统库会让进度永远无法完成。
        let search_scope_total = self
            .schemas
            .iter()
            .filter(|schema| show_system || !is_system_schema(&schema.name))
            .count();
        let search_incomplete = has_filter && searchable_schemas < search_scope_total;
        if search_incomplete {
            header_text.push_str(&format!(
                " · 当前搜索范围 {searchable_schemas}/{search_scope_total} 个库"
            ));
        }
        if has_filter && failed_schemas > 0 {
            header_text.push_str(&format!(" · {failed_schemas} 个库加载失败"));
        }
        if self.ddl_gate.is_busy() {
            header_text.push_str(" · 结构变更执行中…");
        }
        if self.loading_schemas {
            header_text.push_str(" · 刷新中…");
        }
        if let Some(error) = self.error.as_deref() {
            header_text.push_str(&format!(" · 刷新失败：{error}"));
        }
        let toggle_icon = if show_system {
            IconName::Eye
        } else {
            IconName::EyeOff
        };
        let qp_visible = self.editor_visible;
        let table_filter = self.table_filter;
        let filter_entity = cx.entity().clone();
        let filter_button = ramag_ui::clickable_button("table-tree-filter")
            .ghost()
            .small()
            .label(format!("{} ▾", table_filter.label()))
            .flex_none()
            .debug_selector(|| "table-tree-filter".into())
            .pointer_dropdown_menu_with_anchor(gpui_kit::Anchor::BottomLeft, move |menu, _, _| {
                let mut menu = menu;
                for option in [
                    TableTreeFilter::All,
                    TableTreeFilter::Favorites,
                    TableTreeFilter::Recent,
                ] {
                    let entity = filter_entity.clone();
                    menu = menu.item(
                        ramag_ui::menu_item(if option == table_filter {
                            format!("✓ {}", option.label())
                        } else {
                            option.label().to_string()
                        })
                        .on_click(move |_, _, app| {
                            entity.update(app, |this, cx| this.set_table_filter(option, cx));
                        }),
                    );
                }
                menu
            });
        let driver = self.connection.as_ref().map(|c| c.driver);
        let pg_database: Option<String> = self
            .connection
            .as_ref()
            .filter(|c| matches!(c.driver, DriverKind::Postgres))
            .and_then(|c| c.database.clone());
        let active_label = self
            .active_schema
            .clone()
            .unwrap_or_else(|| "未选库".to_string());
        let picker_label = match (driver, pg_database.as_deref()) {
            (Some(DriverKind::Postgres), Some(db)) => {
                format!("DB {db} / {active_label} ▾")
            }
            _ => format!("DB {active_label} ▾"),
        };
        let entity_for_picker = cx.entity().clone();
        let picker_schemas: Vec<String> = self
            .schemas
            .iter()
            .filter(|s| show_system || !is_system_schema(&s.name))
            .map(|s| s.name.clone())
            .collect();
        let active_for_menu = self.active_schema.clone();

        let db_row = h_flex()
            .w_full()
            .px(px(10.0))
            .py(px(6.0))
            .border_b_1()
            .border_color(cx.theme().border)
            .gap(px(8.0))
            .items_center()
            .child(
                ramag_ui::clickable_button("schema-picker")
                    .ghost()
                    .small()
                    .label(picker_label)
                    .pointer_dropdown_menu_with_anchor(
                        gpui_kit::Anchor::BottomLeft,
                        move |menu, _, _| {
                            let mut m = menu;
                            let entity = entity_for_picker.clone();
                            let active = active_for_menu.clone();
                            for s in &picker_schemas {
                                let s_owned = s.clone();
                                let is_active = active.as_deref() == Some(s.as_str());
                                let label = if is_active {
                                    format!("✓ {s}")
                                } else {
                                    format!("  {s}")
                                };
                                let entity = entity.clone();
                                m = m.item(ramag_ui::menu_item(label).on_click(
                                    move |_, _, app| {
                                        let s = s_owned.clone();
                                        entity.update(app, |this, cx| {
                                            if this.active_schema.as_deref() != Some(s.as_str()) {
                                                this.active_schema = Some(s.clone());
                                                cx.emit(TreeEvent::SchemaActivated { schema: s });
                                                cx.notify();
                                            }
                                        });
                                    },
                                ));
                            }
                            m
                        },
                    ),
            );

        let header_bar = ramag_ui::responsive_toolbar()
            .debug_selector(|| "table-tree-header".into())
            .flex_none()
            .w_full()
            .px(px(10.0))
            .py(px(6.0))
            .border_b_1()
            .border_color(cx.theme().border)
            .gap(px(8.0))
            .child(
                div()
                    .debug_selector(|| "table-tree-search".into())
                    .flex_1()
                    .min_w_0()
                    .child(
                        ramag_ui::cleanable_input(&self.search, "table-search-clear", false, cx)
                            .small()
                            .prefix(Icon::new(IconName::Search).small().text_color(muted_fg)),
                    ),
            )
            .child(filter_button)
            .child(
                ramag_ui::clickable_button("toggle-system")
                    .ghost()
                    .xsmall()
                    .flex_none()
                    .debug_selector(|| "toggle-system".into())
                    .icon(toggle_icon)
                    .tooltip(if show_system {
                        "隐藏系统库"
                    } else {
                        "显示系统库"
                    })
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.toggle_show_system(cx);
                    })),
            )
            .child(
                ramag_ui::clickable_button("refresh-schemas")
                    .ghost()
                    .xsmall()
                    .flex_none()
                    .debug_selector(|| "refresh-schemas".into())
                    .icon(ramag_ui::icons::refresh_cw())
                    .tooltip("刷新")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.refresh(cx);
                    })),
            )
            .child(
                ramag_ui::clickable_button("toggle-query-panel")
                    .ghost()
                    .xsmall()
                    .flex_none()
                    .debug_selector(|| "toggle-query-panel".into())
                    .icon(IconName::SquareTerminal)
                    .selected(qp_visible)
                    .tooltip(if qp_visible {
                        "隐藏编辑器"
                    } else {
                        "显示编辑器"
                    })
                    .on_click(cx.listener(|_this, _: &ClickEvent, _, cx| {
                        cx.emit(TreeEvent::ToggleSqlEditor);
                    })),
            );

        let can_retry_failed = failed_schemas > 0;
        let header_bar =
            if has_filter && search_incomplete && (search_scope_total > 50 || can_retry_failed) {
                if let Some(progress) = self.full_search {
                    header_bar.child(
                        ramag_ui::clickable_button("stop-full-schema-search")
                            .small()
                            .label(format!("停止 {}/{}", progress.completed, progress.total))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.cancel_full_search(cx);
                            })),
                    )
                } else {
                    let retry_only = failed_schemas > 0
                        && searchable_schemas.saturating_add(failed_schemas) == search_scope_total;
                    header_bar.child(
                        ramag_ui::clickable_button("search-all-schemas")
                            .small()
                            .label(if retry_only {
                                "重试失败"
                            } else {
                                "搜索全部"
                            })
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.load_all_tables_for_search(cx);
                            })),
                    )
                }
            } else {
                header_bar
            };
        let header_bar = if self.error.is_some() {
            header_bar.child(
                ramag_ui::clickable_button("retry-schemas")
                    .small()
                    .label("重试")
                    .debug_selector(|| "retry-schemas".into())
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.load_schemas(cx);
                    })),
            )
        } else {
            header_bar
        };

        let transfer_row = ramag_ui::transfer_progress_row(
            "table-transfer-cancel",
            &self.transfer,
            |this: &mut Self| &this.transfer,
            cx,
        );

        let tree_rows_rc = tree_view.rows;
        let body = uniform_list(
            "mysql-tree-rows",
            tree_rows_rc.len(),
            cx.processor({
                let tree_rows_rc = tree_rows_rc.clone();
                move |this, range: Range<usize>, _w, cx| {
                    range
                        .map(|i| this.render_tree_row(&tree_rows_rc[i], cx))
                        .collect::<Vec<_>>()
                }
            }),
        )
        .track_scroll(&self.uniform_scroll)
        .flex_1();

        v_flex()
            .size_full()
            .overflow_hidden()
            .child(render_connection_context(self, muted_fg, cx))
            .child(db_row)
            .child(header_bar)
            .children(transfer_row)
            .child(body)
            .child(
                div()
                    .debug_selector(|| "table-tree-status".into())
                    .flex_none()
                    .w_full()
                    .px_2()
                    .py(px(4.0))
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .text_xs()
                    .text_color(muted_fg)
                    .child(header_text),
            )
            .into_any_element()
    }
}

/// 在对象树顶部显示当前连接上下文，避免用户在切换多个数据库标签后丢失目标。
///
/// 状态来自当前元数据生命周期：首次读取期间显示“连接中”，读取成功后显示“已连接”，
/// 元数据失败时显示“连接失败”。这不是额外的心跳连接，不会为了绘制状态而发起网络请求。
fn render_connection_context(
    panel: &TableTreePanel,
    muted_fg: gpui_kit::Hsla,
    cx: &Context<TableTreePanel>,
) -> impl IntoElement {
    let theme = cx.theme();
    let Some(connection) = panel.connection.as_ref() else {
        return div()
            .id("database-connection-context")
            .debug_selector(|| "database-connection-context".into())
            .h(px(36.0))
            .flex_none()
            .into_any_element();
    };

    let (status, status_color) = if panel.error.is_some() {
        ("连接失败", theme.danger)
    } else if panel.loading_schemas {
        ("连接中", theme.accent)
    } else if panel.schemas.is_empty() {
        ("未加载", muted_fg)
    } else {
        ("已连接", theme.success)
    };
    let kind = match connection.driver {
        DriverKind::Mysql => "MySQL",
        DriverKind::Postgres => "PostgreSQL",
        DriverKind::Sqlite => "SQLite",
        DriverKind::Redis => "Redis",
        DriverKind::Mongodb => "MongoDB",
    };
    let brand_icon = ramag_ui::icons::db_brand_icon(match connection.driver {
        DriverKind::Mysql => "mysql",
        DriverKind::Postgres => "postgres",
        DriverKind::Sqlite => "sqlite",
        DriverKind::Redis => "redis",
        DriverKind::Mongodb => "mongodb",
    });
    let address = format!("{}:{}", connection.host, connection.port);
    let name = connection.name.clone();
    h_flex()
        .id("database-connection-context")
        .debug_selector(|| "database-connection-context".into())
        .w_full()
        .h(px(44.0))
        .flex_none()
        .items_center()
        .gap(px(8.0))
        .px(px(10.0))
        .border_b_1()
        .border_color(theme.border)
        .child(
            div()
                .w(px(20.0))
                .flex_none()
                .flex()
                .justify_center()
                .when_some(brand_icon, |slot, icon| {
                    slot.child(img(icon).size(px(16.0)))
                }),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(1.0))
                .child(
                    div()
                        .debug_selector(|| "database-connection-name".into())
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .text_xs()
                        .text_color(theme.foreground)
                        .child(name),
                )
                .child(
                    div()
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .text_xs()
                        .text_color(muted_fg)
                        .child(format!("{kind} · {address}")),
                ),
        )
        .child(
            h_flex()
                .id("database-connection-status")
                .debug_selector(|| "database-connection-status".into())
                .flex_none()
                .items_center()
                .gap(px(4.0))
                .text_xs()
                .text_color(status_color)
                .child(div().size(px(6.0)).rounded_full().bg(status_color))
                .child(status),
        )
        .into_any_element()
}
