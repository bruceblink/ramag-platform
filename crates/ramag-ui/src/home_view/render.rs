//! 首页筛选、列表和网格的实际渲染与交互。

use super::*;
use gpui_kit::TestSupportExt as _;

impl Render for HomeView {
    /// 按当前搜索、分类与显示方式绘制首页工具，并保留未筛选列表的拖拽排序。
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let search = self.ensure_search(window, cx);
        let tool_manager = match self.tool_manager.clone() {
            Some(manager) => manager,
            None => {
                let manager = create_tool_surface_manager(
                    self.registry.clone(),
                    ToolDragSurface::Home,
                    window,
                    cx,
                );
                self.tool_manager = Some(manager.clone());
                manager
            }
        };
        let theme = cx.theme();
        let muted_fg = theme.muted_foreground;
        let accent = theme.accent;
        let border = theme.border;
        let fg = theme.foreground;
        let palette = crate::pulse_ui::pulse_palette(cx);
        let bg = palette.background;
        let card_bg = palette.surface;
        let card_hover_bg = palette.raised;
        let window_width = f32::from(window.bounds().size.width);
        let compact = window_width < 640.0;
        let card_width = home_card_width(window_width);
        // 共享密度和界面字号共同决定网格卡片尺寸，拖拽计算复用同一几何值。
        let density = crate::content_density(cx);
        let card_padding = home_card_padding_for_density(density);
        let card_height =
            home_card_height_for_density(card_width, density, f32::from(theme.font_size));
        // 紧凑网格正文只占卡片剩余空间；完整说明仍可通过提示和辅助名称读取。
        let compact_grid_description_height =
            (card_height - 2.0 * card_padding - 2.0 - f32::from(theme.font_size) * 1.25 - 8.0)
                .max(0.0);

        let mut accent_border = accent;
        accent_border.a = 0.55;

