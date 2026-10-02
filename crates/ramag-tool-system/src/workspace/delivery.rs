use super::*;
impl WorkspaceView {
    pub(super) fn deliver(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.deliver_optional(Some(window), cx);
    }

    pub(super) fn deliver_optional(&mut self, window: Option<&mut Window>, cx: &mut Context<Self>) {
        if let Some(writer) = &self.diagnostics
            && let Some(error) = writer.take_error()
        {
            self.notice = error;
            cx.notify();
        }
        if let Some(snapshot) = self.service.as_ref().and_then(SamplingService::take_latest) {
            if let Some(window) = window {
                self.accept_snapshot(snapshot, window, cx);
            } else {
                self.accept_background_snapshot(snapshot, cx);
            }
        } else {
            let clock = self.shared.borrow().accepted_clock;
            let Some((accepted, collector_ms)) = clock else {
                return;
            };
            let now = collector_ms.saturating_add(accepted.elapsed().as_millis() as u64);
            let mut data = self.shared.borrow_mut();
            let threshold = data.session.workspace.interval_ms * 2;
            if data.history.mark_stale(now, threshold) {
                if self.diagnostics.is_some()
                    || (self.attached_window.is_some() && self.screen_view.is_none())
                {
                    data.prepare_processes();
                }
                drop(data);
                self.publish_diagnostics(now);
                self.notify_panels(cx);
            }
        }
    }

    pub(crate) fn accept_snapshot(
        &mut self,
        snapshot: Snapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.accept_snapshot_optional(snapshot, Some(window), cx);
    }

    pub(crate) fn accept_background_snapshot(
        &mut self,
        snapshot: Snapshot,
        cx: &mut Context<Self>,
    ) {
        self.accept_snapshot_optional(snapshot, None, cx);
    }

    fn accept_snapshot_optional(
        &mut self,
        snapshot: Snapshot,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        let model_started = self
            .diagnostics
            .as_ref()
            .and_then(|writer| writer.timestamp());
        let unix_ns = live::unix_ns();
        let now_ms = live::collector_now_ms(&snapshot, unix_ns);
        let mut data = self.shared.borrow_mut();
        let known: BTreeSet<_> = data.session.workspace.panels.keys().cloned().collect();
        let interval = data.session.workspace.interval_ms;
        let Data {
            session,
            history,
            live,
            ..
        } = &mut *data;
        if let Err(error) =
            live.accept(&snapshot, &mut session.workspace, history, now_ms, interval)
        {
            self.notice = error;
            cx.notify();
            return;
        }
        if !self.fixture_mode {
            let mut visible_gpu = snapshot.monitors.iter().any(|m| {
                crate::layout::is_gpu(&m.id)
                    && known.contains(&m.id)
                    && data
                        .session
                        .workspace
                        .panels
                        .get(&m.id)
                        .is_some_and(|p| p.visible)
            });
            for monitor in &snapshot.monitors {
                if !known.contains(&monitor.id) {
                    let primary = matches!(
                        monitor.id.as_str(),
                        "cpu:host" | "memory:host" | "processes"
                    );
                    let gpu = crate::layout::is_gpu(&monitor.id) && !visible_gpu;
                    data.session.workspace.panel_mut(&monitor.id).visible = primary || gpu;
                    visible_gpu |= gpu;
                }
            }
        }
        let catalog = live::catalog(&data.session.workspace);
        let changed = data.catalog != catalog;
        data.catalog = catalog;
        let snapshot = std::sync::Arc::new(snapshot);
        data.snapshot = Some(snapshot);
        data.accepted_clock = Some((std::time::Instant::now(), now_ms));
        data.process_presentation_stale = None;
        data.processes.clear();
        let legacy = self.attached_window.is_some() && self.screen_view.is_none();
        if legacy || self.diagnostics.is_some() {
            data.prepare_processes();
        }
        let initial_dock = window
            .as_ref()
            .filter(|_| self.initial_layout_pending)
            .and_then(|_| {
                let catalog = data.catalog.clone();
                crate::layout::initialize(
                    &mut data.session.workspace,
                    &catalog,
                    &mut self.initial_layout_pending,
                )
            });
        self.accepted_unix_ns = unix_ns;
        drop(data);
        self.accepted_model_timing = model_started.zip(
            self.diagnostics
                .as_ref()
                .and_then(|writer| writer.timestamp()),
        );
        self.publish_diagnostics(now_ms);
        self.refresh_legacy_panels(cx);
        if let Some(window) = window {
            if let Some(state) = initial_dock {
                self.shared.borrow_mut().bounds.clear();
                self.dock.update(cx, |dock, cx| {
                    if let Err(error) = dock.load(state, window, cx) {
                        self.notice = format!("Load first-launch layout: {error}");
                    }
                });
            }
            if changed {
                self.capture_sizes(cx);
                ensure_enabled_regions(&self.shared, &self.dock, window, cx);
                self.record(cx);
            }
        }
        if changed {
            self.queue_save(cx);
        }
        self.notify_panels(cx);
    }

