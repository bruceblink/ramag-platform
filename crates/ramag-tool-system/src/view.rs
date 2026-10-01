//! System Pulse style system monitor view, adapted to Ramag's GPUI lifecycle.

use std::time::Duration;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{AppContext as _, Context, Entity, FocusHandle, ScrollHandle, Window};

use super::{ProcessSort, RefreshInterval, StableProcessIdentity, SystemMonitor};
use helpers::notice_for_termination;

mod process_selection;
use process_selection::SelectedProcess;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum SystemSection {
    #[default]
    Summary,
    Cpu,
    Memory,
    Gpu,
    Disks,
    Network,
    Energy,
    Thermals,
    Processes,
    Settings,
}

impl SystemSection {
    pub(super) const ALL: [Self; 10] = [
        Self::Summary,
        Self::Cpu,
        Self::Memory,
        Self::Gpu,
        Self::Disks,
        Self::Network,
        Self::Energy,
        Self::Thermals,
        Self::Processes,
        Self::Settings,
    ];

    pub(super) const fn id(self) -> &'static str {
        match self {
            Self::Summary => "summary",
            Self::Cpu => "cpu",
            Self::Memory => "memory",
            Self::Gpu => "gpu",
            Self::Disks => "disks",
            Self::Network => "network",
            Self::Energy => "energy",
            Self::Thermals => "thermals",
            Self::Processes => "processes",
            Self::Settings => "settings",
        }
    }

    pub(super) const fn title(self) -> &'static str {
        match self {
            Self::Summary => "Summary",
            Self::Cpu => "CPU",
            Self::Memory => "Memory",
            Self::Gpu => "GPU",
            Self::Disks => "Disks",
            Self::Network => "Network",
            Self::Energy => "Energy",
            Self::Thermals => "Thermals",
            Self::Processes => "Processes",
            Self::Settings => "Settings",
        }
    }
}

/// Captures one confirmed target independently of sorting, filtering and PID reuse.
/// Only the identity-checked worker may act on it; cancelling drops the request without signaling.
#[derive(Clone, Debug)]
pub(super) struct TerminationRequest {
    pub identity: StableProcessIdentity,
    pub name: String,
}

impl TerminationRequest {
    /// Names the captured target and the data-loss boundary before destructive confirmation.
    fn description(&self) -> String {
        format!(
            "可能丢失未保存数据；仅影响此进程，不包括子进程。强制退出 {}（PID {}）？",
            self.name, self.identity.pid
        )
    }
}

#[derive(Clone, Debug)]
pub(super) struct Notice {
    pub message: String,
    pub error: bool,
}

/// Owns UI-only state while SystemMonitor owns the sampling service and cache.
pub struct SystemView {
    pub(super) monitor: SystemMonitor,
    pub(super) section: SystemSection,
    pub(super) termination_request: Option<TerminationRequest>,
    pub(super) termination_focus: FocusHandle,
    /// Retains body scrolling across sampling redraws and resets for each newly captured target.
    pub(super) termination_scroll: ScrollHandle,
    /// Focus moves into the dialog once per opening so sampling renders do not steal Tab focus.
    pub(super) termination_focus_requested: bool,
    pub(super) termination_in_progress: bool,
    pub(super) notice: Option<Notice>,
    pub(super) process_search: Entity<InputState>,
    /// Scrolls only the desktop table; sampling and sorting retain the user's column position.
    pub(super) process_table_scroll: ScrollHandle,
    /// Stores only one complete process identity; live details are resolved from each new snapshot.
    selected_process: Option<SelectedProcess>,
    /// Retains bounded detail body scrolling without moving the close control during sampling.
    pub(super) process_detail_scroll: ScrollHandle,
    pub(super) presentation: ramag_ui::MonitorPresentationSettings,
    _search_subscription: Option<gpui_kit::Subscription>,
    /// Dropping the view unregisters observers and stops the periodic redraw task.
    _settings_subscription: Option<gpui_kit::Subscription>,
}

