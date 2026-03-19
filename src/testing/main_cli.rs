use clap::Parser;
use clap::error::ErrorKind;

use super::Cli;

#[test]
fn cli_accepts_no_path() {
    let cli = Cli::try_parse_from(["squid"]).unwrap();
    assert!(cli.path.is_none());
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