    fn refresh_legacy_panels(&self, cx: &mut Context<Self>) {
        // The tabbed application renders ScreenView; the dock remains for saved-layout compatibility.
        if self.screen_view.is_some() || self.attached_window.is_none() {
            return;
        }
        let data = self.shared.borrow();
        let updates: Vec<_> = data
            .catalog
            .iter()
            .filter_map(|monitor| {
                data.views
                    .get(&monitor.id)
                    .map(|view| (view.clone(), monitor.clone()))
            })
            .collect();
        let identities = data.process_identities();
        drop(data);
        for (view, monitor) in updates {
            let _ = view.update(cx, |panel, cx| {
                panel.refresh(monitor, cx);
                live::reconcile_selection(&mut panel.selected, &identities);
            });
        }
    }

    fn publish_diagnostics(&mut self, rendered_at_collector_ms: u64) {
        let Some(writer) = &self.diagnostics else {
            return;
        };
        let mut data = self.shared.borrow_mut();
        data.prepare_processes_at(rendered_at_collector_ms);
        let Some(snapshot) = data.snapshot.clone() else {
            return;
        };
        self.diagnostic_revision += 1;
        let construction_started = writer.timestamp();
        let record = crate::diagnostics::Record::new(
            snapshot,
            self.accepted_unix_ns,
            self.diagnostic_revision,
            rendered_at_collector_ms,
            &data,
        );
        if construction_started.is_some() {
            writer.submit_timed(record, self.accepted_model_timing, construction_started);
        } else {
            writer.submit(record);
        }
    }

    pub(super) fn capture_sizes(&mut self, cx: &App) {
        capture_preferences(&self.shared, &self.dock.read(cx).dump(cx));
    }

    pub(crate) fn record(&mut self, cx: &App) {
        self.capture_sizes(cx);
        let state = self.dock.read(cx).dump(cx);
        if let Ok(value) = serde_json::to_value(state) {
            self.shared.borrow_mut().session.workspace.dock = value;
        }
    }

    pub(super) fn notify_panels(&self, cx: &mut Context<Self>) {
        if self.attached_window.is_none() {
            return;
        }
        if let Some(view) = self.screen_view.clone() {
            // A screen command can be issued while that view is being updated.
            // Refresh after its event callback releases the entity borrow.
            cx.defer(move |cx| {
                let _ = view.update(cx, |screen, cx| screen.refresh(cx));
            });
            return;
        }
        let views: Vec<_> = self.shared.borrow().views.values().cloned().collect();
        for view in views {
            let _ = view.update(cx, |panel, cx| {
                if let Some(group) = &panel.group {
                    let _ = group.update(cx, |_, cx| cx.notify());
                }
                if let Some(settings) = &panel.settings {
                    settings.update(cx, |settings, cx| settings.refresh(cx));
                }
                cx.notify();
            });
        }
        cx.notify();
    }

