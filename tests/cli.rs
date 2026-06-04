use clap::Parser;
use clap::error::ErrorKind;
use squid::cli::Cli;

#[test]
fn cli_accepts_no_path() {
    let cli = Cli::try_parse_from(["squid"]).unwrap();
    assert!(cli.path.is_none());
    assert!(!cli.readonly);
    assert!(!cli.no_session);
}

#[test]
fn cli_accepts_path_argument() {
    let cli = Cli::try_parse_from(["squid", "sakila.db"]).unwrap();
    assert_eq!(cli.path.as_deref(), Some(std::path::Path::new("sakila.db")));
}

#[test]
fn cli_supports_help_flags() {
    for args in [["squid", "--help"], ["squid", "-h"]] {
        let err = Cli::try_parse_from(args).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    }
}

#[test]
fn cli_supports_version_flags() {
    for args in [["squid", "--version"], ["squid", "-V"]] {
        let err = Cli::try_parse_from(args).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::DisplayVersion);
    }
}

#[test]
fn cli_accepts_readonly_flag() {
    let cli = Cli::try_parse_from(["squid", "--readonly", "test.db"]).unwrap();
    assert!(cli.readonly);
    assert_eq!(cli.path.as_deref(), Some(std::path::Path::new("test.db")));
}

#[test]
fn cli_accepts_no_session_flag() {
    let cli = Cli::try_parse_from(["squid", "--no-session", "test.db"]).unwrap();
    assert!(cli.no_session);
}

#[test]
fn cli_accepts_scheme_flag() {
    let cli = Cli::try_parse_from(["squid", "--scheme", "dracula", "test.db"]).unwrap();
    assert_eq!(cli.scheme.as_deref(), Some("dracula"));
}

#[test]
fn cli_scheme_requires_value() {
    let err = Cli::try_parse_from(["squid", "--scheme"]).unwrap_err();
    assert!(matches!(
        err.kind(),
        ErrorKind::MissingRequiredArgument | ErrorKind::InvalidValue
    ));
}
