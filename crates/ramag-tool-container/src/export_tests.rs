use std::sync::atomic::{AtomicU64, Ordering};

use super::{ContainerView, write_container_logs};
use ramag_domain::entities::{DockerContainerLogLine, DockerContainerLogs, DockerLogStream};

fn test_root(label: &str) -> std::path::PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "ramag-container-log-export-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn container_log_export_writes_the_same_bounded_visible_text() -> std::io::Result<()> {
    let root = test_root("visible");
    std::fs::create_dir_all(&root)?;
    let path = root.join("container.log");
    let logs = DockerContainerLogs {
        container_id: "container-export".into(),
        lines: vec![
            DockerContainerLogLine {
                stream: DockerLogStream::Stdout,
                message: "ready".into(),
            },
            DockerContainerLogLine {
                stream: DockerLogStream::Stderr,
                message: "警告".into(),
            },
        ],
        bytes: 11,
        dropped_lines: 0,
        truncated: false,
    };

    write_container_logs(&path, &logs).expect("日志导出应成功");
    assert_eq!(
        std::fs::read_to_string(&path)?,
        "stdout: ready\nstderr: 警告"
    );
    assert_eq!(std::fs::read_dir(&root)?.count(), 1);

    std::fs::remove_file(path)?;
    std::fs::remove_dir(root)
}

#[test]
fn container_view_without_service_starts_with_export_idle() {
    let view = ContainerView::without_service();
    assert!(!view.logs_exporting);
}
