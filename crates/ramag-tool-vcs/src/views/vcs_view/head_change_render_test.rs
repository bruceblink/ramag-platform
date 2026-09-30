use super::render_test::add_vcs_window;
use chrono::Utc;
use gpui_kit::TestAppContext;
use ramag_domain::entities::{Commit, CommitId, FileChangeKind, FileStatus, Signature};

/// HEAD 变化后不能继续显示旧提交详情或让旧文件树回包写回当前工作区。
#[gpui_kit::test]
fn head_change_invalidates_commit_detail_target(cx: &mut TestAppContext) {
    let (view, cx) = add_vcs_window(cx);
    view.update(cx, |view, cx| {
        let signature = Signature {
            name: "Author".into(),
            email: "author@example.com".into(),
            timestamp: Utc::now(),
        };
        view.viewing_commit = Some(std::rc::Rc::new(Commit {
            id: CommitId("a".repeat(40)),
            parents: Vec::new(),
            author: signature.clone(),
            committer: signature,
            subject: "old commit".into(),
            body: String::new(),
            refs: Vec::new(),
        }));
        view.commit_files = std::rc::Rc::new(vec![FileStatus {
            path: "old.rs".into(),
            old_path: None,
            staged: None,
            unstaged: Some(FileChangeKind::Modified),
        }]);
        view.loading_commit_files = true;
        let request_seq = view.commit_detail_request_seq;

        view.refresh_after_head_change(cx);

        assert!(view.viewing_commit.is_none());
        assert!(view.commit_files.is_empty());
        assert!(!view.loading_commit_files);
        assert!(view.commit_detail_request_seq > request_seq);
    });
}
