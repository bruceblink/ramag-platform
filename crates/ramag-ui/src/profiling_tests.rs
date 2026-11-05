use std::time::{Duration, Instant};

use gpui_kit::profiler::journal::{IntervalBoundary, PresentedFrame};
use gpui_kit::profiler::{DrawPhases, FrameTiming, PresentTiming};
use gpui_kit::WindowId;

use super::ProfileReport;
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

#[test]
fn report_preserves_relative_timestamps_and_marks_missing_history() {
    let started_at = Instant::now();
    let mut report = ProfileReport::new(started_at);
    let before_sampler = started_at
        .checked_sub(Duration::from_millis(1))
        .expect("test instant should support a small subtraction");

    assert_eq!(report.timestamp_offset_us(before_sampler), None);
    assert_eq!(report.timestamps_before_sampler, 1);

    let after_sampler = started_at
        .checked_add(Duration::from_millis(3))
        .expect("test instant should support a small addition");
    assert_eq!(report.timestamp_offset_us(after_sampler), Some(3_000));
}

#[test]
fn report_marks_invalid_and_out_of_order_event_times() {
    let started_at = Instant::now();
    let mut report = ProfileReport::new(started_at);
    let first_start = started_at
        .checked_add(Duration::from_millis(20))
        .expect("test instant should support a small addition");
    let first_end = first_start
        .checked_add(Duration::from_millis(5))
        .expect("test instant should support a small addition");
    report.record_event_timing(first_start, first_end);

    let second_start = started_at
        .checked_add(Duration::from_millis(10))
        .expect("test instant should support a small addition");
    let second_end = second_start
        .checked_add(Duration::from_millis(2))
        .expect("test instant should support a small addition");
    report.record_event_timing(second_start, second_end);

    assert_eq!(report.event_time_regressions, 1);
    assert_eq!(report.invalid_time_ranges, 0);
}

#[test]
fn report_marks_invalid_time_ranges_and_completes_only_once() {
    let started_at = Instant::now();
    let mut report = ProfileReport::new(started_at);
    report.record_event_timing(
        started_at
            .checked_add(Duration::from_millis(4))
            .expect("test instant should support a small addition"),
        started_at
            .checked_add(Duration::from_millis(3))
            .expect("test instant should support a small addition"),
    );

    report.emit_complete(Duration::from_millis(8), "app_quit");
    report.emit_complete(Duration::from_millis(9), "duration_elapsed");

    assert_eq!(report.invalid_time_ranges, 1);
    assert!(report.completed);
}

#[test]
fn report_records_present_duration_from_presented_boundary() {
    let started_at = Instant::now();
    let frame_start = started_at
        .checked_add(Duration::from_millis(10))
        .expect("test instant should support a small addition");
    let present_start = frame_start
        .checked_add(Duration::from_millis(2))
        .expect("test instant should support a small addition");
    let present_end = present_start
        .checked_add(Duration::from_micros(700))
        .expect("test instant should support a small addition");
    let mut report = ProfileReport::new(started_at);
    report.record_boundary(IntervalBoundary::Presented(PresentedFrame {
        frame: FrameTiming {
            window_id: WindowId::from(1),
            dirty_at: Some(frame_start),
            invalidations: 1,
            draw_start: frame_start,
            draw_end: present_start,
            refresh_interval: None,
            signal_at: None,
            phases: DrawPhases::default(),
        },
        presentation: PresentTiming {
            window_id: WindowId::from(1),
            present_start,
            present_end,
            animation_interval: None,
        },
    }));

    assert_eq!(report.presented_boundaries, 1);
    assert_eq!(percentile_95(&report.present_durations_us), Some(700));
}