        let tools = self.registry.home_list();
        let has_tools = !tools.is_empty();
        let filtered_tools = tools
            .iter()
            .filter(|tool| tool_matches_home_filter(tool.meta(), &self.search_query, self.category))
            .cloned()
            .collect::<Vec<_>>();
        let filtering = !self.search_query.is_empty() || self.category != HomeToolCategory::All;
        let can_reorder = !filtering;
        let current_order = tools
            .iter()
            .map(|tool| tool.meta().id.clone())
            .collect::<Vec<_>>();
        let drag_state = tool_drag_state(cx);
        let filtered_order = filtered_tools
            .iter()
            .map(|tool| tool.meta().id.clone())
            .collect::<Vec<_>>();
        let display_slots = if can_reorder {
            tool_drag_display_slots(&current_order, ToolDragSurface::Home, &drag_state)
        } else {
            filtered_order.iter().cloned().map(Some).collect()
        };
        let previous_slots =
            std::mem::replace(&mut self.last_rendered_slots, display_slots.clone());
        let layout_revision = cx.read_global::<ToolLayoutGlobal, _>(|state, _| state.revision);
        let columns = grid_columns(window_width);
        let grid_width = match self.layout {
            HomeLayout::Grid => home_grid_width(columns, card_width),
            HomeLayout::List => home_content_width(window_width).min(TOOL_GRID_WIDTH),
        };
        let item_count = filtered_order.len();
        let (drop_width, drop_height, drop_columns, drop_gap) = match self.layout {
            HomeLayout::Grid => (card_width, card_height, columns, TOOL_CARD_GAP),
            HomeLayout::List => (grid_width, HOME_LIST_ROW_HEIGHT, 1, 0.0),
        };
        let home_layout = HomeDropLayout {
            width: drop_width,
            height: drop_height,
            item_count,
            columns: drop_columns,
            gap: drop_gap,
        };
        let target = drag_state
            .target
            .as_ref()
            .filter(|target| can_reorder && target.surface == ToolDragSurface::Home)
            .map(|target| (target.index, target.side));
        let mut cards = Vec::with_capacity(display_slots.len());
        for slot in &display_slots {
            let Some(id) = slot else {
                continue;
            };

            let Some(tool) = filtered_tools.iter().find(|tool| tool.meta().id == *id) else {
                continue;
            };
            let Some(source_index) = filtered_order.iter().position(|item| item == id) else {
                continue;
            };
            let card_id = SharedString::from(format!("home-tool-{id}"));
            let debug_card_id = card_id.clone();
            let id_for_click = id.clone();
            let name = tool.meta().name.clone();
            let description = tool.meta().description.clone();
            let title_selector = format!("home-tool-title-{id}");
            let description_selector = format!("home-tool-description-{id}");
            let tooltip = format!("{name} — {description}");
            // 紧凑卡片会减少可见正文高度，辅助名称继续提供完整工具说明。
            let accessibility_label = tooltip.clone();
            let icon_selector = format!("home-tool-icon-{id}");
            let debug_icon_selector = icon_selector.clone();
            let icon = ActivityBar::icon_for_meta(tool.meta());
            let preview_icon = icon.clone();
            let preview_name = name.clone();
            let preview_description = description.clone();
            let drag = ToolDrag { id: id.clone() };
            let is_dragged = drag_state.dragged_id.as_deref() == Some(id.as_str());
            let card_background = if is_dragged {
                dragged_item_background(card_bg)
            } else {
                card_bg
            };
            let card_border = if is_dragged {
                accent.opacity(0.78)
            } else {
                border.opacity(0.65)
            };

            let title = div()
                .debug_selector(move || title_selector.clone())
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_sm()
                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                .text_color(fg)
                .child(name);
            let description = div()
                .debug_selector(move || description_selector.clone())
                .min_w_0()
                .overflow_hidden()
                .whitespace_normal()
                .text_xs()
                .line_height(gpui_kit::rems(1.35))
                .text_color(muted_fg)
                .child(description);
            let description = if density.is_compact() && self.layout == HomeLayout::Grid {
                description.max_h(px(compact_grid_description_height))
            } else {
                description
            };
            let icon = h_flex()
                .id(icon_selector)
                .debug_selector(move || debug_icon_selector.clone())
                .size(px(24.0))
                .flex_none()
                .items_center()
                .justify_center()
                .text_color(accent)
                .child(icon);
            let content = match self.layout {
                HomeLayout::Grid => v_flex()
                    .w_full()
                    .min_w_0()
                    .flex_1()
                    .gap(px(8.0))
                    .child(
                        h_flex()
                            .w_full()
                            .min_w_0()
                            .pr(px(14.0))
                            .items_center()
                            .gap(px(8.0))
                            .child(icon)
                            .child(title.flex_1()),
                    )
                    .child(description.w_full().flex_1().min_h_0())
                    .into_any_element(),
                HomeLayout::List => h_flex()
                    .w_full()
                    .min_w_0()
                    .flex_1()
                    .items_center()
                    .gap(px(12.0))
                    .child(icon)
                    .child(
                        v_flex()
                            .min_w_0()
                            .flex_1()
                            .gap(px(3.0))
                            .child(title)
                            .child(description.w_full()),
                    )
                    .into_any_element(),
            };
            let mut card = match self.layout {
                HomeLayout::Grid => v_flex()
                    .w(px(card_width))
                    .h(px(card_height))
                    .p(px(card_padding))
                    .gap(px(8.0))
                    .border_1()
                    .rounded(px(6.0))
                    .shadow_sm(),
                HomeLayout::List => h_flex()
                    .w_full()
                    .h(px(HOME_LIST_ROW_HEIGHT))
                    .min_h(px(HOME_LIST_ROW_HEIGHT))
                    .px(px(12.0))
                    .py(px(12.0))
                    .gap(px(12.0))
                    .border_b_1()
                    .rounded(px(0.0)),
            }
            .id(card_id)
            .debug_selector(move || debug_card_id.to_string())
            .bg(if self.layout == HomeLayout::List {
                bg
            } else {
                card_background
            })
            .border_color(card_border)
            .relative()
            .cursor_pointer()
            .role(gpui_kit::Role::Button)
            .aria_label(accessibility_label)
            .tooltip(move |window, cx| {
                gpui_kit::component::tooltip::Tooltip::new(tooltip.clone()).build(window, cx)
            })
            .hover(move |this| {
                this.border_color(accent_border)
                    .bg(card_hover_bg)
                    .shadow_md()
            })
            .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                cx.emit(HomeEvent::OpenTool(id_for_click.clone()));
            }))
            .child(content);
            if can_reorder {
                card = card
                    .on_drag(drag, move |drag, position, _, cx| {
                        begin_tool_drag(&drag.id, ToolDragSurface::Home, source_index, cx);
                        cx.new(|_| {
                            ToolDragPreview::new(
                                ToolDragSurface::Home,
                                preview_icon.clone(),
                                preview_name.clone(),
                                preview_description.clone(),
                            )
                            .position(position)
                        })
                    })
                    .on_drop(cx.listener(move |this, drag: &ToolDrag, _, cx| {
                        this.complete_drop(&drag.id, source_index, cx);
                    }))
                    .child(match self.layout {
                        HomeLayout::Grid => div()
                            .absolute()
                            .top(px(8.0))
                            .right(px(8.0))
                            .child(tool_drag_handle(accent.opacity(0.65)))
                            .into_any_element(),
                        HomeLayout::List => div()
                            .flex_none()
                            .child(tool_drag_handle(accent.opacity(0.65)))
                            .into_any_element(),
                    });
            }
            if is_dragged {
                card = card.opacity(DRAGGED_ITEM_OPACITY);
            }
            let card = card.test_support();
            let (from_x, from_y) = reorder_animation_offset(
                &previous_slots,
                &display_slots,
                id,
                drop_columns,
                drop_width,
                drop_height,
                drop_gap,
            );
            let card = if from_x == px(0.0) && from_y == px(0.0) {
                card.into_any_element()
            } else {
                EffectTransition::new(REORDER_TRANSITION_DURATION)
                    .ease(ease_in_out_cubic)
                    .slide_x(from_x, px(0.0))
                    .slide_y(from_y, px(0.0))
                    .apply(card, format!("home-tool-reorder-{id}-{layout_revision}"))
                    .into_any_element()
            };
            cards.push(card);
        }

        let layout_for_drop = self.layout;
        let mut tool_grid = div()
            .id("home-tool-grid")
            .debug_selector(|| "home-tool-grid".into())
            .w(px(grid_width))
            .max_w(px(TOOL_GRID_WIDTH))
            .flex()
            .relative();
        tool_grid = match self.layout {
            HomeLayout::Grid => tool_grid
                .flex_row()
                .flex_wrap()
                .justify_start()
                .gap(px(TOOL_CARD_GAP)),
            HomeLayout::List => tool_grid.flex_col().gap(px(0.0)),
        }
        .children(cards);
        if can_reorder {
            tool_grid = tool_grid
                .on_drag_move(
                    cx.listener(move |_, event: &DragMoveEvent<ToolDrag>, _, cx| {
                        let local_x = f32::from(event.event.position.x - event.bounds.left());
                        let local_y = f32::from(event.event.position.y - event.bounds.top());
                        let target = match layout_for_drop {
                            HomeLayout::Grid => home_drop_target_from_position(
                                local_x,
                                local_y,
                                drag_state.source_index,
                                home_layout,
                            ),
                            HomeLayout::List => {
                                home_list_drop_target_from_position(local_y, home_layout)
                            }
                        };
                        if let Some((target_index, side)) = target {
                            update_tool_drop_target(ToolDragSurface::Home, target_index, side, cx);
                        }
                    }),
                )
                .on_drop(cx.listener(move |this, drag: &ToolDrag, _, cx| {
                    this.complete_drop(&drag.id, item_count, cx);
                }));
        }
        if can_reorder
            && let Some((target_index, target_side)) = target
            && let Some(indicator) = home_drop_indicator(
                accent,
                drag_state.source_index,
                target_index,
                target_side,
                home_layout,
            )
        {
            tool_grid = tool_grid.child(
                indicator
                    .id("home-tool-drop-indicator")
                    .on_mouse_move(cx.listener(move |_, _, _, cx| {
                        update_tool_drop_target(
                            ToolDragSurface::Home,
                            target_index,
                            target_side,
                            cx,
                        );
                    }))
                    .on_drop(cx.listener(move |this, drag: &ToolDrag, _, cx| {
                        this.complete_drop(&drag.id, target_index, cx);
                    })),
            );
        }
        if can_reorder && has_tools {
            let drop_end_index = item_count;
            let drop_end_target = drop_end_index - 1;
            tool_grid = tool_grid.child(
                div()
                    .id("home-tool-drop-end")
                    .w_full()
                    .h(px(10.0))
                    .flex_none()
                    .on_mouse_move(cx.listener(move |_, _, _, cx| {
                        update_tool_drop_target(
                            ToolDragSurface::Home,
                            drop_end_target,
                            ToolDropSide::Bottom,
                            cx,
                        );
                    }))
                    .on_drop(cx.listener(move |this, drag: &ToolDrag, _, cx| {
                        this.complete_drop(&drag.id, drop_end_index, cx);
                    })),
            );
        }

        let category_view = cx.entity();
        let selected_category = self.category;
        let category_button = crate::clickable_button("home-category")
            .debug_selector(|| "home-category".into())
            .outline()
            .small()
            .label(selected_category.label())
            .dropdown_caret(true)
            .accessibility_label(format!("工具分类：{}", selected_category.label()))
            .pointer_dropdown_menu(move |mut menu, _, _| {
                for category in HomeToolCategory::ALL {
                    let view = category_view.clone();
                    menu = menu.item(
                        // 标准菜单项会把显示文本作为辅助名称暴露给读屏工具。
                        PopupMenuItem::new(category.label())
                            .checked(category == selected_category)
                            .on_click(move |_, _, app| {
                                view.update(app, |this, cx| {
                                    this.category = category;
                                    this.last_rendered_slots.clear();
                                    cx.notify();
                                });
                            }),
                    );
                }
                menu
            });
        let grid_view = cx.entity();
        // 选中按钮使用共享 muted 中性色，避免把布局状态绘制成强调色蓝底。
        let neutral_selected_bg = theme.muted;
        let selected_fg = theme.foreground;
        let grid_toggle = div()
            .id("home-layout-grid")
            .debug_selector(|| "home-layout-grid".into())
            .child(
                Toggle::new("home-layout-grid-toggle")
                    .icon(Icon::new(gpui_kit::assets::IconName::LayoutGrid))
                    .checked(self.layout == HomeLayout::Grid)
                    .when(self.layout == HomeLayout::Grid, |toggle| {
                        toggle.bg(neutral_selected_bg).text_color(selected_fg)
                    })
                    .tooltip("网格视图")
                    .on_click(move |_, _, app| {
                        grid_view.update(app, |this, cx| {
                            this.layout = HomeLayout::Grid;
                            this.last_rendered_slots.clear();
                            cx.notify();
                        });
                    }),
            );
        let list_view = cx.entity();
        let list_toggle = div()
            .id("home-layout-list")
            .debug_selector(|| "home-layout-list".into())
            .child(
                Toggle::new("home-layout-list-toggle")
                    .icon(Icon::new(gpui_kit::assets::IconName::List))
                    .checked(self.layout == HomeLayout::List)
                    .when(self.layout == HomeLayout::List, |toggle| {
                        toggle.bg(neutral_selected_bg).text_color(selected_fg)
                    })
                    .tooltip("列表视图")
                    .on_click(move |_, _, app| {
                        list_view.update(app, |this, cx| {
                            this.layout = HomeLayout::List;
                            this.last_rendered_slots.clear();
                            cx.notify();
                        });
                    }),
            );
        let search_input = crate::cleanable_input(&search, "home-search-clear", false, cx)
            .small()
            .prefix(Icon::new(IconName::Search).small().text_color(muted_fg));
        let toolbar = h_flex()
            .id("home-toolbar")
            .debug_selector(|| "home-toolbar".into())
            .w(px(grid_width))
            .min_w_0()
            .flex_wrap()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .id("home-search")
                    .debug_selector(|| "home-search".into())
                    .aria_label("搜索工具")
                    .flex_1()
                    .min_w(px(180.0))
                    .child(search_input),
            )
            .child(category_button)
            .child(
                h_flex()
                    .id("home-layout-control")
                    .debug_selector(|| "home-layout-control".into())
                    .items_center()
                    .gap(px(2.0))
                    .child(grid_toggle)
                    .child(list_toggle),
            );
        let filtered_count = filtered_tools.len();
        let filter_empty = filtering && filtered_count == 0;
        let clear_view = cx.entity();
        let clear_filters = crate::clickable_button("home-filter-clear")
            .debug_selector(|| "home-filter-clear".into())
            .ghost()
            .small()
            .label("清除搜索和筛选")
            .on_click(move |_, window, cx| {
                clear_view.update(cx, |this, cx| this.clear_filters(window, cx));
            });

        v_flex()
            .id("home-view")
            .debug_selector(|| "home-view".into())
            .size_full()
            .bg(bg)
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .vertical_scrollbar(&self.scroll)
            .items_center()
            .on_mouse_up(MouseButton::Left, |_, _, cx| clear_tool_drag(cx))
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(TOOL_GRID_WIDTH + home_content_padding(window_width) * 2.0))
                    .p(px(home_content_padding(window_width)))
                    .gap(px(if compact { 16.0 } else { 24.0 }))
                    .items_center()
                    .child(render_home_header(tools.len(), tool_manager, grid_width, cx))
                    .child(toolbar)
                    .child(
                        h_flex()
                            .id("home-tool-count")
                            .debug_selector(|| "home-tool-count".into())
                            .w(px(grid_width))
                            .justify_between()
                            .items_center()
                            .child(div().text_sm().child("全部工具"))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted_fg)
                                    .child(format!("{filtered_count} 个工具")),
                            ),
                    )
                    .when(filter_empty, |content| {
                        content.child(
                            v_flex()
                                .id("home-filter-empty")
                                .debug_selector(|| "home-filter-empty".into())
                                .w(px(grid_width))
                                .items_center()
                                .gap(px(8.0))
                                .p(px(24.0))
                                .border_1()
                                .border_color(border.opacity(0.65))
                                .rounded(px(6.0))
                                .child(div().text_sm().child("没有匹配的工具。"))
                                .child(clear_filters),
                        )
                    })
                    .when(!filter_empty, |content| content.child(tool_grid))
                    .when(filtering && !filtered_tools.is_empty(), |content| {
                        content.child(
                            div()
                                .id("home-filter-sort-hint")
                                .debug_selector(|| "home-filter-sort-hint".into())
                                .w(px(grid_width))
                                .text_xs()
                                .text_color(muted_fg)
                                .child("筛选期间暂停排序；清除筛选后可拖拽排序。"),
                        )
                    })
                    .when(!has_tools, |content| {
                        content.child(crate::pulse_ui::pulse_status_notice(
                            crate::pulse_ui::PulseStatus::Unavailable,
                            "首页没有显示工具。使用右上角的“管理工具”恢复入口，也可在设置的“工具显示”页管理。",
                            cx,
                        ))
                    }),
            )
    }
}
