//! Reading the one file inside a (possibly password-protected) ZIP archive, in memory
//! (ADR-0014). Nothing is extracted to disk and entry names are never used as paths.
//! The archive knows nothing about OTP: its output is the text a normal import would read.

use std::io::{Cursor, Read};

use thiserror::Error;
use zeroize::Zeroizing;
use zip::ZipArchive;

use crate::import::MAX_IMPORT_BYTES;

const MAX_ENTRIES: usize = 16;
const ZIP_SIGNATURE: &[u8] = b"PK\x03\x04";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArchiveError {
    #[error("the file is not a ZIP archive")]
    NotAnArchive,
    #[error("the archive is larger than 5 MiB, or holds a file that is")]
    TooLarge,
    #[error("the archive holds more than {MAX_ENTRIES} entries")]
    TooManyEntries,
    #[error("the archive holds no file")]
    NoFile,
    #[error("the archive holds more than one file")]
    SeveralFiles,
    #[error("the archive needs a password")]
    PasswordRequired,
    #[error("wrong password, or the file is damaged")]
    WrongPasswordOrDamaged,
}

/// Recognised by content, never by file name.
#[must_use]
pub fn is_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(ZIP_SIGNATURE)
}

/// Content of the single file in the archive. `password` may be empty for an unencrypted one.
///
/// # Errors
/// See [`ArchiveError`]; a wrong password and a damaged archive cannot be told apart.
pub fn read_single_file(
    archive: &[u8],
    password: &[u8],
) -> Result<Zeroizing<Vec<u8>>, ArchiveError> {
    if !is_zip(archive) {
        return Err(ArchiveError::NotAnArchive);
    }
    if archive.len() > MAX_IMPORT_BYTES {
        return Err(ArchiveError::TooLarge);
    }
    let mut zip = ZipArchive::new(Cursor::new(archive)).map_err(|_| damaged())?;
    if zip.len() > MAX_ENTRIES {
        return Err(ArchiveError::TooManyEntries);
    }
    let index = the_only_file(&mut zip)?;
    let (encrypted, declared_size) = {
        let entry = zip.by_index_raw(index).map_err(|_| damaged())?;
        (entry.encrypted(), entry.size())
    };
    if encrypted && password.is_empty() {
        return Err(ArchiveError::PasswordRequired);
    }
    if declared_size > MAX_IMPORT_BYTES as u64 {
        return Err(ArchiveError::TooLarge);
    }
    let entry = zip
        .by_index_decrypt(index, password)
        .map_err(|_| damaged())?;
    read_capped(entry, declared_size)
}

fn damaged() -> ArchiveError {
    ArchiveError::WrongPasswordOrDamaged
}

fn the_only_file(zip: &mut ZipArchive<Cursor<&[u8]>>) -> Result<usize, ArchiveError> {
    let mut found = None;
    for index in 0..zip.len() {
        let is_file = zip.by_index_raw(index).map_err(|_| damaged())?.is_file();
        if is_file {
            if found.is_some() {
                return Err(ArchiveError::SeveralFiles);
            }
            found = Some(index);
        }
    }
    found.ok_or(ArchiveError::NoFile)
}

