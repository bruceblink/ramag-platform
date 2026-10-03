#[cfg(test)]
mod process_presentation_tests {
    #[cfg(test)]
    use crate::test_support::TestUnwrapExt;
    use crate::workspace::Snapshot;
    use gpui_kit::TestAppContext;

    #[gpui_kit::test]
    fn deferred_process_presentation_ages_without_another_delivery(cx: &mut TestAppContext) {
        let (view, cx) = crate::native_tests::harness(cx);
        let reading = ramag_infra_system::Reading {
            sensor_id: "test".into(),
            value: Some(42.),
            total: None,
            availability: ramag_infra_system::Availability::Available,
            reason: None,
            observations: vec![],
        };
        let failed = ramag_infra_system::Reading {
            availability: ramag_infra_system::Availability::Failed,
            value: None,
            reason: Some("Permission denied".into()),
            ..reading.clone()
        };
        cx.update(|_, cx| {
            view.update(cx, |view, _| {
                let mut data = view.shared.borrow_mut();
                data.session.workspace.interval_ms = 1000;
                data.snapshot = Some(std::sync::Arc::new(Snapshot {
                    sequence: 1,
                    capture_finished_ns: 20_000_000_000,
                    processes: vec![ramag_infra_system::ProcessRow {
                        identity: ramag_infra_system::ProcessIdentity {
                            pid: 7,
                            start_time_ticks: 9,
                        },
                        name: "deferred".into(),
                        user: None,
                        user_reason: None,
                        cpu_percent: reading.clone(),
                        memory_bytes: failed,
                        read_bytes_per_second: reading.clone(),
                        write_bytes_per_second: reading.clone(),
                        threads: reading,
                    }],
                    ..Snapshot::default()
                }));
                data.processes.clear();
                data.prepare_processes_at(22_000);
                assert!(!data.processes[0].cells[2].contains("Stale"));
                data.prepare_processes_at(22_001);
                assert!(data.processes[0].cells[2].contains("Stale"));
                assert!(data.processes[0].cells[3].contains("No access"));
                assert_eq!(data.snapshot.as_ref().test_unwrap().sequence, 1);
                assert_eq!(
                    data.snapshot.as_ref().test_unwrap().processes[0]
                        .cpu_percent
                        .value,
                    Some(42.)
                );
            })
        });
    }
}

#[cfg(test)]
mod monitor_settings_tests {
    #[cfg(test)]
    use crate::test_support::TestUnwrapExt;
    use crate::workspace::WorkspaceView;
    use gpui_kit::{AppContext, TestAppContext};

    #[gpui_kit::test]
    fn global_sampling_changes_update_workspace_and_survive_layout_restore(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui_kit::component::init);
        let mut workspace = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| WorkspaceView::new_fixture(window, cx));
            workspace = Some(view.clone());
            gpui_kit::component::Root::new(view, window, cx)
        });
        let workspace = workspace.test_unwrap();

        cx.update(|_, app| {
            ramag_ui::set_monitor_settings(
                ramag_ui::MonitorSettings {
                    refresh_rate: ramag_ui::MonitorRefreshRate::HalfSecond,
                },
                app,
            );
        });
        cx.run_until_parked();
        assert_eq!(
            cx.read(|cx| workspace
                .read(cx)
                .shared
                .borrow()
                .session
                .workspace
                .interval_ms),
            500
        );

        let raw = cx.read(|cx| {
            let mut saved_workspace = workspace.read(cx).shared.borrow().session.workspace.clone();
            saved_workspace.interval_ms = 5_000;
            serde_json::to_string(&saved_workspace).test_unwrap()
        });
        cx.update(|window, app| {
            workspace.update(app, |view, cx| view.restore(&raw, window, cx));
        });
        assert_eq!(
            cx.read(|cx| workspace
                .read(cx)
                .shared
                .borrow()
                .session
                .workspace
                .interval_ms),
            500,
            "restoring a layout must retain the global sampling cadence"
        );

        cx.update(|_, app| {
            ramag_ui::set_monitor_settings(
                ramag_ui::MonitorSettings {
                    refresh_rate: ramag_ui::MonitorRefreshRate::TwoSeconds,
                },
                app,
            );
        });
        cx.run_until_parked();
        assert_eq!(
            cx.read(|cx| workspace
                .read(cx)
                .shared
                .borrow()
                .session
                .workspace
                .interval_ms),
            2_000
        );
    }
}

