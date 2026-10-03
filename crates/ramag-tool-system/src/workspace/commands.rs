use super::*;
impl WorkspaceView {
    pub(crate) fn command(
        &mut self,
        command: Command,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(
            &command,
            Command::PanelVisible(_)
                | Command::PanelCollapse(_)
                | Command::Save
                | Command::SavePreset
                | Command::RecallPreset
                | Command::Preset(_)
        ) {
            self.initial_layout_pending = false;
        }
        match command {
            #[cfg(target_os = "windows")]
            Command::EnableCpuTemperatures => {
                self.notice = match self.service.as_ref() {
                    Some(service) => service.enable_cpu_temperatures().err().unwrap_or_default(),
                    None => "CPU temperature access requires the live collector.".into(),
                };
                self.notify_panels(cx);
                cx.notify();
                return;
            }
            #[cfg(target_os = "windows")]
            Command::DisableCpuTemperatures => {
                if let Some(service) = self.service.as_ref() {
                    service.disable_cpu_temperatures();
                }
                self.notice.clear();
                self.notify_panels(cx);
                cx.notify();
                return;
            }
            Command::Screen(screen) => {
                self.shared.borrow_mut().session.workspace.screens.active = screen;
            }
            Command::ScreenDevice(screen, id) => {
                let valid = crate::screen_data::devices(&self.shared.borrow(), screen)
                    .iter()
                    .any(|choice| choice.id == id);
                if !valid {
                    self.notice = "The selected device is no longer available.".into();
                    cx.notify();
                    return;
                }
                self.shared
                    .borrow_mut()
                    .session
                    .workspace
                    .screens
                    .devices
                    .insert(screen, id);
            }
            Command::AskResetLayout => {
                self.confirm_layout_reset = true;
                self.cancel_layout_reset_focus.focus(window, cx);
                cx.notify();
                return;
            }
            Command::CancelResetLayout => {
                self.confirm_layout_reset = false;
                self.focus.focus(window, cx);
                cx.notify();
                return;
            }
            Command::ResetLayout => {
                if !std::mem::take(&mut self.confirm_layout_reset) {
                    return;
                }
                self.record(cx);
                let restored = {
                    let data = self.shared.borrow();
                    crate::layout::reset(&data.session.workspace, &data.catalog)
                };
                let initial_layout_pending = self.initial_layout_pending;
                self.restore(
                    &serde_json::to_string(&restored).unwrap_or_else(|error| {
                        eprintln!("Restored workspace serialization failed: {error}");
                        String::new()
                    }),
                    window,
                    cx,
                );
                self.initial_layout_pending = initial_layout_pending;
                self.shared
                    .borrow()
                    .scroll
                    .set_offset(point(px(0.), px(0.)));
                self.focus.focus(window, cx);
                if !self.read_blocked {
                    self.notice = "Default layout restored".into();
                }
            }
            Command::ShowSettings => {
                if !self.shared.borrow().session.workspace.panels["settings"].visible {
                    self.command(Command::PanelVisible("settings".into()), window, cx);
                }
                let panel = self
                    .shared
                    .borrow()
                    .views
                    .get("settings")
                    .and_then(|view| view.upgrade());
                if let Some(panel) = panel {
                    panel.read(cx).controls["collapse"]
                        .handle
                        .clone()
                        .focus(window, cx);
                }
                cx.on_next_frame(window, |this, window, cx| {
                    let data = this.shared.borrow();
                    if let Some(bounds) = data.bounds.get("settings") {
                        crate::controls::reveal(*bounds, &data.scroll);
                    }
                    window.refresh();
                    cx.notify();
                });
                cx.notify();
                return;
            }
            Command::Scroll(dx, dy) => {
                let scroll = self.shared.borrow().scroll.clone();
                let max = scroll.max_offset();
                let old = scroll.offset();
                scroll.set_offset(point(
                    (old.x + px(dx)).clamp(-max.x, px(0.)),
                    (old.y + px(dy)).clamp(-max.y, px(0.)),
                ));
                self.request_keyboard_repaint(window, cx);
                return;
            }
            Command::PanelCollapse(id) => {
                self.capture_sizes(cx);
                let view = self.shared.borrow().views.get(&id).cloned();
                if let Some(view) = view {
                    let _ =
                        view.update(cx, |p, cx| p.controls["collapse"].handle.focus(window, cx));
                }
                let mut data = self.shared.borrow_mut();
                let panel = data.session.workspace.panel_mut(&id);
                panel.collapsed = !panel.collapsed;
                drop(data);
            }
            Command::PanelVisible(id) => {
                self.capture_sizes(cx);
                let mut data = self.shared.borrow_mut();
                let panel = data.session.workspace.panel_mut(&id);
                panel.visible = !panel.visible;
                let visible = panel.visible;
                let old = data.views.get(&id).and_then(WeakEntity::upgrade);
                drop(data);
                if visible {
                    let monitor = self
                        .shared
                        .borrow()
                        .catalog
                        .iter()
                        .find(|m| m.id == id)
                        .cloned();
                    if let Some(monitor) = monitor {
                        let panel = old.unwrap_or_else(|| {
                            cx.new(|cx| MonitorPanel::new(monitor, self.shared.clone(), cx))
                        });
                        self.shared.borrow_mut().views.insert(id, panel.downgrade());
                        self.dock.update(cx, |dock, cx| {
                            dock.add_panel(panel, DockPlacement::Center, Some(px(280.)), window, cx)
                        });
                    }
                } else if let Some(panel) = old {
                    self.focus.focus(window, cx);
                    self.dock
                        .update(cx, |dock, cx| dock.remove_panel(panel, window, cx));
                }
            }
            Command::RowCollapse(id, sensor) => {
                let view = self.shared.borrow().views.get(&id).cloned();
                if let Some(view) = view {
                    let _ = view.update(cx, |p, cx| {
                        p.controls[&format!("row:{sensor}")]
                            .handle
                            .focus(window, cx)
                    });
                }
                let mut data = self.shared.borrow_mut();
                let row = data.session.workspace.panel_mut(&id).sensor_mut(&sensor);
                row.collapsed = !row.collapsed;
            }
            Command::SensorVisible(id, sensor) => {
                let mut data = self.shared.borrow_mut();
                let row = data.session.workspace.panel_mut(&id).sensor_mut(&sensor);
                row.visible = !row.visible;
            }
            Command::SensorMove(id, sensor, direction) => {
                let result = self
                    .shared
                    .borrow_mut()
                    .session
                    .workspace
                    .panels
                    .get_mut(&id)
                    .ok_or_else(|| "The monitor no longer exists".to_owned())
                    .and_then(|panel| panel.move_sensor(&sensor, direction));
                if let Err(error) = result {
                    self.notice = error;
                }
            }
            Command::SensorMeter(id, sensor, meter) => {
                let mut data = self.shared.borrow_mut();
                let quantity = data
                    .catalog
                    .iter()
                    .find(|monitor| monitor.id == id)
                    .and_then(|monitor| monitor.sensors.iter().find(|s| s.id == sensor))
                    .map(|s| s.quantity);
                let result = quantity
                    .ok_or_else(|| "The sensor no longer exists".to_owned())
                    .and_then(|quantity| {
                        data.session
                            .workspace
                            .panels
                            .get_mut(&id)
                            .ok_or_else(|| "The monitor no longer exists".to_owned())?
                            .select_meter(&sensor, quantity, meter)
                    });
                if let Err(error) = result {
                    self.notice = error;
                }
            }
            Command::Meter(id, sensor) => {
                let mut data = self.shared.borrow_mut();
                let quantity = data
                    .catalog
                    .iter()
                    .find(|m| m.id == id)
                    .and_then(|m| m.sensors.iter().find(|s| s.id == sensor))
                    .map(|s| s.quantity)
                    .unwrap_or(system_pulse_model::Quantity::Scalar);
                let row = data.session.workspace.panel_mut(&id).sensor_mut(&sensor);
                row.meter = quantity.next_meter(row.meter);
            }
            Command::SavePreset => {
                if self.read_blocked {
                    self.notice = "Saved state could not be read; correct the read error and restart before saving".into();
                    cx.notify();
                    return;
                }
                self.record(cx);
                match self.shared.borrow().session.autosave_json() {
                    Ok(json) => {
                        self.preset = Some(json.clone());
                        if let Some(dir) = &self.directory {
                            self.revision += 1;
                            let path = dir.join("preset.json");
                            let storage = self.storage.clone();
                            let revision = self.revision;
                            cx.spawn(async move |weak, cx| {
                                if let Err(e) =
                                    smol::unblock(move || storage.write(&path, revision, &json))
                                        .await
                                {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.notice = e;
                                        cx.notify();
                                    });
                                }
                            })
                            .detach();
                        }
                    }
                    Err(e) => self.notice = e,
                }
            }
            Command::RecallPreset => {
                if let Some(raw) = self.preset.clone() {
                    self.restore(&raw, window, cx);
                } else {
                    self.notice = "Save a preset before recalling it".into();
                }
            }
            Command::Recover => {
                if self.read_blocked {
                    self.notice = "Correct the configuration-directory read error and restart; the original file remains untouched".into();
                    cx.notify();
                    return;
                }
                let rejected = self
                    .shared
                    .borrow()
                    .session
                    .rejected
                    .as_ref()
                    .map(|r| r.original.clone());
                if let (Some(raw), Some(dir)) = (rejected, &self.directory) {
                    self.revision += 1;
                    if let Err(e) = self.storage.write(
                        &dir.join("workspace.rejected.json"),
                        self.revision,
                        &raw,
                    ) {
                        self.notice = e;
                        cx.notify();
                        return;
                    }
                }
                // The recovery button disappears after acceptance. Keep the
                // keyboard dispatch path in the retained workspace before removal.
                self.focus.focus(window, cx);
                self.shared.borrow_mut().session.accept_recovery();
                self.notice.clear();
            }
            Command::Preset(command) => {
                self.preset_command(command, window, cx);
                return;
            }
            Command::Interval(ms) => {
                let rate = match ms {
                    500 => ramag_ui::MonitorRefreshRate::HalfSecond,
                    1_000 => ramag_ui::MonitorRefreshRate::OneSecond,
                    2_000 => ramag_ui::MonitorRefreshRate::TwoSeconds,
                    5_000 => ramag_ui::MonitorRefreshRate::FiveSeconds,
                    _ => {
                        self.notice = format!("Unsupported sampling interval: {ms} ms");
                        return;
                    }
                };
                ramag_ui::save_monitor_settings(
                    ramag_ui::MonitorSettings { refresh_rate: rate },
                    cx,
                );
                self.shared.borrow_mut().session.workspace.interval_ms = ms;
            }
            Command::Save => {}
        }
        self.record(cx);
        self.queue_save(cx);
        self.notify_panels(cx);
    }
}