/// Reads at most the import limit however large the header claims the file to be, so a lying
/// header cannot expand further. The decoder checks the checksum at the end of the data.
fn read_capped(source: impl Read, expected_size: u64) -> Result<Zeroizing<Vec<u8>>, ArchiveError> {
    let limit = MAX_IMPORT_BYTES as u64;
    let capacity = usize::try_from(expected_size.min(limit)).unwrap_or(0);
    let mut content = Zeroizing::new(Vec::with_capacity(capacity));
    source
        .take(limit + 1)
        .read_to_end(&mut content)
        .map_err(|_| damaged())?;
    if content.len() as u64 > limit {
        return Err(ArchiveError::TooLarge);
    }
    Ok(content)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use zip::unstable::write::FileOptionsExt;
    use zip::write::{FileOptions, ZipWriter};
    use zip::CompressionMethod;

    use super::*;

    const PASSWORD: &[u8] = b"correct horse";

    fn options() -> FileOptions<'static, 'static, ()> {
        FileOptions::default().compression_method(CompressionMethod::Deflated)
    }

    fn archive(build: impl FnOnce(&mut ZipWriter<Cursor<Vec<u8>>>)) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        build(&mut writer);
        writer.finish().unwrap().into_inner()
    }

    fn one_file(content: &[u8], password: Option<&[u8]>) -> Vec<u8> {
        archive(|writer| {
            let options = match password {
                Some(password) => options().with_deprecated_encryption(password).unwrap(),
                None => options(),
            };
            writer.start_file("export.txt", options).unwrap();
            writer.write_all(content).unwrap();
        })
    }

    #[test]
    fn an_encrypted_file_opens_with_the_right_password() {
        let zip = one_file(b"otpauth://one\r\nline two", Some(PASSWORD));
        assert_eq!(
            read_single_file(&zip, PASSWORD).unwrap().as_slice(),
            b"otpauth://one\r\nline two"
        );
    }

    #[test]
    fn an_unencrypted_file_opens_without_a_password() {
        let zip = one_file(b"plain", None);
        assert_eq!(read_single_file(&zip, b"").unwrap().as_slice(), b"plain");
    }

    #[test]
    fn a_missing_password_is_asked_for_rather_than_called_wrong() {
        let zip = one_file(b"secret text", Some(PASSWORD));
        assert_eq!(
            read_single_file(&zip, b""),
            Err(ArchiveError::PasswordRequired)
        );
    }

    #[test]
    fn wrong_passwords_never_return_content() {
        let zip = one_file(&vec![b'a'; 4000], Some(PASSWORD));
        for attempt in [
            &b"Correct horse"[..],
            b"correct hors",
            b"x",
            b"correct horse ",
        ] {
            assert_eq!(
                read_single_file(&zip, attempt),
                Err(ArchiveError::WrongPasswordOrDamaged),
                "{attempt:?}"
            );
        }
    }

    #[test]
    fn a_damaged_archive_is_reported_without_content() {
        let mut zip = one_file(&vec![b'a'; 4000], Some(PASSWORD));
        let name_length = usize::from(u16::from_le_bytes([zip[26], zip[27]]));
        let extra_length = usize::from(u16::from_le_bytes([zip[28], zip[29]]));
        let encrypted_data = 30 + name_length + extra_length;
        zip[encrypted_data + 14] ^= 0xFF;
        assert_eq!(
            read_single_file(&zip, PASSWORD),
            Err(ArchiveError::WrongPasswordOrDamaged)
        );
        let truncated = &zip[..zip.len() - 30];
        assert_eq!(
            read_single_file(truncated, PASSWORD),
            Err(ArchiveError::WrongPasswordOrDamaged)
        );
    }

    #[test]
    fn only_archives_are_accepted_by_content() {
        assert_eq!(
            read_single_file(b"otpauth://x", b""),
            Err(ArchiveError::NotAnArchive)
        );
        assert_eq!(read_single_file(b"", b""), Err(ArchiveError::NotAnArchive));
        assert!(is_zip(&one_file(b"x", None)));
        assert!(!is_zip(b"PK"));
    }

    #[test]
    fn exactly_one_file_is_required() {
        let none = archive(|writer| {
            writer.add_directory("folder/", options()).unwrap();
        });
        assert_eq!(read_single_file(&none, b""), Err(ArchiveError::NoFile));

        let two = archive(|writer| {
            for name in ["a.txt", "b.txt"] {
                writer.start_file(name, options()).unwrap();
                writer.write_all(b"x").unwrap();
            }
        });
        assert_eq!(read_single_file(&two, b""), Err(ArchiveError::SeveralFiles));
    }

    #[test]
    fn a_folder_next_to_the_file_is_ignored() {
        let zip = archive(|writer| {
            writer.add_directory("folder/", options()).unwrap();
            writer.start_file("folder/export.txt", options()).unwrap();
            writer.write_all(b"inside").unwrap();
        });
        assert_eq!(read_single_file(&zip, b"").unwrap().as_slice(), b"inside");
    }

    #[test]
    fn many_entries_are_refused() {
        let zip = archive(|writer| {
            for number in 0..=MAX_ENTRIES {
                writer
                    .start_file(format!("{number}.txt"), options())
                    .unwrap();
            }
        });
        assert_eq!(
            read_single_file(&zip, b""),
            Err(ArchiveError::TooManyEntries)
        );
    }

    #[test]
    fn a_file_that_would_expand_beyond_the_limit_is_refused_from_its_header() {
        let zip = one_file(&vec![0_u8; MAX_IMPORT_BYTES + 1], None);
        assert!(
            zip.len() < MAX_IMPORT_BYTES,
            "compresses small, as a bomb would"
        );
        assert_eq!(read_single_file(&zip, b""), Err(ArchiveError::TooLarge));
    }

    #[test]
    fn reading_stops_at_the_limit_whatever_the_header_claimed() {
        let endless = std::io::repeat(b'a');
        assert_eq!(read_capped(endless, 10), Err(ArchiveError::TooLarge));
        let exact = Cursor::new(vec![b'a'; MAX_IMPORT_BYTES]);
        assert_eq!(read_capped(exact, 0).unwrap().len(), MAX_IMPORT_BYTES);
    }

    #[test]
    fn an_archive_over_the_size_limit_is_refused_before_it_is_opened() {
        let mut big = ZIP_SIGNATURE.to_vec();
        big.resize(MAX_IMPORT_BYTES + 1, 0);
        assert_eq!(read_single_file(&big, b""), Err(ArchiveError::TooLarge));
    }
}
