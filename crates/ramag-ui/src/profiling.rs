//! 有界的原生 UI 性能采样。
//!
//! 该模块只在 `ui-profiling` 特性下编译。运行时还必须显式设置
//! `RAMAG_UI_PROFILE=1`，因此正常构建和正常启动不会启用 GPUI profiler。

use std::sync::{
    Arc, Mutex,
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
    let report = Arc::new(Mutex::new(ProfileReport::new(started_at)));
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
    let report_for_sampler = Arc::clone(&report);
    let sampler = std::thread::Builder::new()
        .name("ramag-ui-profiler".to_owned())
        .spawn(move || {
            loop {
                std::thread::sleep(SAMPLE_INTERVAL);
                let drained = collector.collect_unseen();
                let elapsed = started_at.elapsed();
                let should_stop = stop_for_sampler.load(Ordering::Acquire);
                let Ok(mut report) = report_for_sampler.lock() else {
                    info!(
                        operation = "ui_profiling_complete",
                        elapsed_ms = elapsed.as_millis() as u64,
                        stop_reason = "report_lock_unavailable",
                        "UI profiling stopped without a final report"
                    );
                    profiler::set_trace_enabled(false);
                    break;
                };
                report.accept(drained);

                if should_stop {
                    report.emit_complete(elapsed, "app_quit");
                    profiler::set_trace_enabled(false);
                    break;
                }

                report.emit_sample(elapsed);

                if elapsed >= config.duration {
                    report.emit_complete(elapsed, "duration_elapsed");
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
    let report_for_quit = Arc::clone(&report);
    cx.on_app_quit(move |_| {
        stop.store(true, Ordering::Release);
        profiler::set_trace_enabled(false);
        if let Ok(mut report) = report_for_quit.lock() {
            report.emit_complete(started_at.elapsed(), "app_quit");
        } else {
            info!(
                operation = "ui_profiling_complete",
                stop_reason = "report_lock_unavailable",
                "UI profiling stopped without a final report"
            );
        }
        async {}
    })
    .detach();
    true
}

struct ProfileReport {
    started_at: Instant,
    completed: bool,
    journal_entries: u64,
    foreground_events: u64,
    input_events: u64,
    draw_events: u64,
    present_events: u64,
    boundary_events: u64,
    presented_boundaries: u64,
    same_window_presented_pairs: u64,
    cross_window_presented_pairs: u64,
    invalid_presented_pairs: u64,
    invalid_time_ranges: u64,
    event_time_regressions: u64,
    timestamps_before_sampler: u64,
    input_window_ids_unavailable: u64,
    journal_lost: u64,
    raw_event_records: u64,
    logged_event_records: usize,
    duration_samples_dropped: u64,
    input_durations_us: Vec<u64>,
    draw_durations_us: Vec<u64>,
    present_durations_us: Vec<u64>,
    last_event_end: Option<Instant>,
}

impl ProfileReport {
    fn new(started_at: Instant) -> Self {
        Self {
            started_at,
            completed: false,
            journal_entries: 0,
            foreground_events: 0,
            input_events: 0,
            draw_events: 0,
            present_events: 0,
            boundary_events: 0,
            presented_boundaries: 0,
            same_window_presented_pairs: 0,
            cross_window_presented_pairs: 0,
            invalid_presented_pairs: 0,
            invalid_time_ranges: 0,
            event_time_regressions: 0,
            timestamps_before_sampler: 0,
            input_window_ids_unavailable: 0,
            journal_lost: 0,
            raw_event_records: 0,
            logged_event_records: 0,
            duration_samples_dropped: 0,
            input_durations_us: Vec::new(),
            draw_durations_us: Vec::new(),
            present_durations_us: Vec::new(),
            last_event_end: None,
        }
    }

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
            self.journal_entries = self.journal_entries.saturating_add(1);
            match entry {
                journal::ForegroundJournalEntry::Event(event) => self.record_event(event),
                journal::ForegroundJournalEntry::Boundary(boundary) => {
                    self.record_boundary(boundary)
                }
                journal::ForegroundJournalEntry::Discontinuity { .. }
                | journal::ForegroundJournalEntry::FrameState(_)
                | journal::ForegroundJournalEntry::PlatformSignal(_)
                | journal::ForegroundJournalEntry::ReentrantFrameSkipped { .. } => {}
            }
        }
    }

    fn record_event(&mut self, event: journal::ForegroundEvent) {
        self.foreground_events = self.foreground_events.saturating_add(1);
        self.record_event_timing(event.start_time(), event.end_time());
        match event {
            journal::ForegroundEvent::Input(input) => {
                self.input_events = self.input_events.saturating_add(1);
                self.input_window_ids_unavailable =
                    self.input_window_ids_unavailable.saturating_add(1);
                let duration_us = duration_us(input.end.saturating_duration_since(input.start));
                Self::push_duration(
                    &mut self.input_durations_us,
                    duration_us,
                    &mut self.duration_samples_dropped,
                );
                let timestamp_start_us = self.timestamp_offset_us(input.start);
                let timestamp_end_us = self.timestamp_offset_us(input.end);
                if self.reserve_raw_event_record() {
                    info!(
                        operation = "ui_profiling_event",
                        event = "input",
                        timestamp_start_us = ?timestamp_start_us,
                        timestamp_end_us = ?timestamp_end_us,
                        window_id = "unavailable_from_gpui_journal",
                        kind = input.kind,
                        duration_us,
                        caused_invalidation = input.caused_invalidation,
                        "foreground input event"
                    );
                }
            }
            journal::ForegroundEvent::Draw(draw) => {
                self.draw_events = self.draw_events.saturating_add(1);
                let draw_duration_us =
                    duration_us(draw.draw_end.saturating_duration_since(draw.draw_start));
                Self::push_duration(
                    &mut self.draw_durations_us,
                    draw_duration_us,
                    &mut self.duration_samples_dropped,
                );
                let timestamp_start_us = self.timestamp_offset_us(draw.draw_start);
                let timestamp_end_us = self.timestamp_offset_us(draw.draw_end);
                if self.reserve_raw_event_record() {
                    info!(
                        operation = "ui_profiling_event",
                        event = "draw",
                        timestamp_start_us = ?timestamp_start_us,
                        timestamp_end_us = ?timestamp_end_us,
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
                let present_duration_us = duration_us(
                    present
                        .present_end
                        .saturating_duration_since(present.present_start),
                );
                Self::push_duration(
                    &mut self.present_durations_us,
                    present_duration_us,
                    &mut self.duration_samples_dropped,
                );
                let timestamp_start_us = self.timestamp_offset_us(present.present_start);
                let timestamp_end_us = self.timestamp_offset_us(present.present_end);
                if self.reserve_raw_event_record() {
                    info!(
                        operation = "ui_profiling_event",
                        event = "present",
                        timestamp_start_us = ?timestamp_start_us,
                        timestamp_end_us = ?timestamp_end_us,
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

    fn record_boundary(&mut self, boundary: journal::IntervalBoundary) {
        self.boundary_events = self.boundary_events.saturating_add(1);
        match boundary {
            journal::IntervalBoundary::Presented(presented) => {
                self.presented_boundaries = self.presented_boundaries.saturating_add(1);
                let same_window = presented.frame.window_id == presented.presentation.window_id;
                if same_window {
                    self.same_window_presented_pairs =
                        self.same_window_presented_pairs.saturating_add(1);
                } else {
                    self.cross_window_presented_pairs =
                        self.cross_window_presented_pairs.saturating_add(1);
                }
                let valid_timing = presented.frame.draw_end <= presented.presentation.present_start;
                if !valid_timing {
                    self.invalid_presented_pairs = self.invalid_presented_pairs.saturating_add(1);
                }
                if self.reserve_raw_event_record() {
                    info!(
                        operation = "ui_profiling_boundary",
                        boundary = "presented",
                        frame_window_id = presented.frame.window_id.as_u64(),
                        present_window_id = presented.presentation.window_id.as_u64(),
                        same_window,
                        valid_timing,
                        frame_draw_start_us = ?self.timestamp_offset_us(presented.frame.draw_start),
                        frame_draw_end_us = ?self.timestamp_offset_us(presented.frame.draw_end),
                        present_start_us = ?self.timestamp_offset_us(presented.presentation.present_start),
                        present_end_us = ?self.timestamp_offset_us(presented.presentation.present_end),
                        "presented frame relationship"
                    );
                }
            }
            journal::IntervalBoundary::FrameSkipped(skipped) => {
                if self.reserve_raw_event_record() {
                    info!(
                        operation = "ui_profiling_boundary",
                        boundary = "frame_skipped",
                        window_id = skipped.window_id.as_u64(),
                        timestamp_us = ?self.timestamp_offset_us(skipped.at),
                        reason = ?skipped.reason,
                        "frame request completed without a draw"
                    );
                }
            }
            journal::IntervalBoundary::Idle { ended_at } => {
                if self.reserve_raw_event_record() {
                    info!(
                        operation = "ui_profiling_boundary",
                        boundary = "idle",
                        timestamp_us = ?self.timestamp_offset_us(ended_at),
                        "foreground journal became idle"
                    );
                }
            }
            journal::IntervalBoundary::PowerTransition { ended_at } => {
                if self.reserve_raw_event_record() {
                    info!(
                        operation = "ui_profiling_boundary",
                        boundary = "power_transition",
                        timestamp_us = ?self.timestamp_offset_us(ended_at),
                        "foreground journal observed a power transition"
                    );
                }
            }
        }
    }

    fn record_event_timing(&mut self, start: Instant, end: Instant) {
        if end < start {
            self.invalid_time_ranges = self.invalid_time_ranges.saturating_add(1);
        }
        if let Some(previous_end) = self.last_event_end
            && end < previous_end
        {
            self.event_time_regressions = self.event_time_regressions.saturating_add(1);
        }
        self.last_event_end = Some(end);
    }

    fn timestamp_offset_us(&mut self, timestamp: Instant) -> Option<u64> {
        match timestamp.checked_duration_since(self.started_at) {
            Some(elapsed) => Some(u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX)),
            None => {
                self.timestamps_before_sampler = self.timestamps_before_sampler.saturating_add(1);
                None
            }
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

    fn emit_complete(&mut self, elapsed: Duration, stop_reason: &'static str) {
        if self.completed {
            return;
        }
        self.completed = true;
        let overflow = self.journal_lost > 0
            || self.duration_samples_dropped > 0
            || self.raw_event_records > self.logged_event_records as u64;
        let sample_status = if self.foreground_events == 0 {
            "no_samples"
        } else {
            "samples_available"
        };
        info!(
            operation = "ui_profiling_complete",
            elapsed_ms = elapsed.as_millis() as u64,
            stop_reason,
            sample_status,
            journal_entries = self.journal_entries,
            foreground_events = self.foreground_events,
            input_events = self.input_events,
            draw_events = self.draw_events,
            present_events = self.present_events,
            boundary_events = self.boundary_events,
            presented_boundaries = self.presented_boundaries,
            same_window_presented_pairs = self.same_window_presented_pairs,
            cross_window_presented_pairs = self.cross_window_presented_pairs,
            invalid_presented_pairs = self.invalid_presented_pairs,
            invalid_time_ranges = self.invalid_time_ranges,
            event_time_regressions = self.event_time_regressions,
            timestamps_before_sampler = self.timestamps_before_sampler,
            input_window_ids_unavailable = self.input_window_ids_unavailable,
            journal_lost = self.journal_lost,
            raw_event_records = self.raw_event_records,
            logged_event_records = self.logged_event_records,
            duration_samples_dropped = self.duration_samples_dropped,
            overflow,
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
    include!("profiling_tests.rs");
}