    /// Advance deterministic fixture history once so test views have useful initial data.
    #[cfg(test)]
    pub(crate) fn advance(&mut self, cx: &mut Context<Self>) {
        self.tick += 1;
        if let Err(error) = fixture::advance(&mut self.shared.borrow_mut().history, self.tick) {
            self.notice = error;
        }
        self.shared.borrow_mut().processes = fixture::processes();
        self.notify_panels(cx);
    }

    pub(super) fn queue_save(&mut self, cx: &mut Context<Self>) {
        if self.read_blocked {
            return;
        }
        let Some(dir) = &self.directory else { return };
        let data = self.shared.borrow();
        let result = validate_dock_mode(&data.session.workspace.dock, self.fixture_mode)
            .and_then(|_| data.session.autosave_json());
        drop(data);
        let json = match result {
            Ok(json) => json,
            Err(e) => {
                self.notice = e;
                cx.notify();
                return;
            }
        };
        self.revision += 1;
        let revision = self.revision;
        let path = dir.join("workspace.json");
        let storage = self.storage.clone();
        self.save_task = Some(cx.spawn(async move |weak, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(250))
                .await;
            let result = smol::unblock(move || storage.write(&path, revision, &json)).await;
            if let Err(error) = result {
                let _ = weak.update(cx, |this, cx| {
                    this.notice = error;
                    cx.notify();
                });
            }
        }));
    }

    pub(crate) fn restore(&mut self, raw: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.initial_layout_pending = false;
        let catalog = self.shared.borrow().catalog.clone();
        let mut fallback = Workspace::new(
            serde_json::to_value(default_dock_for(&catalog)).unwrap_or_else(|error| {
                eprintln!("Default workspace serialization failed: {error}");
                serde_json::Value::Null
            }),
        );
        live::discover(&mut fallback, &catalog);
        let mut session = restore_session(raw, fallback, self.fixture_mode);
        live::discover(&mut session.workspace, &catalog);
        if let Some(rejected) = self.shared.borrow().session.rejected.clone() {
            session.rejected = Some(rejected);
        }
        let interval = Duration::from_millis(session.workspace.interval_ms);
        if let Some(service) = &self.service
            && let Err(e) = service.set_interval(interval)
        {
            self.notice = e;
        }
        if !self.fixture_mode {
            self.shared.borrow_mut().catalog = live::catalog(&session.workspace);
        }
        // Restoring a saved workspace must update both fonts and the host theme;
        // otherwise the model says Light while the visible window stays Dark.
        crate::settings::apply_user_choice(session.workspace.appearance, window, cx);
        self.shared.borrow_mut().session = session;
        let state = {
            let raw = self.shared.borrow().session.workspace.dock.clone();
            serde_json::from_value(raw).ok()
        };
        if let Some(state) = state {
            self.shared.borrow_mut().bounds.clear();
            self.dock.update(cx, |dock, cx| {
                if let Err(error) = dock.load(state, window, cx) {
                    self.notice = format!("Load restored layout: {error}");
                }
            });
        }
        ensure_enabled_regions(&self.shared, &self.dock, window, cx);
        self.notify_panels(cx);
    }

    pub(super) fn request_keyboard_repaint(&mut self, window: &mut Window, cx: &Context<Self>) {
        if self.keyboard_repaint_pending {
            return;
        }
        self.keyboard_repaint_pending = true;
        // Keep queued Alt navigation from forcing a redraw for every key.
        cx.on_next_frame(window, |this, _, cx| {
            this.keyboard_repaint_pending = false;
            cx.notify();
        });
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn cpu_temperatures_enabled(&self) -> bool {
        self.service
            .as_ref()
            .is_some_and(SamplingService::cpu_temperatures_enabled)
    }
}
