use super::*;
impl WorkspaceView {
    pub(crate) fn screen_notice(&self) -> String {
        if let Some(rejected) = &self.shared.borrow().session.rejected {
            format!(
                "Saved settings could not be restored: {}. Original input retained; autosave is paused.",
                rejected.error
            )
        } else {
            self.notice.clone()
        }
    }
}

/// Keep scroll clipping bounds in the native accessibility ancestry.
pub(crate) fn scroll_viewport(id: &'static str, native_id: String) -> Stateful<Div> {
    div()
        .id(id)
        .role(Role::ScrollView)
        .accessibility_id(native_id)
}

#[cfg(test)]
mod viewport_accessibility_tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn scroll_viewports_expose_stable_native_geometry_nodes() {
        for (internal, native) in [
            ("workspace-scroll", "workspace:viewport"),
            ("process-table-viewport", "processes:viewport"),
            ("sensor-scroll", "cpu:host:viewport"),
            ("process-row-clip", "processes:rows-viewport"),
        ] {
            let element = scroll_viewport(internal, native.into());
            assert_eq!(element.a11y_role(), Some(Role::ScrollView));
            let mut node = gpui_kit::accesskit::Node::new(Role::ScrollView);
            element.write_a11y_info(&mut node);
            assert_eq!(node.author_id(), Some(native));
        }
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let data = self.shared.borrow();
        let monitor_commands = data
            .catalog
            .iter()
            .map(|m| {
                let visible = data.session.workspace.panels[&m.id].visible;
                (
                    format!("{} {}", if visible { "Hide" } else { "Show" }, m.title),
                    Command::PanelVisible(m.id.clone()),
                )
            })
            .collect::<Vec<_>>();
        let mut commands = vec![
            ("Settings & presets".into(), Command::ShowSettings),
            ("Reset layout".into(), Command::AskResetLayout),
            ("Save".into(), Command::Save),
            ("Save preset".into(), Command::SavePreset),
            ("Recall preset".into(), Command::RecallPreset),
        ];
        let selected_interval = data.session.workspace.interval_ms;
        commands.extend([500, 1000, 2000, 5000].map(|ms| {
            (
                format!(
                    "{}{} s",
                    if ms == selected_interval { "✓ " } else { "" },
                    ms as f64 / 1000.
                ),
                Command::Interval(ms),
            )
        }));
        let mut message = self.notice.clone();
        if let Some(rejected) = &data.session.rejected {
            message = format!(
                "Saved layout rejected: {}. Original input retained; autosave blocked. {}",
                rejected.error, self.notice
            );
            commands.push(("Accept recovered layout".into(), Command::Recover));
        }
        let scroll = data.scroll.clone();
        drop(data);
        let extent = self.dock.read(cx).bounds().size;
        let toolbar = div().flex().flex_wrap().gap_1().children(
            commands
                .into_iter()
                .map(|(label, command)| command_button(label, command, cx)),
        );
        let visibility_scroll = self.visibility_scroll.clone();
        let visibility =
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .children(monitor_commands.into_iter().map(|(label, command)| {
                    let entry = self
                        .visibility_controls
                        .entry(command.control_id())
                        .or_insert_with(|| crate::controls::FocusEntry::new(cx))
                        .clone();
                    let scroll = visibility_scroll.clone();
                    command_button(label, command, cx)
                        .track_focus(&entry.handle)
                        .on_prepaint(move |bounds, window, _| {
                            if entry.entered(window) {
                                crate::controls::reveal(bounds.dilate(px(1.)), &scroll);
                                window.refresh();
                            }
                        })
                }));
        let menu_shared = self.shared.clone();
        div().id("workspace-context").size_full().flex().flex_col().gap_2().p_2().bg(cx.theme().background)
            .font_family(cx.theme().font_family.clone()).text_color(cx.theme().foreground).track_focus(&self.focus).tab_group()
              .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                  if this.confirm_layout_reset && event.keystroke.key == "escape" {
                      this.command(Command::CancelResetLayout, window, cx);
                      cx.stop_propagation();
                      return;
                  }
                if event.keystroke.modifiers.alt {
                    let command = match event.keystroke.key.as_str() {
                        "pageup" => Command::Scroll(0., 300.), "pagedown" => Command::Scroll(0., -300.),
                        "left" => Command::Scroll(300., 0.), "right" => Command::Scroll(-300., 0.), _ => return,
                    };
                    this.command(command, window, cx); cx.stop_propagation();
                }
            }))
            .child(div().flex().flex_wrap().items_center().gap_3()
                .child(div().id("workspace-title").text_lg().font_weight(FontWeight::SEMIBOLD).child("System Pulse")
                    .context_menu(move |menu, _, _| crate::panel_context::workspace(menu, &menu_shared)))
                  .child(toolbar))
              .when(self.confirm_layout_reset, |el| el.child(
                  div().id("layout-reset-confirmation").flex().flex_col().gap_1().p_2().bg(cx.theme().muted)
                      .child("Restore the default panel arrangement? Sensor choices, appearance and saved presets will be kept.")
                      .child(div().flex().gap_2()
                          .child(command_button("Cancel".into(), Command::CancelResetLayout, cx).track_focus(&self.cancel_layout_reset_focus))
                          .child(command_button("Restore default layout".into(), Command::ResetLayout, cx)))))
            .child(div().h(px(64.)).flex_none().relative()
                .child(div().id("visibility-controls").size_full().overflow_y_scroll().track_scroll(&self.visibility_scroll).child(visibility))
                .child(Scrollbar::vertical(&self.visibility_scroll).mode(ScrollbarMode::Always)))
            .when(!message.is_empty(), |el| el.child(div().text_sm().child(message)))
            .child(div().flex_1().min_h_0().min_w_0().relative()
                .child(scroll_viewport("workspace-scroll", "workspace:viewport".into()).size_full().overflow_scroll().track_scroll(&scroll)
                    .debug_selector(|| "workspace-viewport".into())
                    .child(div().w(extent.width).h(extent.height).min_w_full().child(self.dock.clone())))
                .child(Scrollbar::new(&scroll).mode(ScrollbarMode::Always)))
            .child(div().text_xs().text_color(cx.theme().muted_foreground).child("Alt+PageUp/PageDown: scroll workspace · Alt+Left/Right: horizontal · Tab: next control"))
    }
}

