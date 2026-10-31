//! 有界的原生 UI 性能采样。
//!
//! 该模块只在 `ui-profiling` 特性下编译。运行时还必须显式设置
//! `RAMAG_UI_PROFILE=1`，因此正常构建和正常启动不会启用 GPUI profiler。

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use gpui_kit::App;
use gpui_kit::profiler::{self, journal};
use tracing::info;

const PROFILE_ENV: &str = "RAMAG_UI_PROFILE";
const PROFILE_SECONDS_ENV: &str = "RAMAG_UI_PROFILE_SECONDS";
const DEFAULT_PROFILE_SECONDS: u64 = 120;
const MAX_PROFILE_SECONDS: u64 = 120;
const SAMPLE_INTERVAL: Duration = Duration::from_millis(250);
const MAX_RAW_EVENT_RECORDS: usize = 10_000;
const MAX_DURATION_SAMPLES: usize = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProfileConfig {
    duration: Duration,
}

impl ProfileConfig {
    fn from_env(profile: Option<&str>, seconds: Option<&str>) -> Option<Self> {
        let enabled = profile.is_some_and(|value| {
            value == "1" || value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("yes")
        });
        if !enabled {
            return None;
        }

        let seconds = seconds
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(DEFAULT_PROFILE_SECONDS)
            .clamp(1, MAX_PROFILE_SECONDS);
        Some(Self {
            duration: Duration::from_secs(seconds),
        })
    }
}

/// Starts the opt-in foreground journal sampler and returns whether it started.
pub fn start(cx: &mut App) -> bool {
    let config = ProfileConfig::from_env(
        std::env::var(PROFILE_ENV).ok().as_deref(),
        std::env::var(PROFILE_SECONDS_ENV).ok().as_deref(),
    );
    let Some(config) = config else {
        return false;
    };

    let journal = cx.foreground_journal();
    let mut collector = journal.collector();
    let started_at = Instant::now();
    profiler::set_trace_enabled(true);
    info!(
        operation = "ui_profiling_start",
        duration_ms = config.duration.as_millis() as u64,
        sample_interval_ms = SAMPLE_INTERVAL.as_millis() as u64,
        max_raw_event_records = MAX_RAW_EVENT_RECORDS,
        "UI profiling enabled"
    );

    // Use a dedicated diagnostics thread for the bounded sampler. It keeps the
    // foreground executor and the general background worker pool free for the
    // application while the journal remains safe to collect concurrently.
    let stop = Arc::new(AtomicBool::new(false));
    let stop_for_sampler = Arc::clone(&stop);
    let sampler = std::thread::Builder::new()
        .name("ramag-ui-profiler".to_owned())
        .spawn(move || {
            let mut report = ProfileReport::default();
            loop {
                std::thread::sleep(SAMPLE_INTERVAL);
                if stop_for_sampler.load(Ordering::Acquire) {
                    break;
                }
                let drained = collector.collect_unseen();
                report.accept(drained);
                let elapsed = started_at.elapsed();
                report.emit_sample(elapsed);

                if elapsed >= config.duration {
                    report.emit_complete(elapsed);
                    profiler::set_trace_enabled(false);
                    break;
                }
            }
        });
    if sampler.is_err() {
        profiler::set_trace_enabled(false);
        info!(
            operation = "ui_profiling_start_failed",
            "UI profiler thread could not start"
        );
        return false;
    }

    // Disable trace collection even when the app closes before the bounded
    // sampler reaches its duration limit.
    cx.on_app_quit(move |_| {
        stop.store(true, Ordering::Release);
        profiler::set_trace_enabled(false);
        async {}
    })
    .detach();
    true
}

#[derive(Default)]
struct ProfileReport {
    input_events: u64,
    draw_events: u64,
    present_events: u64,
    journal_lost: u64,
    raw_event_records: u64,
    logged_event_records: usize,
    duration_samples_dropped: u64,
    input_durations_us: Vec<u64>,
    draw_durations_us: Vec<u64>,
    present_durations_us: Vec<u64>,
}

impl ProfileReport {
    fn accept(&mut self, drained: journal::DrainedEntries) {
        self.journal_lost = self.journal_lost.saturating_add(drained.lost);
        if drained.lost > 0 {
            info!(
                operation = "ui_profiling_loss",
                lost = drained.lost,
                "foreground profiling journal entries were unavailable"
            );
        }

        for entry in drained.entries {
            let journal::ForegroundJournalEntry::Event(event) = entry else {
                continue;
            };
            self.record_event(event);
        }
    }