#[cfg(test)]
mod diagnostic_delivery_tests {
    #[cfg(test)]
    use crate::test_support::TestUnwrapExt;
    use crate::workspace::{Snapshot, WorkspaceView};
    use gpui_kit::{AppContext, Element, Role, TestAppContext};
    use std::time::Duration;

    #[gpui_kit::test]
    fn elapsed_delivery_refreshes_diagnostics_without_refreshing_the_snapshot(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui_kit::component::init);
        let mut view = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let workspace = cx.new(|cx| WorkspaceView::new_fixture(window, cx));
            view = Some(workspace.clone());
            gpui_kit::component::Root::new(workspace, window, cx)
        });
        let view = view.test_unwrap();
        let dir = std::env::temp_dir().join(format!("pulse-stale-delivery-{}", std::process::id()));
        std::fs::create_dir_all(&dir).test_unwrap();
        let path = dir.join("latest.json");
        let reading = ramag_infra_system::Reading {
            sensor_id: "cpu:host/usage".into(),
            value: Some(42.),
            total: None,
            availability: ramag_infra_system::Availability::Available,
            reason: None,
            observations: vec![],
        };
        let snapshot = Snapshot {
            sequence: 43,
            capture_started_ns: 19_900_000_000,
            capture_finished_ns: 20_000_000_000,
            monitors: vec![ramag_infra_system::MonitorDescriptor {
                id: "cpu:host".into(),
                title: "CPU".into(),
                kind: ramag_infra_system::MonitorKind::Cpu,
                summary_sensor_id: "cpu:host/usage".into(),
            }],
            sensors: vec![ramag_infra_system::SensorDescriptor {
                id: "cpu:host/usage".into(),
                monitor_id: "cpu:host".into(),
                title: "Usage".into(),
                kind: ramag_infra_system::SensorKind::Percentage,
                unit: ramag_infra_system::Unit::Percent,
                source: "controlled test".into(),
                scope: "host".into(),
                scale: None,
            }],
            readings: vec![
                reading.clone(),
                ramag_infra_system::Reading {
                    sensor_id: "cpu:host/processes".into(),
                    value: Some(1.),
                    ..reading.clone()
                },
            ],
            processes: vec![ramag_infra_system::ProcessRow {
                identity: ramag_infra_system::ProcessIdentity {
                    pid: 7,
                    start_time_ticks: 9,
                },
                name: "controlled process".into(),
                user: Some("user".into()),
                user_reason: None,
                cpu_percent: reading.clone(),
                memory_bytes: reading.clone(),
                read_bytes_per_second: reading.clone(),
                write_bytes_per_second: reading.clone(),
                threads: reading,
            }],
            ..Snapshot::default()
        };
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                this.diagnostics = Some(
                    crate::diagnostics::Writer::start_with_trace(path.clone(), true).test_unwrap(),
                );
                this.accept_snapshot(snapshot, window, cx);
            })
        });
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        let before: serde_json::Value = loop {
            if let Ok(raw) = std::fs::read_to_string(&path)
                && let Ok(record) = serde_json::from_str::<serde_json::Value>(&raw)
                && record["snapshot"]["sequence"] == 43
            {
                break record;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "initial diagnostic write did not finish"
            );
            std::thread::sleep(Duration::from_millis(5));
        };
        let initial_history_len = cx.read(|cx| {
            view.read(cx)
                .shared
                .borrow()
                .history
                .samples("cpu:host", "cpu:host/usage")
                .test_unwrap()
                .len()
        });
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                this.shared.borrow_mut().accepted_clock = Some((
                    std::time::Instant::now() - Duration::from_millis(3000),
                    20_000,
                ));
                this.deliver(window, cx);
                let revision = this.diagnostic_revision;
                this.deliver(window, cx);
                assert_eq!(
                    this.diagnostic_revision, revision,
                    "unchanged stale state must not publish again"
                );
            })
        });
        let writer = cx.update(|_, cx| view.update(cx, |this, _| this.diagnostics.take()));
        drop(writer); // Flush the bounded worker's final record, without another collection.
        let after: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).test_unwrap()).test_unwrap();
        let mapping = |record: &serde_json::Value, id: &str| {
            record["rendered"]
                .as_array()
                .test_unwrap()
                .iter()
                .find(|r| r["element_id"] == id)
                .test_unwrap()
                .clone()
        };
        let id = "cpu:host:value:cpu:host/usage";
        assert_eq!(mapping(&before, id)["sample"]["status"], "current");
        cx.read(|cx| {
            let data = view.read(cx).shared.borrow();
            let monitor = data
                .catalog
                .iter()
                .find(|m| m.id == "cpu:host")
                .test_unwrap();
            let sample = data
                .history
                .latest("cpu:host", "cpu:host/usage")
                .test_unwrap();
            assert_eq!(sample.status, system_pulse_model::ReadingStatus::Stale);
            assert_eq!(
                data.history
                    .samples("cpu:host", "cpu:host/usage")
                    .test_unwrap()
                    .len(),
                initial_history_len
            );
            let label = crate::meters::sensor_label(monitor, &monitor.sensors[0], sample);
            let mut node = gpui_kit::accesskit::Node::new(Role::Label);
            crate::meters::metric_label(id.into(), label.clone()).write_a11y_info(&mut node);
            assert_eq!(node.value(), Some(label.as_str()));
            assert_eq!(mapping(&after, id)["label"], label);
            assert_eq!(mapping(&after, id)["sample"]["status"], "stale");
            assert_eq!(
                mapping(&after, "cpu:host:summary")["label"],
                crate::meters::summary(monitor, &data.history)
            );
            assert_eq!(
                mapping(&after, "process:7:9:cell:2")["label"],
                data.processes[0].cells[2]
            );
            assert!(data.processes[0].cells[2].contains("Stale"));
        });
        assert_eq!(
            after["render_revision"].as_u64().test_unwrap(),
            before["render_revision"].as_u64().test_unwrap() + 1
        );
        assert_eq!(before["rendered_at_collector_ms"], 20_000);
        assert!(after["rendered_at_collector_ms"].as_u64().test_unwrap() >= 23_000);
        assert_eq!(
            mapping(&after, id)["sample"]["at_ms"],
            mapping(&before, id)["sample"]["at_ms"]
        );
        assert_eq!(after["snapshot"], before["snapshot"]);
        assert_eq!(after["accepted_unix_ns"], before["accepted_unix_ns"]);
        assert_eq!(after["application_pid"], before["application_pid"]);
        let timing_path = path.with_extension("publication-timing.json");
        assert!(timing_path.is_file(), "opt-in publication timings missing");
        let timing: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&timing_path).test_unwrap()).test_unwrap();
        let records = timing["history"]["records"].as_array().test_unwrap();
        assert_eq!(records.len(), 2);
        for record in records {
            assert_eq!(record["sequence"], 43);
            assert_eq!(record["accepted_unix_ns"], before["accepted_unix_ns"]);
            let stages = &record["stages"];
            let values: Vec<_> = [
                "acceptance_started_ns",
                "model_completed_ns",
                "construction_started_ns",
                "construction_completed_ns",
                "submission_started_ns",
                "submission_completed_ns",
                "dequeue_ns",
            ]
            .iter()
            .map(|key| stages[key].as_u64().test_unwrap())
            .collect();
            assert!(values.windows(2).all(|pair| pair[0] <= pair[1]));
        }
        assert_eq!(
            records[0]["stages"]["acceptance_started_ns"],
            records[1]["stages"]["acceptance_started_ns"]
        );
        assert_eq!(
            records[0]["stages"]["model_completed_ns"],
            records[1]["stages"]["model_completed_ns"]
        );
        std::fs::remove_file(timing_path).test_unwrap();
        std::fs::remove_file(path).test_unwrap();
        std::fs::remove_dir(dir).test_unwrap();
    }
}