impl SystemView {
    /// Starts collection and polls only the worker's owned snapshot cache on the UI timer.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let monitor = SystemMonitor::new();
        apply_monitor_preferences(&monitor, cx);
        let settings_subscription = observe_monitor_preferences(cx);
        let process_search = cx
            .new(|cx| ramag_ui::bounded_search_input(window, cx).placeholder("名称 / PID / 用户"));
        let search_subscription = cx.subscribe_in(
            &process_search,
            window,
            move |_, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            },
        );
        let presentation = ramag_ui::monitor_presentation_settings(cx);

        let ticker_monitor = monitor.clone();
        cx.spawn_in(window, async move |this, async_cx| {
            loop {
                async_cx
                    .background_executor()
                    .timer(Duration::from_millis(200))
                    .await;
                let changed = ticker_monitor.refresh_if_due();
                if this
                    .update_in(async_cx, |_, _, cx| {
                        if changed {
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        monitor.refresh_now();

        Self {
            monitor,
            section: SystemSection::default(),
            termination_request: None,
            termination_focus: cx.focus_handle(),
            termination_scroll: ScrollHandle::new(),
            termination_focus_requested: false,
            termination_in_progress: false,
            notice: None,
            process_search,
            process_table_scroll: ScrollHandle::new(),
            selected_process: None,
            process_detail_scroll: ScrollHandle::new(),
            presentation,
            _search_subscription: Some(search_subscription),
            _settings_subscription: Some(settings_subscription),
        }
    }

    pub(super) fn refresh_now(&mut self, cx: &mut Context<Self>) {
        self.monitor.refresh_now();
        cx.notify();
    }

    pub(super) fn select_section(&mut self, section: SystemSection, cx: &mut Context<Self>) {
        if self.section != section {
            self.section = section;
            cx.notify();
        }
    }

    pub(super) fn select_process_sort(&mut self, sort: ProcessSort, cx: &mut Context<Self>) {
        self.monitor.set_process_sort(sort);
        cx.notify();
    }

    /// Captures a verified lifetime for read-only details; delayed clicks never target a new PID owner.
    pub(super) fn select_process(
        &mut self,
        identity: StableProcessIdentity,
        name: String,
        cx: &mut Context<Self>,
    ) {
        if identity.start_time_ticks == 0 {
            return;
        }
        if self
            .selected_process
            .as_ref()
            .is_none_or(|selected| selected.identity != identity)
        {
            self.process_detail_scroll = ScrollHandle::new();
            self.selected_process = Some(SelectedProcess { identity, name });
            cx.notify();
        }
    }

    /// Closes only the read-only details; force-quit confirmation keeps its separately captured target.
    pub(super) fn close_process_details(&mut self, cx: &mut Context<Self>) {
        self.selected_process = None;
        self.process_detail_scroll = ScrollHandle::new();
        cx.notify();
    }

    /// Captures a safe target for confirmation; no operating-system operation happens here.
    pub(super) fn prepare_termination(
        &mut self,
        identity: StableProcessIdentity,
        name: String,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        if identity.pid <= 1
            || identity.pid == std::process::id()
            || identity.start_time_ticks == 0
            || self.termination_in_progress
            || self.termination_request.is_some()
        {
            return None;
        }
        self.notice = None;
        self.termination_focus_requested = false;
        self.termination_scroll = ScrollHandle::new();
        let request = TerminationRequest { identity, name };
        let description = request.description();
        self.termination_request = Some(request);
        cx.notify();
        Some(description)
    }

    pub(super) fn cancel_termination(&mut self, cx: &mut Context<Self>) {
        self.termination_request = None;
        self.termination_focus_requested = false;
        cx.notify();
    }

    /// Runs the identity-checked force-quit action off the UI executor.
    pub(super) fn confirm_termination(&mut self, cx: &mut Context<Self>) {
        let Some(request) = self.termination_request.take() else {
            return;
        };
        self.termination_focus_requested = false;
        self.termination_in_progress = true;
        self.notice = None;
        cx.notify();
        let monitor = self.monitor.clone();
        cx.spawn(async move |this, async_cx| {
            let result = async_cx
                .background_executor()
                .spawn(async move { monitor.terminate_process(&request.identity, &request.name) })
                .await;
            let notice = notice_for_termination(result);
            let _ = this.update(async_cx, |view, cx| {
                view.termination_in_progress = false;
                view.notice = Some(notice);
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn set_refresh_interval(
        &mut self,
        interval: RefreshInterval,
        cx: &mut Context<Self>,
    ) {
        self.monitor.set_refresh_interval(interval);
        let rate = match interval {
            RefreshInterval::HalfSecond => ramag_ui::MonitorRefreshRate::HalfSecond,
            RefreshInterval::OneSecond => ramag_ui::MonitorRefreshRate::OneSecond,
            RefreshInterval::TwoSeconds => ramag_ui::MonitorRefreshRate::TwoSeconds,
            RefreshInterval::FiveSeconds => ramag_ui::MonitorRefreshRate::FiveSeconds,
        };
        ramag_ui::save_monitor_settings(ramag_ui::MonitorSettings { refresh_rate: rate }, cx);
        cx.notify();
    }

    pub(super) fn save_presentation(&mut self, cx: &mut Context<Self>) {
        ramag_ui::save_monitor_presentation_settings(self.presentation.clone(), cx);
    }

    #[cfg(target_os = "windows")]
    pub(super) fn set_cpu_temperatures(&mut self, enabled: bool, cx: &mut Context<Self>) {
        let monitor = self.monitor.clone();
        cx.spawn(async move |this, async_cx| {
            let result = async_cx
                .background_executor()
                .spawn(async move { monitor.set_cpu_temperatures(enabled) })
                .await;
            let _ = this.update(async_cx, |view, cx| {
                view.notice = Some(match result {
                    Ok(()) => Notice {
                        message: if enabled {
                            "已请求启用 CPU 温度采集"
                        } else {
                            "已关闭 CPU 温度采集"
                        }
                        .into(),
                        error: false,
                    },
                    Err(reason) => Notice {
                        message: format!("CPU 温度采集设置失败：{reason}"),
                        error: true,
                    },
                });
                cx.notify();
            });
        })
        .detach();
    }
}

mod header;
mod helpers;
mod pages;
mod render;

/// Converts the persisted refresh choice into the sampler's bounded interval.
fn apply_monitor_preferences(monitor: &SystemMonitor, cx: &gpui_kit::App) {
    let rate = match ramag_ui::monitor_settings(cx).refresh_rate {
        ramag_ui::MonitorRefreshRate::HalfSecond => RefreshInterval::HalfSecond,
        ramag_ui::MonitorRefreshRate::OneSecond => RefreshInterval::OneSecond,
        ramag_ui::MonitorRefreshRate::TwoSeconds => RefreshInterval::TwoSeconds,
        ramag_ui::MonitorRefreshRate::FiveSeconds => RefreshInterval::FiveSeconds,
    };
    monitor.set_refresh_interval(rate);
}

/// Applies settings changes to an open view and unregisters with the view lifetime.
fn observe_monitor_preferences(cx: &mut Context<SystemView>) -> gpui_kit::Subscription {
    cx.observe_global::<ramag_ui::MonitorSettingsGlobal>(|this, cx| {
        apply_monitor_preferences(&this.monitor, cx);
        cx.notify();
    })
}
