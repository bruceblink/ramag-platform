use super::*;
impl WorkspaceView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::construct(false, window, cx)
    }

    #[cfg(test)]
    pub(crate) fn new_fixture(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::construct(true, window, cx)
    }

    fn construct(fixture_mode: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let live = !fixture_mode;
        let initial = initial_catalog(fixture_mode);
        let mut workspace = if fixture_mode {
            Workspace::new(
                serde_json::to_value(default_dock_for(&initial)).unwrap_or_else(|error| {
                    eprintln!("Default dock serialization failed: {error}");
                    serde_json::Value::Null
                }),
            )
        } else {
            crate::layout::preset(system_pulse_model::BuiltinPreset::Default, &initial)
        };
        live::discover(&mut workspace, &initial);
        #[cfg(test)]
        if fixture_mode {
            workspace.panel_mut("cpu").sensor_mut("overall").meter =
                system_pulse_model::Meter::Line;
        }
        let mut session = Session {
            workspace: workspace.clone(),
            rejected: None,
        };
        let mut notice = String::new();
        let mut read_blocked = false;
        let directory = if live {
            match storage::directory() {
                Ok(path) => Some(path),
                Err(e) => {
                    notice = e;
                    read_blocked = true;
                    None
                }
            }
        } else {
            None
        };
        let mut preset = None;
        let mut has_saved_workspace = false;
        if let Some(dir) = &directory {
            match storage::read(&dir.join("workspace.json")) {
                Ok(Some(raw)) => {
                    has_saved_workspace = true;
                    session = restore_session(&raw, workspace, fixture_mode);
                }
                Ok(None) => {}
                Err(e) => {
                    notice = e;
                    read_blocked = true;
                }
            }
            match storage::read(&dir.join("preset.json")) {
                Ok(raw) => preset = raw,
                Err(e) => {
                    notice = e;
                    read_blocked = true;
                }
            }
        }
        let (presets, preset_error) =
            presets::load(directory.as_deref(), preset.as_deref(), fixture_mode);
        let catalog = if fixture_mode {
            initial
        } else {
            live::catalog(&session.workspace)
        };
        live::discover(&mut session.workspace, &catalog);
        session.workspace.interval_ms =
            Self::effective_interval_ms(session.workspace.interval_ms, cx);
        let interval = Duration::from_millis(session.workspace.interval_ms);
        let service = if live {
            match SamplingService::start(interval) {
                Ok(service) => Some(service),
                Err(e) => {
                    notice = e;
                    None
                }
            }
        } else {
            None
        };
        let diagnostics = if live {
            crate::diagnostics::Writer::from_env().unwrap_or_else(|e| {
                notice = e;
                None
            })
        } else {
            None
        };
        let shared = Rc::new(RefCell::new(Data {
            session,
            history: HistoryStore::new(120).unwrap_or_else(|error| {
                eprintln!("History initialization failed: {error}");
                HistoryStore::new(1).unwrap_or_else(|_| unreachable!("capacity one is valid"))
            }),
            catalog,
            processes: Vec::new(),
            accepted_clock: None,
            process_presentation_stale: None,
            process_widths: live::PROCESS_WIDTHS,
            allow_process_actions: live && !fixture_mode,
            process_action: crate::panel::ProcessActionState::default(),
            snapshot: None,
            live: LiveState::default(),
            presets,
            preset_error,
            preset_notice: String::new(),
            preset_busy: false,
            owner: Some(cx.weak_entity()),
            views: BTreeMap::new(),
            bounds: BTreeMap::new(),
            scroll: ScrollHandle::default(),
        }));
        let dock = Self::create_dock(&shared, window, cx);
        let mut view = Self {
            attached_window: Some(window.window_handle()),
            shared,
            screen_view: None,
            dock,
            notice,
            directory,
            read_blocked,
            storage: Storage::default(),
            revision: 0,
            #[cfg(test)]
            #[cfg(test)]
            tick: 0,
            fixture_mode,
            initial_layout_pending: live && !has_saved_workspace && !read_blocked,
            service,
            monitor_settings_subscription: None,
            accepted_unix_ns: 0,
            accepted_model_timing: None,
            diagnostic_revision: 0,
            diagnostics,
            preset,
            timer: None,
            save_task: None,
            focus: cx.focus_handle(),
            visibility_scroll: ScrollHandle::default(),
            keyboard_repaint_pending: false,
            visibility_controls: BTreeMap::new(),
            confirm_layout_reset: false,
            cancel_layout_reset_focus: cx.focus_handle(),
        };
        view.monitor_settings_subscription = Some(
            cx.observe_global::<ramag_ui::MonitorSettingsGlobal>(|this, cx| {
                this.sync_monitor_settings(cx);
            }),
        );
        #[cfg(test)]
        if fixture_mode {
            view.advance(cx);
        }
        if live {
            view.timer = Some(Self::sampling_timer(cx));
            // The final native window closes before App quits. Retain this view
            // until the App-level callback snapshots it; a weak entity callback
            // can otherwise disappear with the window before writing state.
            let owner = cx.entity();
            App::on_app_quit(cx, move |cx| {
                let (path, storage, json, preset) = owner.update(cx, |this, cx| {
                    this.timer.take();
                    this.record(cx);
                    let data = this.shared.borrow();
                    let json = if this.read_blocked {
                        None
                    } else {
                        validate_dock_mode(&data.session.workspace.dock, this.fixture_mode)
                            .and_then(|_| data.session.autosave_json())
                            .ok()
                    };
                    (
                        this.directory.clone(),
                        this.storage.clone(),
                        json,
                        this.preset.clone(),
                    )
                });
                cx.background_executor().spawn(async move {
                    if let (Some(path), Some(json)) = (path, json) {
                        if let Err(error) =
                            storage.write(&path.join("workspace.json"), u64::MAX, &json)
                        {
                            eprintln!("{error}");
                        }
                        if let Some(preset) = preset
                            && let Err(error) =
                                storage.write(&path.join("preset.json"), u64::MAX, &preset)
                        {
                            eprintln!("{error}");
                        }
                    }
                })
            })
            .detach();
        }
        view
    }
}