pub(crate) fn initial_catalog(fixture_mode: bool) -> Vec<Monitor> {
    #[cfg(test)]
    if fixture_mode {
        return fixture::catalog();
    }
    let _ = fixture_mode;
    live::presentations()
}

pub(crate) fn restore_session(raw: &str, fallback: Workspace, allow_fixture: bool) -> Session {
    let mut session = Session::restore(raw, fallback.clone(), |value| {
        validate_dock_mode(value, allow_fixture)
    });
    if !allow_fixture
        && session.rejected.is_none()
        && (session
            .workspace
            .panels
            .keys()
            .any(|id| live::is_fixture_id(id))
            || session
                .workspace
                .monitors
                .keys()
                .any(|id| live::is_fixture_id(id)))
    {
        session = Session {
            workspace: fallback,
            rejected: Some(system_pulse_model::RejectedInput {
                original: raw.into(),
                error: "Saved workspace contains simulated device identities".into(),
            }),
        };
    }
    // Saved dock leaves may precede persisted presentation metadata.
    fn collect(value: &serde_json::Value, ids: &mut Vec<String>) {
        if let Some(id) = value.get("monitor_id").and_then(serde_json::Value::as_str) {
            ids.push(id.into());
        }
        match value {
            serde_json::Value::Object(map) => {
                for value in map.values() {
                    collect(value, ids);
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    collect(value, ids);
                }
            }
            _ => {}
        }
    }
    let mut ids = Vec::new();
    collect(&session.workspace.dock, &mut ids);
    for id in ids {
        session.workspace.panel_mut(&id);
    }
    session
}

#[cfg(test)]
mod live_tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn live_restore_rejects_copied_fixture_workspace_and_preserves_original() {
        let mut copied = Workspace::new(serde_json::to_value(default_dock()).unwrap());
        fixture::discover(&mut copied, &fixture::catalog());
        let raw = serde_json::to_string(&copied).unwrap();
        let fallback =
            Workspace::new(serde_json::to_value(default_dock_for(&live::presentations())).unwrap());
        let restored = restore_session(&raw, fallback.clone(), false);
        assert_eq!(restored.rejected.as_ref().unwrap().original, raw);
        assert!(restored.autosave_json().is_err());
        assert_eq!(restored.workspace.dock, fallback.dock);
        assert!(
            restored
                .workspace
                .panels
                .keys()
                .all(|id| !live::is_fixture_id(id))
        );
        let mut panels_only = fallback.clone();
        panels_only.panel_mut("gpu:fixture-a");
        assert!(
            restore_session(
                &serde_json::to_string(&panels_only).unwrap(),
                fallback,
                false
            )
            .rejected
            .is_some()
        );
    }
    #[::core::prelude::v1::test]
    fn absent_real_identity_is_valid_before_discovery() {
        let missing = Monitor {
            id: "nvidia:GPU-absent".into(),
            title: "Saved GPU".into(),
            summary: "usage".into(),
            sensors: vec![],
        };
        let dock = serde_json::to_value(default_dock_for(&[missing])).unwrap();
        assert!(validate_dock(&dock).is_ok());
        let workspace = Workspace::new(dock);
        let restored = restore_session(
            &serde_json::to_string(&workspace).unwrap(),
            workspace,
            false,
        );
        assert!(restored.rejected.is_none());
        assert!(restored.workspace.panels.contains_key("nvidia:GPU-absent"));
        assert!(
            live::catalog(&restored.workspace)
                .iter()
                .any(|m| m.id == "nvidia:GPU-absent")
        );
    }
}

fn command_button(label: String, command: Command, cx: &Context<WorkspaceView>) -> Button {
    let selector = command.control_id();
    Button::new(SharedString::from(command.control_id()))
        .debug_selector(move || selector.clone())
        .accessibility_label(label.clone())
        .child(label)
        .px_2()
        .h_7()
        .text_sm()
        .border_1()
        .rounded(cx.theme().radius)
        .border_color(cx.theme().border)
        .focus_visible(|style| style.border_color(cx.theme().ring))
        .on_click(cx.listener(move |this, _, window, cx| this.command(command.clone(), window, cx)))
}
