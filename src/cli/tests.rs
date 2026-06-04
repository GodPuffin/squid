use super::Cli;

#[test]
fn cli_defaults_from_path_option() {
    let cli: Cli = None::<std::path::PathBuf>.into();
    assert!(cli.path.is_none());
    assert!(!cli.readonly);
    assert!(cli.scheme.is_none());
    assert!(!cli.no_session);
}
