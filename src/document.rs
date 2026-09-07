use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use tempfile::NamedTempFile;

pub const MAX_FILE_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Document {
    path: PathBuf,
    text: String,
    saved_text: String,
    disk_baseline: Option<Vec<u8>>,
    uses_crlf: bool,
}

impl Document {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let disk_baseline = read_existing_bounded(&path)?;
        let raw_text = match &disk_baseline {
            Some(bytes) => String::from_utf8(bytes.clone())
                .with_context(|| format!("{} is not valid UTF-8", path.display()))?,
            None => String::new(),
        };
        let uses_crlf = raw_text.contains("\r\n");
        let text = if uses_crlf {
            raw_text.replace("\r\n", "\n")
        } else {
            raw_text
        };

        Ok(Self {
            path,
            saved_text: text.clone(),
            text,
            disk_baseline,
            uses_crlf,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn replace_text(&mut self, text: String) {
        self.text = text;
    }

    pub fn is_dirty(&self) -> bool {
        self.text != self.saved_text
    }

    pub fn save(&mut self) -> Result<()> {
        self.verify_unchanged()?;

        let parent = self
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
        let existing_permissions = fs::metadata(&self.path)
            .ok()
            .map(|metadata| metadata.permissions());
        let bytes = self.serialized_bytes();
        let mut temporary = NamedTempFile::new_in(parent).with_context(|| {
            format!("failed to create a temporary file in {}", parent.display())
        })?;
        temporary.write_all(&bytes)?;
        temporary.as_file().sync_all()?;
        if let Some(permissions) = existing_permissions {
            temporary.as_file().set_permissions(permissions)?;
        }

        // Narrow the compare/replace race after potentially slow disk writes.
        self.verify_unchanged()?;
        if self.disk_baseline.is_some() {
            temporary
                .persist(&self.path)
                .map_err(|error| error.error)
                .with_context(|| format!("failed to replace {}", self.path.display()))?;
        } else {
            temporary
                .persist_noclobber(&self.path)
                .map_err(|error| error.error)
                .with_context(|| format!("failed to create {}", self.path.display()))?;
        }
        sync_directory(parent)?;

        self.saved_text.clone_from(&self.text);
        self.disk_baseline = Some(bytes);
        Ok(())
    }

    fn verify_unchanged(&self) -> Result<()> {
        if read_existing_bounded(&self.path)? != self.disk_baseline {
            bail!("file changed on disk; reopen it before saving");
        }
        Ok(())
    }

    fn serialized_bytes(&self) -> Vec<u8> {
        if self.uses_crlf {
            self.text.replace('\n', "\r\n").into_bytes()
        } else {
            self.text.as_bytes().to_vec()
        }
    }
}

fn read_existing_bounded(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!(
                "{} is a symbolic link; open the target directly",
                path.display()
            )
        }
        Ok(metadata) if !metadata.is_file() => bail!("{} is not a regular file", path.display()),
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to inspect {}", path.display()));
        }
    }

    let file = open_regular_nofollow(path)?;
    let metadata = file
        .metadata()
        .with_context(|| format!("failed to inspect open file {}", path.display()))?;
    if !metadata.is_file() {
        bail!("{} is not a regular file", path.display());
    }
    if metadata.len() > MAX_FILE_BYTES {
        bail!(
            "{} is larger than the {} MiB MVP limit",
            path.display(),
            MAX_FILE_BYTES / 1024 / 1024
        );
    }

    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("failed to read {}", path.display()))?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        bail!(
            "{} is larger than the {} MiB MVP limit",
            path.display(),
            MAX_FILE_BYTES / 1024 / 1024
        );
    }
    Ok(Some(bytes))
}

#[cfg(unix)]
fn open_regular_nofollow(path: &Path) -> Result<File> {
    use rustix::fs::{Mode, OFlags, open};

    let fd = open(path, OFlags::RDONLY | OFlags::NOFOLLOW, Mode::empty())
        .with_context(|| format!("failed to open {} without following links", path.display()))?;
    Ok(File::from(fd))
}

#[cfg(not(unix))]
fn open_regular_nofollow(path: &Path) -> Result<File> {
    File::open(path).with_context(|| format!("failed to open {}", path.display()))
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<()> {
    File::open(path)?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn opens_edits_and_atomically_saves() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("note.md");
        fs::write(&path, "old").unwrap();
        let mut document = Document::open(&path).unwrap();
        document.replace_text("new".into());
        assert!(document.is_dirty());
        document.save().unwrap();
        assert_eq!(fs::read_to_string(path).unwrap(), "new");
        assert!(!document.is_dirty());
    }

    #[test]
    fn saves_bare_relative_filename() {
        let dir = tempdir().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        fs::write("note.md", "old").unwrap();
        let mut document = Document::open("note.md").unwrap();
        document.replace_text("new".into());
        let result = document.save();
        std::env::set_current_dir(original_dir).unwrap();
        result.unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("note.md")).unwrap(),
            "new"
        );
        assert!(!document.is_dirty());
    }

    #[test]
    fn preserves_crlf_line_endings() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("note.md");
        fs::write(&path, "a\r\nb\r\n").unwrap();
        let mut document = Document::open(&path).unwrap();
        assert_eq!(document.text(), "a\nb\n");
        document.replace_text("a\nb!\n".into());
        document.save().unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"a\r\nb!\r\n");
    }

    #[test]
    fn refuses_to_overwrite_external_changes() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("note.md");
        fs::write(&path, "old").unwrap();
        let mut document = Document::open(&path).unwrap();
        document.replace_text("mine".into());
        fs::write(&path, "theirs").unwrap();
        assert!(document.save().is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "theirs");
    }

    #[test]
    fn refuses_to_overwrite_file_created_after_open() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("note.md");
        let mut document = Document::open(&path).unwrap();
        document.replace_text("mine".into());
        fs::write(&path, "theirs").unwrap();
        assert!(document.save().is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "theirs");
    }

    #[test]
    fn rejects_non_utf8() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("note.md");
        fs::write(&path, [0xff]).unwrap();
        assert!(Document::open(path).is_err());
    }

    #[test]
    fn rejects_oversized_files() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("large.md");
        let file = File::create(&path).unwrap();
        file.set_len(MAX_FILE_BYTES + 1).unwrap();
        assert!(Document::open(path).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symbolic_links() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let target = dir.path().join("target.md");
        let link = dir.path().join("link.md");
        fs::write(&target, "text").unwrap();
        symlink(target, &link).unwrap();
        assert!(Document::open(link).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn preserves_existing_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let path = dir.path().join("note.md");
        fs::write(&path, "old").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        let mut document = Document::open(&path).unwrap();
        document.replace_text("new".into());
        document.save().unwrap();
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }
}
