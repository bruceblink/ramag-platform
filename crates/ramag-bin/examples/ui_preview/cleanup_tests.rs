//! 验证预览退出时只移除本次创建的临时资源。

use super::*;

#[test]
fn removes_preview_resources_and_preserves_neighbor_files() -> anyhow::Result<()> {
    let root = std::env::temp_dir().join(format!(
        "ramag-preview-cleanup-test-{}-{}",
        std::process::id(),
        ConnectionId::new()
    ));
    std::fs::create_dir(&root)?;
    let redb_path = root.join("preview.redb");
    std::fs::write(&redb_path, b"preview")?;
    let monitor_state = root.join("monitor");
    std::fs::create_dir(&monitor_state)?;
    std::fs::write(monitor_state.join("state"), b"preview")?;
    let sqlite_path = root.join("sqlite");
    std::fs::create_dir(&sqlite_path)?;
    std::fs::write(sqlite_path.join("designer.sqlite"), b"preview")?;
    let neighbor = root.join("keep.txt");
    std::fs::write(&neighbor, b"keep")?;
    let sqlite_directory = PreviewSqliteDirectory {
        path: sqlite_path.clone(),
        owned: true,
    };

    cleanup_preview_resources(&redb_path, &monitor_state, Some(sqlite_directory))?;

    assert!(!redb_path.exists());
    assert!(!monitor_state.exists());
    assert!(!sqlite_path.exists());
    assert_eq!(std::fs::read(&neighbor)?, b"keep");
    std::fs::remove_file(neighbor)?;
    std::fs::remove_dir(root)?;
    Ok(())
}
