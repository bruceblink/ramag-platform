//! UI 过渡的共享时长令牌。
//!
//! 这些时长保持输入响应，并为 60 Hz 下的位置和透明度插值保留连续帧。

use std::time::Duration;

/// 卡片或 Activity Bar 工具项移动到新槽位时使用的时长。
pub const REORDER_TRANSITION_DURATION: Duration = Duration::from_millis(180);

/// 加载结果替换占位内容时使用的时长。
pub const LOADING_TRANSITION_DURATION: Duration = Duration::from_millis(160);

#[cfg(test)]
mod tests {
    use super::{LOADING_TRANSITION_DURATION, REORDER_TRANSITION_DURATION};

    #[test]
    fn transition_tokens_stay_inside_the_motion_budget() {
        let loading_ms = LOADING_TRANSITION_DURATION.as_millis();
        let reorder_ms = REORDER_TRANSITION_DURATION.as_millis();
        assert!((120..=180).contains(&loading_ms));
        assert!((120..=180).contains(&reorder_ms));
        assert!(loading_ms <= reorder_ms);
    }
}
