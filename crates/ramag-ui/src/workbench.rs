//! JetBrains 风格工作区的共享几何令牌。
//!
//! 这些常量只描述跨工具一致的壳层尺寸和收缩规则；领域视图仍负责自己的
//! 数据状态、滚动内容和工具栏动作。把边界集中在这里可以避免数据库、API
//! 和 Kafka 工作区各自发明不一致的侧栏宽度和紧凑断点。

/// 左侧对象导航器的可见宽度下限；低于此宽度时整个导航器收缩为紧凑入口。
pub const WORKBENCH_NAV_MIN_WIDTH: f32 = 240.0;
/// 桌面窗口的默认对象导航器宽度，允许用户通过分隔条继续调整。
pub const WORKBENCH_NAV_DEFAULT_WIDTH: f32 = 300.0;
/// 对象导航器的最大宽度，避免挤压中央工作区。
pub const WORKBENCH_NAV_MAX_WIDTH: f32 = 520.0;
/// 低于此窗口宽度时，导航器必须收缩，优先保护中央工作区。
pub const WORKBENCH_COMPACT_BREAKPOINT: f32 = 720.0;
/// 紧凑 IDE 工具栏的高度。
pub const WORKBENCH_TOOLBAR_HEIGHT: f32 = 32.0;
/// 标签栏的高度。
pub const WORKBENCH_TAB_HEIGHT: f32 = 30.0;
/// 树行、结果行和状态行共用的密集行高基准。
pub const WORKBENCH_DENSE_ROW_HEIGHT: f32 = 26.0;
/// 底部结果状态栏的高度。
pub const WORKBENCH_STATUS_HEIGHT: f32 = 26.0;

/// 根据窗口宽度判断是否应隐藏可调整的对象导航器。
pub fn is_compact_workbench(width: f32) -> bool {
    !width.is_finite() || width < WORKBENCH_COMPACT_BREAKPOINT
}

/// 为首次显示的导航器选择安全初始宽度，保留中央工作区的最小可用空间。
pub fn initial_navigation_width(width: f32) -> f32 {
    if is_compact_workbench(width) {
        0.0
    } else if width < 960.0 {
        WORKBENCH_NAV_MIN_WIDTH
    } else {
        WORKBENCH_NAV_DEFAULT_WIDTH
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_workbench_hides_navigation_below_breakpoint() {
        assert!(is_compact_workbench(WORKBENCH_COMPACT_BREAKPOINT - 1.0));
        assert!(!is_compact_workbench(WORKBENCH_COMPACT_BREAKPOINT));
        assert!(!is_compact_workbench(1440.0));
        assert!(is_compact_workbench(f32::NAN));
    }

    #[test]
    fn initial_navigation_width_preserves_dense_desktop_layout() {
        assert_eq!(initial_navigation_width(640.0), 0.0);
        assert_eq!(initial_navigation_width(800.0), WORKBENCH_NAV_MIN_WIDTH);
        assert_eq!(
            initial_navigation_width(1440.0),
            WORKBENCH_NAV_DEFAULT_WIDTH
        );
    }

    #[test]
    fn shared_dimensions_are_stable_for_headless_geometry_checks() {
        assert_eq!(WORKBENCH_TOOLBAR_HEIGHT, 32.0);
        assert_eq!(WORKBENCH_TAB_HEIGHT, 30.0);
        assert_eq!(WORKBENCH_DENSE_ROW_HEIGHT, 26.0);
        assert_eq!(WORKBENCH_STATUS_HEIGHT, 26.0);
    }
}
