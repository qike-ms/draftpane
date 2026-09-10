mod app;
mod document;
mod editor;
mod markdown;
mod safety;
mod theme;
mod update;

use std::{env, ffi::OsString, io::stdout, panic, path::PathBuf, process::ExitCode};

use anyhow::{Context, Result};
use app::App;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
};
use safety::printable;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Error: {}", printable(&format!("{error:#}")));
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    match parse_command(env::args_os().skip(1))? {
        CliCommand::Edit(path) => run_editor(path),
        CliCommand::Update => update::run(),
        CliCommand::Version => {
            println!("draftpane {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        CliCommand::Help => {
            print!("{USAGE}");
            Ok(())
        }
    }
}

fn run_editor(path: PathBuf) -> Result<()> {
    let mut terminal = ratatui::init();
    if let Err(error) = execute!(stdout(), EnableMouseCapture) {
        ratatui::restore();
        return Err(error).context("enable mouse capture");
    }
    let previous_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = execute!(stdout(), DisableMouseCapture);
        previous_hook(info);
    }));
    let result = App::open(path).and_then(|app| app.run(&mut terminal));
    let mouse_result = execute!(stdout(), DisableMouseCapture).context("disable mouse capture");
    ratatui::restore();
    result.and(mouse_result)
}

const USAGE: &str = "DraftPane — secure terminal Markdown editor\n\n\
Usage:\n  draftpane <markdown-file>\n  draftpane -- <markdown-file>\n  draftpane update\n\n\
Commands:\n  update       Download, verify, and install the latest release\n\n\
Options:\n  -h, --help   Show this help\n  -V, --version  Show the installed version\n";

#[derive(Debug, Eq, PartialEq)]
enum CliCommand {
    Edit(PathBuf),
    Update,
    Version,
    Help,
}

fn parse_command(args: impl IntoIterator<Item = OsString>) -> Result<CliCommand> {
    let mut args = args.into_iter();
    let first = args.next().context(USAGE)?;

    if first == "--" {
        let path = args.next().context(USAGE)?;
        if args.next().is_some() {
            anyhow::bail!("{USAGE}");
        }
        return Ok(CliCommand::Edit(PathBuf::from(path)));
    }
    if args.next().is_some() {
        anyhow::bail!("{USAGE}");
    }

    match first.to_str() {
        Some("update") => Ok(CliCommand::Update),
        Some("-V" | "--version") => Ok(CliCommand::Version),
        Some("-h" | "--help") => Ok(CliCommand::Help),
        _ => Ok(CliCommand::Edit(PathBuf::from(first))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fatal_errors_cannot_emit_document_controls() {
        let error = anyhow::anyhow!("bad \x1b]52;c;SGk=\x07 path");
        let rendered = printable(&format!("{error:#}"));
        assert!(!rendered.chars().any(char::is_control));
    }

    #[test]
    fn parses_editor_update_and_metadata_commands() {
        assert_eq!(
            parse_command([OsString::from("README.md")]).unwrap(),
            CliCommand::Edit(PathBuf::from("README.md"))
        );
        assert_eq!(
            parse_command([OsString::from("update")]).unwrap(),
            CliCommand::Update
        );
        assert_eq!(
            parse_command([OsString::from("--version")]).unwrap(),
            CliCommand::Version
        );
        assert_eq!(
            parse_command([OsString::from("--help")]).unwrap(),
            CliCommand::Help
        );
        assert_eq!(
            parse_command([OsString::from("--"), OsString::from("update")]).unwrap(),
            CliCommand::Edit(PathBuf::from("update"))
        );
        assert!(parse_command(Vec::<OsString>::new()).is_err());
        assert!(parse_command([OsString::from("a.md"), OsString::from("b.md")]).is_err());
    }
}
