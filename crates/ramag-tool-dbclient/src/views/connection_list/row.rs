//! 单行数据库连接。

use gpui_kit::component::{
    ActiveTheme as _, Sizable as _,
    button::ButtonVariants as _,
    h_flex,
    menu::{ContextMenuExt as _, PopupMenu},
    v_flex,
};
use gpui_kit::{
    ClickEvent, Context, IntoElement, ParentElement, SharedString, Styled, div, img, prelude::*, px,
};
use ramag_domain::entities::{ConnectionConfig, DriverKind};

use super::{ConnectionListPanel, ListEvent};

#[derive(Clone, Copy, PartialEq)]
pub(super) enum RowDensity {
    Full,
    Medium,
    Narrow,
}

const KIND_WIDTH: f32 = 104.0;
const ADDRESS_WIDTH: f32 = 176.0;
const ACCOUNT_WIDTH: f32 = 176.0;
const ACTIONS_WIDTH: f32 = 108.0;

/// Keep the fixed heading and virtual rows on the same responsive column boundaries.
pub(super) fn connection_header(density: RowDensity, cx: &gpui_kit::App) -> gpui_kit::Div {
    h_flex()
        .debug_selector(|| "connection-list-column-header".into())
        .w_full()
        .flex_none()
        .h(px(32.0))
        .items_center()
        .px(px(14.0))
        .gap(px(12.0))
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .border_b_1()
        .border_color(cx.theme().border)
        .child(div().flex_1().min_w_0().pl(px(32.0)).child("数据源"))
        .when(density != RowDensity::Narrow, |header| {
            header.child(
                div()
                    .debug_selector(|| "connection-header-kind".into())
                    .flex_none()
                    .w(px(KIND_WIDTH))
                    .child("类型 / 版本"),
            )
        })
        .when(density != RowDensity::Narrow, |header| {
            header.child(
                div()
                    .debug_selector(|| "connection-header-address".into())
                    .flex_none()
                    .w(px(ADDRESS_WIDTH))
                    .child("地址"),
            )
        })
        .when(density == RowDensity::Full, |header| {
            header.child(
                div()
                    .debug_selector(|| "connection-header-account".into())
                    .flex_none()
                    .w(px(ACCOUNT_WIDTH))
                    .child("账号 / 数据库"),
            )
        })
        .child(
            div()
                .debug_selector(|| "connection-header-actions".into())
                .flex_none()
                .w(px(ACTIONS_WIDTH))
                .flex()
                .justify_end()
                .child("操作"),
        )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn connection_row(
    idx: usize,
    conn: ConnectionConfig,
    is_selected: bool,
    show_sync: bool,
    version: Option<String>,
    density: RowDensity,
    border: gpui_kit::Hsla,
    hover_bg: gpui_kit::Hsla,
    accent: gpui_kit::Hsla,
    fg: gpui_kit::Hsla,
    muted_fg: gpui_kit::Hsla,
    cx: &mut Context<ConnectionListPanel>,
) -> impl IntoElement {
    let show_address = density != RowDensity::Narrow;
    let show_account = density == RowDensity::Full;
    let kind_label = match conn.driver {
        DriverKind::Mysql => "MySQL",
        DriverKind::Postgres => "PostgreSQL",
        DriverKind::Sqlite => "SQLite",
        DriverKind::Redis => "Redis",
        DriverKind::Mongodb => "MongoDB",
    };

    let brand_icon: Option<&'static str> = ramag_ui::icons::db_brand_icon(match conn.driver {
        DriverKind::Mysql => "mysql",
        DriverKind::Postgres => "postgres",
        DriverKind::Sqlite => "sqlite",
        DriverKind::Redis => "redis",
        DriverKind::Mongodb => "mongodb",
    });

    let badge_fg: gpui_kit::Hsla = match conn.driver {
        DriverKind::Mysql => accent,
        DriverKind::Postgres => gpui_kit::hsla(265.0 / 360.0, 0.55, 0.55, 1.0),
        DriverKind::Sqlite => gpui_kit::hsla(200.0 / 360.0, 0.60, 0.50, 1.0),
        DriverKind::Redis => gpui_kit::hsla(0.0, 0.65, 0.55, 1.0),
        DriverKind::Mongodb => gpui_kit::hsla(140.0 / 360.0, 0.55, 0.45, 1.0),
    };
    let mut badge_bg = badge_fg;
    badge_bg.a = 0.12;

    let row_id = SharedString::from(format!("conn-row-{}", conn.id));
    let edit_id = SharedString::from(format!("conn-edit-{}", conn.id));
    let sync_id = SharedString::from(format!("conn-sync-{}", conn.id));
    let del_id = SharedString::from(format!("conn-del-{}", conn.id));

    let conn_for_open = conn.clone();
    let conn_for_edit = conn.clone();
    let conn_for_sync = conn.clone();
    let conn_for_duplicate = conn.clone();
    let conn_id_for_del = conn.id.clone();
    let entity_for_menu = cx.entity().clone();
    let is_production = conn.production;
    let environment = conn.environment.clone().unwrap_or_default();

    let host_port = if conn.driver == DriverKind::Sqlite {
        conn.host.clone()
    } else {
        format!("{}:{}", conn.host, conn.port)
    };

    let name_collapsed_with_host = conn.name == conn.host;
    let primary_label = if name_collapsed_with_host && density == RowDensity::Narrow {
        host_port.clone()
    } else {
        conn.name.clone()
    };
    let address_text = host_port.clone();

    let account_text = {
        let user = conn.username.trim();
        let db = conn.database.as_deref().map(str::trim).unwrap_or("");
        match (user.is_empty(), db.is_empty()) {
            (false, false) => format!("{user} @ {db}"),
            (false, true) => user.to_string(),
            (true, false) => db.to_string(),
            (true, true) => String::new(),
        }
    };

    let version_text = version.unwrap_or_default();

    let secondary_col = move |selector: String, w: f32, text: String| {
        let tooltip = text.clone();
        div()
            .id(SharedString::from(selector.clone()))
            .debug_selector(move || selector.clone())
            .flex_none()
            .w(px(w))
            .text_sm()
            .text_color(muted_fg)
            .overflow_hidden()
            .text_ellipsis()
            .whitespace_nowrap()
            .tooltip(move |window, cx| {
                gpui_kit::component::tooltip::Tooltip::new(tooltip.clone()).build(window, cx)
            })
            .child(if text.is_empty() { "—".into() } else { text })
    };

    let danger = cx.theme().danger;
    let success = cx.theme().success;
    let warning = cx.theme().warning;
    let mut prod_bg = danger;
    prod_bg.a = 0.15;

    let key = conn.id.to_string();
    let name_selector = format!("connection-row-name-{key}");
    let actions_selector = format!("connection-row-actions-{key}");
    let metadata_selector = format!("connection-row-metadata-{key}");
    let row_selector = format!("connection-row-{key}");
    let name_tooltip = format!("{}\n{}", conn.name, host_port);
    let brand = div()
        .flex_none()
        .w(px(24.0))
        .flex()
        .justify_center()
        .when_some(brand_icon, |slot, icon| {
            slot.child(img(icon).size(px(18.0)).flex_none())
        });
    let name = div()
        .id(SharedString::from(name_selector.clone()))
        .debug_selector(move || name_selector.clone())
        .flex_1()
        .min_w_0()
        .text_sm()
        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
        .text_color(fg)
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .tooltip(move |window, cx| {
            gpui_kit::component::tooltip::Tooltip::new(name_tooltip.clone()).build(window, cx)
        })
        .child(primary_label);
    let environment_badge = (!environment.trim().is_empty()).then(|| {
        let (env_fg, env_bg) =
            environment_badge_colors(&environment, muted_fg, success, warning, danger);
        let tooltip = environment.clone();
        div()
            .id(SharedString::from(format!("connection-environment-{key}")))
            .min_w_0()
            .px(px(6.0))
            .py(px(1.0))
            .rounded(px(4.0))
            .text_xs()
            .text_color(env_fg)
            .bg(env_bg)
            .max_w_full()
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .tooltip(move |window, cx| {
                gpui_kit::component::tooltip::Tooltip::new(tooltip.clone()).build(window, cx)
            })
            .child(environment)
    });
    let driver_badge = div()
        .flex_none()
        .px(px(8.0))
        .py(px(2.0))
        .rounded(px(4.0))
        .text_xs()
        .text_color(badge_fg)
        .bg(badge_bg)
        .child(kind_label);
    let production_badge = is_production.then(|| {
        div()
            .flex_none()
            .px(px(6.0))
            .py(px(1.0))
            .rounded(px(4.0))
            .text_xs()
            .text_color(danger)
            .bg(prod_bg)
            .child(ramag_ui::PRODUCTION_BADGE_LABEL)
    });
    let actions = h_flex()
        .debug_selector(move || actions_selector.clone())
        .flex_none()
        .gap(px(4.0))
        .w(px(ACTIONS_WIDTH))
        .justify_end()
        .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
            cx.stop_propagation()
        })
        .when(show_sync, |actions| {
            actions.child(
                ramag_ui::clickable_button(sync_id)
                    .debug_selector(move || format!("connection-row-sync-{key}"))
                    .ghost()
                    .small()
                    .icon(ramag_ui::icons::database_sync())
                    .tooltip("数据同步")
                    .on_click(cx.listener(move |_this, _: &ClickEvent, _, cx| {
                        cx.emit(ListEvent::RequestSync(conn_for_sync.clone()));
                    })),
            )
        })
        .child(
            ramag_ui::clickable_button(edit_id)
                .ghost()
                .small()
                .icon(ramag_ui::icons::pencil())
                .tooltip("编辑")
                .on_click(cx.listener(move |_this, _: &ClickEvent, _, cx| {
                    cx.emit(ListEvent::RequestEdit(conn_for_edit.clone()));
                })),
        )
        .child(
            ramag_ui::clickable_button(del_id)
                .ghost()
                .small()
                .icon(ramag_ui::icons::trash())
                .tooltip("删除")
                .on_click(cx.listener(move |_this, _: &ClickEvent, _, cx| {
                    cx.emit(ListEvent::RequestDelete(conn_id_for_del.clone()));
                })),
        );

    // UniformList requires a consistent row height: narrow rows always reserve one metadata line.
    let narrow = density == RowDensity::Narrow;
    let row = if narrow { v_flex() } else { h_flex() };
    let mut row = row
        .id(row_id)
        .debug_selector(move || row_selector.clone())
        .w_full()
        .min_w_0()
        .h(px(if narrow { 74.0 } else { 56.0 }))
        .bg(hover_bg.opacity(if idx.is_multiple_of(2) { 0.18 } else { 0.0 }))
        .items_center()
        .gap(px(if narrow { 4.0 } else { 12.0 }))
        .px(px(14.0))
        .py(px(8.0))
        .border_b_1()
        .border_color(border)
        .cursor_pointer()
        .hover(move |this| this.bg(hover_bg))
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.handle_click(conn_for_open.clone(), cx);
        }));
    if narrow {
        row = row
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .gap(px(8.0))
                    .items_center()
                    .child(brand)
                    .child(name)
                    .child(actions),
            )
            .child(
                h_flex()
                    .debug_selector(move || metadata_selector.clone())
                    .w_full()
                    .min_w_0()
                    .h(px(24.0))
                    .pl(px(32.0))
                    .gap(px(8.0))
                    .items_center()
                    .child(driver_badge)
                    .when_some(environment_badge, |line, badge| {
                        line.child(div().flex_1().min_w_0().child(badge))
                    })
                    .when_some(production_badge, |line, badge| line.child(badge)),
            );
    } else {
        row = row
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(8.0))
                    .items_center()
                    .child(brand)
                    .child(v_flex().flex_1().min_w_0().gap(px(2.0)).child(name).when(
                        environment_badge.is_some() || production_badge.is_some(),
                        |cell| {
                            cell.child(
                                h_flex()
                                    .min_w_0()
                                    .gap(px(6.0))
                                    .when_some(environment_badge, |line, badge| {
                                        line.child(div().min_w_0().max_w(px(160.0)).child(badge))
                                    })
                                    .when_some(production_badge, |line, badge| line.child(badge)),
                            )
                        },
                    )),
            )
            .child(
                v_flex()
                    .debug_selector(move || format!("connection-row-kind-{}", conn.id))
                    .flex_none()
                    .w(px(KIND_WIDTH))
                    .gap(px(2.0))
                    .child(driver_badge)
                    .when(!version_text.is_empty(), |cell| {
                        cell.child(
                            div()
                                .min_w_0()
                                .text_xs()
                                .text_color(muted_fg)
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(version_text),
                        )
                    }),
            )
            .when(show_address, |row| {
                row.child(secondary_col(
                    format!("connection-row-address-{}", conn_for_duplicate.id),
                    ADDRESS_WIDTH,
                    address_text,
                ))
            })
            .when(show_account, |row| {
                row.child(secondary_col(
                    format!("connection-row-account-{}", conn_for_duplicate.id),
                    ACCOUNT_WIDTH,
                    account_text,
                ))
            })
            .child(actions);
    }

    if is_selected {
        let mut sel_bg = accent;
        sel_bg.a = 0.12;
        row = row.bg(sel_bg);
    }

    row.context_menu(move |menu: PopupMenu, _, _| {
        let entity = entity_for_menu.clone();
        let connection = conn_for_duplicate.clone();
        menu.item(ramag_ui::menu_item("Duplicate").on_click(move |_, _, app| {
            entity.update(app, |_this, cx| {
                cx.emit(ListEvent::RequestDuplicate(connection.clone()));
            });
        }))
    })
}

