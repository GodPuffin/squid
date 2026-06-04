#[test]
fn unique_export_path_avoids_overwriting_existing_file() {
    let dir = tempfile_export_dir();
    std::fs::create_dir_all(&dir).expect("create dir");
    let base = dir.join("users.csv");
    std::fs::write(&base, "existing").expect("seed file");

    let unique = super::unique_export_path(&base);
    assert_eq!(unique, dir.join("users-1.csv"));

    let _ = std::fs::remove_dir_all(dir);
}

fn tempfile_export_dir() -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("squid-export-{stamp}"))
}
