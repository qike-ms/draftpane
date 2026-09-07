mod app;
mod document;
mod editor;
mod markdown;
mod safety;

use std::{env, path::PathBuf, process::ExitCode};

use anyhow::{Context, Result};
use app::App;
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
    let path = parse_path()?;
    let mut terminal = ratatui::init();
    let result = App::open(path).and_then(|app| app.run(&mut terminal));
    ratatui::restore();
    result
}

fn parse_path() -> Result<PathBuf> {
    let mut args = env::args_os().skip(1);
    let path = args
        .next()
        .map(PathBuf::from)
        .context("usage: draftpane <markdown-file>")?;
    if args.next().is_some() {
        anyhow::bail!("usage: draftpane <markdown-file>");
    }
    Ok(path)
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
}