#[cfg(test)]
mod preset_bound_tests {
    #[cfg(test)]
    use crate::test_support::TestUnwrapExt;
    use crate::workspace::{Command, WorkspaceView};
    use gpui_kit::{AppContext, TestAppContext};

    #[gpui_kit::test]
    fn oversized_preset_keeps_the_previous_slot_and_reports_the_save_error(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui_kit::component::init);
        let mut view = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let workspace = cx.new(|cx| WorkspaceView::new_fixture(window, cx));
            view = Some(workspace.clone());
            gpui_kit::component::Root::new(workspace, window, cx)
        });
        let view = view.test_unwrap();
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                let previous = this.shared.borrow().session.autosave_json().test_unwrap();
                this.preset = Some(previous.clone());
                this.shared
                    .borrow_mut()
                    .session
                    .workspace
                    .monitors
                    .values_mut()
                    .next()
                    .test_unwrap()
                    .title = "x".repeat(system_pulse_model::MAX_CONFIGURATION_BYTES + 1);
                this.command(Command::SavePreset, window, cx);
                assert!(
                    this.preset.as_deref() == Some(previous.as_str()),
                    "rejected serialization must preserve the previous preset slot"
                );
                assert!(this.notice.contains("16 MiB"));
            })
        });
    }
}