fn environment_badge_colors(
    environment: &str,
    fallback_fg: gpui_kit::Hsla,
    success: gpui_kit::Hsla,
    warning: gpui_kit::Hsla,
    danger: gpui_kit::Hsla,
) -> (gpui_kit::Hsla, gpui_kit::Hsla) {
    let fg = match environment.trim().to_ascii_lowercase().as_str() {
        "dev" => success,
        "test" => warning,
        "prod" => danger,
        _ => fallback_fg,
    };
    let mut bg = fg;
    bg.a = 0.12;
    (fg, bg)
}

#[cfg(test)]
mod tests {
    use super::environment_badge_colors;

    #[test]
    fn environment_badges_use_theme_status_colors() {
        let fallback = gpui_kit::hsla(0.1, 0.2, 0.3, 1.0);
        let success = gpui_kit::hsla(0.2, 0.3, 0.4, 1.0);
        let warning = gpui_kit::hsla(0.3, 0.4, 0.5, 1.0);
        let danger = gpui_kit::hsla(0.4, 0.5, 0.6, 1.0);

        assert_eq!(
            environment_badge_colors("dev", fallback, success, warning, danger).0,
            success
        );
        assert_eq!(
            environment_badge_colors("test", fallback, success, warning, danger).0,
            warning
        );
        assert_eq!(
            environment_badge_colors("PROD", fallback, success, warning, danger).0,
            danger
        );
        assert_eq!(
            environment_badge_colors("custom", fallback, success, warning, danger).0,
            fallback
        );
    }
}
