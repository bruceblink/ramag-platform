//! 系统工具的 GPUI 视图；采集层保持独立，便于后台刷新和单元测试。

use std::time::Duration;

use gpui_kit::{Context, Window};

use super::{ProcessSort, RefreshInterval, SystemMonitor};
use helpers::notice_for_termination;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum SystemSection {
    #[default]
    Performance,
    Processes,
}

#[derive(Clone, Debug)]
struct TerminationRequest {
    pid: u32,
    name: String,
}

#[derive(Clone, Debug)]
struct Notice {
    message: String,
    error: bool,
}

/// 系统监控和任务管理器的主视图，负责导航、刷新设置和进程操作反馈。
pub struct SystemView {
    monitor: SystemMonitor,
    section: SystemSection,
    termination_request: Option<TerminationRequest>,
    termination_in_progress: bool,
    notice: Option<Notice>,
    /// 订阅工具设置；视图释放时自动注销，采样器不持有设置页面。
    _settings_subscription: Option<gpui_kit::Subscription>,
}

impl SystemView {
    /// 创建视图并启动一次采集以及一个受刷新间隔控制的后台轮询器。
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let monitor = SystemMonitor::new();
        apply_monitor_preferences(&monitor, cx);
        let settings_subscription = observe_monitor_preferences(cx);
        let ticker_monitor = monitor.clone();
        cx.spawn_in(window, async move |this, async_cx| {
            loop {
                async_cx
                    .background_executor()
                    .timer(Duration::from_millis(200))
                    .await;
                if ticker_monitor.refresh_if_due()
                    && this.update_in(async_cx, |_, _, cx| cx.notify()).is_err()
                {
                    break;
                }
            }
        })
        .detach();

        let initial_monitor = monitor.clone();
        cx.spawn(async move |this, async_cx| {
            initial_monitor.refresh_now();
            let _ = this.update(async_cx, |_, cx| cx.notify());
        })
        .detach();

        Self {
            monitor,
            section: SystemSection::default(),
            termination_request: None,
            termination_in_progress: false,
            notice: None,
            _settings_subscription: Some(settings_subscription),
        }
    }

    fn refresh_in_background(&self, cx: &mut Context<Self>) {
        let monitor = self.monitor.clone();
        cx.spawn(async move |this, async_cx| {
            monitor.refresh_now();
            let _ = this.update(async_cx, |_, cx| cx.notify());
        })
        .detach();
    }

    fn refresh_now(&mut self, cx: &mut Context<Self>) {
        self.refresh_in_background(cx);
        cx.notify();
    }

    fn select_section(&mut self, section: SystemSection, cx: &mut Context<Self>) {
        if self.section != section {
            self.section = section;
            cx.notify();
        }
    }

    fn select_process_sort(&mut self, sort: ProcessSort, cx: &mut Context<Self>) {
        self.monitor.set_process_sort(sort);
        cx.notify();
    }

    fn request_termination(&mut self, pid: u32, name: String, cx: &mut Context<Self>) {
        if pid == std::process::id() || self.termination_in_progress {
            return;
        }
        self.notice = None;
        self.termination_request = Some(TerminationRequest { pid, name });
        cx.notify();
    }

    fn cancel_termination(&mut self, cx: &mut Context<Self>) {
        self.termination_request = None;
        cx.notify();
    }

    /// 终止前在后台重新核对 PID 和名称，完成后刷新列表并显示结果。
    fn confirm_termination(&mut self, cx: &mut Context<Self>) {
        let Some(request) = self.termination_request.take() else {
            return;
        };
        self.termination_in_progress = true;
        self.notice = None;
        cx.notify();

        let monitor = self.monitor.clone();
        cx.spawn(async move |this, async_cx| {
            let result = monitor.terminate_process(request.pid, &request.name);
            monitor.refresh_now();
            let notice = notice_for_termination(result);
            let _ = this.update(async_cx, |view, cx| {
                view.termination_in_progress = false;
                view.notice = Some(notice);
                cx.notify();
            });
        })
        .detach();
    }
}

mod header;

/// 将专属设置转换为采样层档位，设置页无需依赖监控线程或系统进程数据。
fn apply_monitor_preferences(monitor: &SystemMonitor, cx: &gpui_kit::App) {
    let rate = match ramag_ui::monitor_settings(cx).refresh_rate {
        ramag_ui::MonitorRefreshRate::OneSecond => RefreshInterval::OneSecond,
        ramag_ui::MonitorRefreshRate::TwoSeconds => RefreshInterval::TwoSeconds,
        ramag_ui::MonitorRefreshRate::FiveSeconds => RefreshInterval::FiveSeconds,
    };
    monitor.set_refresh_interval(rate);
}

/// 保持监控模型与工具设置一致；注销由视图持有的 Subscription 生命周期控制。
fn observe_monitor_preferences(cx: &mut Context<SystemView>) -> gpui_kit::Subscription {
    cx.observe_global::<ramag_ui::MonitorSettingsGlobal>(|this, cx| {
        apply_monitor_preferences(&this.monitor, cx);
        cx.notify();
    })
}
mod helpers;
mod render;
