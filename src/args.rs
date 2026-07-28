//! Minimal command-line parser for the viewer executable.

use std::ffi::OsString;
use std::path::PathBuf;

/// Action selected by the supplied command-line arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchAction {
    /// Starts the GUI, optionally opening a file immediately.
    Run {
        /// Markdown file supplied as the single positional argument.
        file: Option<PathBuf>,
    },
    /// Prints command-line help.
    Help,
    /// Prints the package version.
    Version,
}

/// Parses arguments after the executable name.
///
/// Returns an error for unknown options or more than one positional file.
pub fn parse_args<I>(arguments: I) -> Result<LaunchAction, String>
where
    I: IntoIterator<Item = OsString>,
{
    let mut file = None;
    let mut positional_only = false;

    for argument in arguments {
        if !positional_only {
            if argument == "--" {
                positional_only = true;
                continue;
            }
            if argument == "--help" || argument == "-h" {
                return Ok(LaunchAction::Help);
            }
            if argument == "--version" || argument == "-V" {
                return Ok(LaunchAction::Version);
            }
            if argument.to_string_lossy().starts_with('-') {
                return Err(format!("unknown option: {}", argument.to_string_lossy()));
            }
        }

        if file.is_some() {
            return Err("only one Markdown file can be opened per window".to_owned());
        }
        file = Some(PathBuf::from(argument));
    }

    Ok(LaunchAction::Run { file })
}

/// Command-line help shown for `--help` and invalid arguments.
pub const HELP: &str = "\
md-viewer - small read-only Markdown viewer

Usage:
  md-viewer [OPTIONS] [FILE.md]

Options:
  -h, --help       Show this help
  -V, --version    Show the version

You can also open a file with Ctrl+O or drag it onto the window.
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_argument_opens_empty_window() {
        assert_eq!(
            parse_args(Vec::<OsString>::new()).unwrap(),
            LaunchAction::Run { file: None }
        );
    }

    #[test]
    fn accepts_one_path() {
        assert_eq!(
            parse_args([OsString::from("notes.md")]).unwrap(),
            LaunchAction::Run {
                file: Some(PathBuf::from("notes.md"))
            }
        );
    }

    #[test]
    fn double_dash_allows_dash_prefixed_filename() {
        assert_eq!(
            parse_args([OsString::from("--"), OsString::from("-notes.md")]).unwrap(),
            LaunchAction::Run {
                file: Some(PathBuf::from("-notes.md"))
            }
        );
    }

    #[test]
    fn rejects_unknown_option_and_multiple_files() {
        assert!(parse_args([OsString::from("--edit")]).is_err());
        assert!(parse_args([OsString::from("one.md"), OsString::from("two.md")]).is_err());
    }
}
