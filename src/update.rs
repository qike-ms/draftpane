use std::{
    env,
    ffi::{OsStr, OsString},
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use anyhow::{Context, Result};

use crate::safety::printable;

const INSTALLER: &str = include_str!("../install.sh");

pub fn run() -> Result<()> {
    #[cfg(not(unix))]
    anyhow::bail!("self-update is supported only on macOS and Linux");

    #[cfg(unix)]
    if rustix::process::geteuid().is_root() {
        anyhow::bail!(
            "refusing to update as root; reinstall DraftPane to a user-writable directory"
        );
    }

    let executable = env::current_exe().context("locate the running DraftPane executable")?;
    let install_dir = install_dir_for(&executable)?;

    println!(
        "Checking for a DraftPane update (currently {})...",
        env!("CARGO_PKG_VERSION")
    );
    let output = run_script(INSTALLER, &install_dir, env!("CARGO_PKG_VERSION"))?;
    emit_output(&output)?;

    if !output.status.success() {
        anyhow::bail!("update installer exited with {}", output.status);
    }
    Ok(())
}

fn install_dir_for(executable: &Path) -> Result<PathBuf> {
    if executable.file_name() != Some(OsStr::new("draftpane")) {
        anyhow::bail!(
            "cannot update an executable not named 'draftpane': {}",
            printable(&executable.display().to_string())
        );
    }
    executable
        .parent()
        .map(Path::to_path_buf)
        .context("running executable has no parent directory")
}

fn run_script(script: &str, install_dir: &Path, minimum_version: &str) -> Result<Output> {
    let mut child = Command::new("/bin/sh")
        .args([
            OsStr::new("-s"),
            OsStr::new("--"),
            OsStr::new("--install-dir"),
        ])
        .arg(install_dir)
        .arg("--minimum-version")
        .arg(minimum_version)
        .env_clear()
        .envs(updater_environment())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("start the embedded update installer with /bin/sh")?;

    child
        .stdin
        .take()
        .context("open update installer input")?
        .write_all(script.as_bytes())
        .context("send embedded update installer to /bin/sh")?;

    child
        .wait_with_output()
        .context("wait for the update installer")
}

fn updater_environment() -> Vec<(&'static str, OsString)> {
    let mut environment = vec![("PATH", OsString::from("/usr/bin:/bin:/usr/sbin:/sbin"))];
    for name in ["HOME", "TMPDIR", "SSL_CERT_FILE", "SSL_CERT_DIR"] {
        if let Some(value) = env::var_os(name) {
            environment.push((name, value));
        }
    }
    environment
}

fn emit_output(output: &Output) -> Result<()> {
    let stdout = sanitized_process_output(&output.stdout);
    let stderr = sanitized_process_output(&output.stderr);
    io::stdout()
        .write_all(stdout.as_bytes())
        .context("write update output")?;
    io::stderr()
        .write_all(stderr.as_bytes())
        .context("write update error output")?;
    Ok(())
}

fn sanitized_process_output(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .split('\n')
        .map(printable)
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn update_targets_the_running_binary_directory() {
        assert_eq!(
            install_dir_for(Path::new("/opt/tools/draftpane")).unwrap(),
            Path::new("/opt/tools")
        );
        assert!(install_dir_for(Path::new("/opt/tools/renamed")).is_err());
    }

    #[test]
    fn embedded_script_runner_passes_install_directory_without_shell_interpolation() {
        let dir = tempdir().unwrap();
        let output = run_script(
            "test \"$1\" = --install-dir && test \"$3\" = --minimum-version && test -n \"${HOME:-}\" && printf '%s:%s:%s' \"$2\" \"$4\" \"${PATH:-missing}\"",
            dir.path(),
            "1.2.3",
        )
        .unwrap();

        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!(
                "{}:1.2.3:/usr/bin:/bin:/usr/sbin:/sbin",
                dir.path().to_string_lossy()
            )
        );
    }

    #[test]
    fn updater_output_cannot_emit_terminal_controls() {
        let rendered = sanitized_process_output(b"bad\x1b]52;c;SGk=\x07\nnext\rline");
        assert!(!rendered.chars().any(|ch| ch.is_control() && ch != '\n'));
        assert!(rendered.contains('␛'));
        assert!(rendered.contains('␍'));
    }
}
