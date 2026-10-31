use super::*;
use qbsdiff::Bsdiff;
use ramag_domain::entities::ReleaseDelta;

#[test]
fn reconstructs_verified_installer_and_rejects_changed_inputs() {
    let root = std::env::temp_dir().join(format!(
        "ramag-delta-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).expect("test directory");
    let base = root.join("base");
    let patch = root.join("patch");
    let output = root.join("output");
    let old = b"first release installer data";
    let new = b"second release installer data and new content";
    let mut patch_data = Vec::new();
    Bsdiff::new(old, new)
        .compare(&mut patch_data)
        .expect("build patch");
    std::fs::write(&base, old).expect("base");
    std::fs::write(&patch, &patch_data).expect("patch");
    let asset = ReleaseAsset {
        name: "installer".into(),
        download_url: String::new(),
        size: new.len() as u64,
        sha256: Some(hex::encode(Sha256::digest(new))),
        delta: Some(ReleaseDelta {
            from_version: "1.0.0".into(),
            base_name: "base".into(),
            base_size: old.len() as u64,
            base_sha256: hex::encode(Sha256::digest(old)),
            patch_name: "patch".into(),
            patch_size: patch_data.len() as u64,
            patch_sha256: hex::encode(Sha256::digest(&patch_data)),
        }),
    };
    reconstruct(
        &base,
        &patch,
        &output,
        &asset,
        &UpdateCancellation::default(),
    )
    .expect("reconstruct");
    assert_eq!(std::fs::read(&output).expect("output"), new);
    std::fs::remove_file(&output).expect("remove output");
    std::fs::write(&base, b"modified").expect("modify base");
    assert!(
        reconstruct(
            &base,
            &patch,
            &output,
            &asset,
            &UpdateCancellation::default()
        )
        .is_err()
    );
    assert!(!output.exists());
    std::fs::remove_dir_all(root).expect("cleanup test directory");
}