    fn record_event(&mut self, event: journal::ForegroundEvent) {
        match event {
            journal::ForegroundEvent::Input(input) => {
                self.input_events = self.input_events.saturating_add(1);
                let duration_us = duration_us(input.end.duration_since(input.start));
                Self::push_duration(
                    &mut self.input_durations_us,
                    duration_us,
                    &mut self.duration_samples_dropped,
                );
                if self.reserve_raw_event_record() {
                    info!(
                        operation = "ui_profiling_event",
                        event = "input",
                        kind = input.kind,
                        duration_us,
                        caused_invalidation = input.caused_invalidation,
                        "foreground input event"
                    );
                }
            }
            journal::ForegroundEvent::Draw(draw) => {
                self.draw_events = self.draw_events.saturating_add(1);
                let draw_duration_us = duration_us(draw.draw_duration());
                Self::push_duration(
                    &mut self.draw_durations_us,
                    draw_duration_us,
                    &mut self.duration_samples_dropped,
                );
                if self.reserve_raw_event_record() {
                    info!(
                        operation = "ui_profiling_event",
                        event = "draw",
                        window_id = draw.window_id.as_u64(),
                        duration_us = draw_duration_us,
                        request_layout_us = duration_us(draw.phases.request_layout),
                        prepaint_us = duration_us(draw.phases.prepaint),
                        paint_us = duration_us(draw.phases.paint),
                        invalidations = draw.invalidations,
                        refresh_interval_us = ?draw.refresh_interval.map(duration_us),
                        "window draw event"
                    );
                }
            }
            journal::ForegroundEvent::Present(present) => {
                self.present_events = self.present_events.saturating_add(1);
                let present_duration_us = duration_us(present.present_duration());
                Self::push_duration(
                    &mut self.present_durations_us,
                    present_duration_us,
                    &mut self.duration_samples_dropped,
                );
                if self.reserve_raw_event_record() {
                    info!(
                        operation = "ui_profiling_event",
                        event = "present",
                        window_id = present.window_id.as_u64(),
                        duration_us = present_duration_us,
                        animation_interval_us = ?present.animation_interval.map(duration_us),
                        "platform present event"
                    );
                }
            }
            journal::ForegroundEvent::TaskPoll(_)
            | journal::ForegroundEvent::Action(_)
            | journal::ForegroundEvent::View(_)
            | journal::ForegroundEvent::SmallPolls(_) => {}
        }
    }

    fn push_duration(samples: &mut Vec<u64>, value: u64, dropped: &mut u64) {
        if samples.len() < MAX_DURATION_SAMPLES {
            samples.push(value);
        } else {
            *dropped = dropped.saturating_add(1);
        }
    }

    fn reserve_raw_event_record(&mut self) -> bool {
        self.raw_event_records = self.raw_event_records.saturating_add(1);
        if self.logged_event_records < MAX_RAW_EVENT_RECORDS {
            self.logged_event_records += 1;
            true
        } else {
            false
        }
    }

    fn emit_sample(&self, elapsed: Duration) {
        info!(
            operation = "ui_profiling_sample",
            elapsed_ms = elapsed.as_millis() as u64,
            input_events = self.input_events,
            draw_events = self.draw_events,
            present_events = self.present_events,
            journal_lost = self.journal_lost,
            raw_event_records = self.raw_event_records,
            logged_event_records = self.logged_event_records,
            duration_samples_dropped = self.duration_samples_dropped,
            "UI profiling sample"
        );
    }

    fn emit_complete(&mut self, elapsed: Duration) {
        info!(
            operation = "ui_profiling_complete",
            elapsed_ms = elapsed.as_millis() as u64,
            input_events = self.input_events,
            draw_events = self.draw_events,
            present_events = self.present_events,
            journal_lost = self.journal_lost,
            raw_event_records = self.raw_event_records,
            logged_event_records = self.logged_event_records,
            duration_samples_dropped = self.duration_samples_dropped,
            input_p95_us = ?percentile_95(&self.input_durations_us),
            draw_p95_us = ?percentile_95(&self.draw_durations_us),
            present_p95_us = ?percentile_95(&self.present_durations_us),
            "UI profiling complete"
        );
    }
}

fn duration_us(duration: Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}

fn percentile_95(values: &[u64]) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let rank = ((sorted.len() as f64) * 0.95).ceil() as usize;
    sorted
        .get(rank.saturating_sub(1).min(sorted.len() - 1))
        .copied()
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_PROFILE_SECONDS, MAX_PROFILE_SECONDS, ProfileConfig, percentile_95};

    #[test]
    fn profiling_requires_an_explicit_truthy_flag() {
        assert!(ProfileConfig::from_env(None, None).is_none());
        assert!(ProfileConfig::from_env(Some("0"), None).is_none());
        assert!(ProfileConfig::from_env(Some("1"), None).is_some());
        assert!(ProfileConfig::from_env(Some("TRUE"), None).is_some());
    }

    #[test]
    fn profiling_duration_is_bounded() {
        assert_eq!(
            ProfileConfig::from_env(Some("1"), None)
                .expect("enabled profiling should use a default duration")
                .duration
                .as_secs(),
            DEFAULT_PROFILE_SECONDS
        );
        assert_eq!(
            ProfileConfig::from_env(Some("1"), Some("9999"))
                .expect("enabled profiling should clamp the duration")
                .duration
                .as_secs(),
            MAX_PROFILE_SECONDS
        );
        assert_eq!(
            ProfileConfig::from_env(Some("1"), Some("0"))
                .expect("enabled profiling should clamp zero to one second")
                .duration
                .as_secs(),
            1
        );
    }

    #[test]
    fn percentile_uses_the_ceiling_rank() {
        assert_eq!(percentile_95(&[]), None);
        assert_eq!(percentile_95(&[1, 2, 3, 4]), Some(4));
        assert_eq!(percentile_95(&[40, 10, 30, 20, 50]), Some(50));
    }
}
