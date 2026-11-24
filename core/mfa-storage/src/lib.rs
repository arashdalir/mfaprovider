//! Local-file [`Storage`] for the vault (TASK-0006). The only crate that touches the
//! filesystem for vault data; `mfa-core` stays I/O-free (ADR-0005).

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use mfa_core::vault::{Storage, StorageError};

/// A vault stored in one file. Writes go to `<name>.tmp` next to it, are synced to disk and
/// then renamed over the vault, so a crash leaves either the old or the new file intact.
pub struct FileStorage {
    path: PathBuf,
}

impl FileStorage {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    fn temp_path(&self) -> Result<PathBuf, StorageError> {
        let mut name = self
            .path
            .file_name()
            .ok_or_else(|| StorageError::new("the vault path has no file name"))?
            .to_os_string();
        name.push(".tmp");
        Ok(self.path.with_file_name(name))
    }
}

impl Storage for FileStorage {
    fn read(&self) -> Result<Option<Vec<u8>>, StorageError> {
        match fs::read(&self.path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(failure(&error)),
        }
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), StorageError> {
        let temp = self.temp_path()?;
        let replaced = write_synced(&temp, bytes)
            .and_then(|()| fs::rename(&temp, &self.path))
            .and_then(|()| sync_parent_directory(&self.path));
        if let Err(error) = replaced {
            let _ = fs::remove_file(&temp);
            return Err(failure(&error));
        }
        Ok(())
    }
}

fn failure(error: &io::Error) -> StorageError {
    StorageError::new(error.to_string())
}

fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Makes the rename itself durable. Windows cannot open a directory for syncing; there the
/// rename is already written through by the filesystem, so this does nothing.
#[cfg_attr(not(unix), allow(clippy::unnecessary_wraps))]
fn sync_parent_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        let parent = match path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            _ => Path::new("."),
        };
        fs::File::open(parent)?.sync_all()
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn storage_in(directory: &Path) -> FileStorage {
        FileStorage::new(directory.join("vault.mfa"))
    }

    fn file_names(directory: &Path) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn a_missing_file_reads_as_no_vault() {
        let directory = tempfile::tempdir().unwrap();
        assert_eq!(storage_in(directory.path()).read().unwrap(), None);
    }

    #[test]
    fn written_bytes_read_back() {
        let directory = tempfile::tempdir().unwrap();
        let mut storage = storage_in(directory.path());
        storage.write(b"first").unwrap();
        assert_eq!(storage.read().unwrap(), Some(b"first".to_vec()));
    }

    #[test]
    fn overwriting_replaces_the_content_and_leaves_no_temporary_file() {
        let directory = tempfile::tempdir().unwrap();
        let mut storage = storage_in(directory.path());
        storage.write(b"first").unwrap();
        storage.write(b"second, and longer").unwrap();
        storage.write(b"third").unwrap();
        assert_eq!(storage.read().unwrap(), Some(b"third".to_vec()));
        assert_eq!(file_names(directory.path()), ["vault.mfa"]);
    }

    #[test]
    fn large_files_survive_intact() {
        let directory = tempfile::tempdir().unwrap();
        let mut storage = storage_in(directory.path());
        let bytes: Vec<u8> = (0..1_048_576_u32).map(|n| (n % 251) as u8).collect();
        storage.write(&bytes).unwrap();
        assert_eq!(storage.read().unwrap(), Some(bytes));
    }

    #[test]
    fn a_leftover_temporary_file_from_a_crash_does_not_get_in_the_way() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("vault.mfa.tmp"),
            b"half-written junk that is longer",
        )
        .unwrap();
        let mut storage = storage_in(directory.path());
        storage.write(b"good").unwrap();
        assert_eq!(storage.read().unwrap(), Some(b"good".to_vec()));
        assert_eq!(file_names(directory.path()), ["vault.mfa"]);
    }

    #[test]
    fn writing_into_a_missing_directory_fails_without_creating_anything() {
        let directory = tempfile::tempdir().unwrap();
        let mut storage = FileStorage::new(directory.path().join("missing").join("vault.mfa"));
        assert!(storage.write(b"x").is_err());
        assert_eq!(file_names(directory.path()).len(), 0);
    }

    #[test]
    fn a_failed_replace_cleans_up_its_temporary_file() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join("vault.mfa")).unwrap();
        let mut storage = storage_in(directory.path());
        assert!(storage.write(b"x").is_err());
        assert_eq!(file_names(directory.path()), ["vault.mfa"]);
    }

    #[test]
    fn a_path_without_a_file_name_is_refused() {
        let mut storage = FileStorage::new("/");
        assert!(storage.write(b"x").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn the_vault_file_is_private_to_the_owner() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let mut storage = storage_in(directory.path());
        storage.write(b"secret").unwrap();
        let mode = fs::metadata(directory.path().join("vault.mfa"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
