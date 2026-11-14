// SPDX-License-Identifier: AGPL-3.0-or-later
//! Read-only text preview for the selected file (Space / F3).
//!
//! Safety/conservatism:
//! - symlinks are refused (no reading through links),
//! - only regular files are previewed,
//! - at most `max_bytes` are read (larger files are truncated with a flag),
//! - files with a NUL byte in the first 8 KiB are treated as binary and
//!   refused,
//! - invalid UTF-8 is replaced (lossy) instead of failing.
//!
//! GPUI-free so the logic can be unit tested headlessly.

use std::io::Read;
use std::path::Path;

/// Default maximum number of bytes read for a preview.
pub const MAX_PREVIEW_BYTES: u64 = 256 * 1024;

/// Result of reading a text preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    pub text: String,
    /// True when the file is larger than the read limit.
    pub truncated: bool,
    /// Full size of the file in bytes.
    pub size: u64,
}

/// Read a text preview of `path`, reading at most `max_bytes`.
pub fn read_preview(path: &Path, max_bytes: u64) -> Result<Preview, String> {
    let link_metadata =
        std::fs::symlink_metadata(path).map_err(|error| format!("読めません: {error}"))?;
    if link_metadata.file_type().is_symlink() {
        return Err("シンボリックリンクはプレビューできません".to_string());
    }
    if !link_metadata.is_file() {
        return Err("ファイルではありません".to_string());
    }
    let size = link_metadata.len();
    let file = std::fs::File::open(path).map_err(|error| format!("開けません: {error}"))?;
    let mut bytes = Vec::new();
    file.take(max_bytes)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("読み取りに失敗しました: {error}"))?;
    if bytes.iter().take(8 * 1024).any(|byte| *byte == 0) {
        return Err("バイナリファイルはプレビューできません".to_string());
    }
    Ok(Preview {
        text: String::from_utf8_lossy(&bytes).into_owned(),
        truncated: size > max_bytes,
        size,
    })
}

#[cfg(test)]
mod tests {
    use super::{read_preview, MAX_PREVIEW_BYTES};
    use std::fs;
    use std::path::PathBuf;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gpui-fe-preview-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reads_utf8_text() {
        let dir = scratch("text");
        let path = dir.join("a.txt");
        fs::write(&path, b"hello\nworld\n").unwrap();
        let preview = read_preview(&path, MAX_PREVIEW_BYTES).unwrap();
        assert_eq!(preview.text, "hello\nworld\n");
        assert!(!preview.truncated);
        assert_eq!(preview.size, 12);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn truncates_large_files() {
        let dir = scratch("large");
        let path = dir.join("big.txt");
        fs::write(&path, vec![b'a'; 100]).unwrap();
        let preview = read_preview(&path, 10).unwrap();
        assert_eq!(preview.text.len(), 10);
        assert!(preview.truncated);
        assert_eq!(preview.size, 100);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn refuses_binary_files() {
        let dir = scratch("binary");
        let path = dir.join("bin");
        fs::write(&path, b"abc\0def").unwrap();
        assert!(read_preview(&path, MAX_PREVIEW_BYTES).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn refuses_directories_and_symlinks() {
        let dir = scratch("types");
        let sub = dir.join("sub");
        fs::create_dir_all(&sub).unwrap();
        assert!(read_preview(&sub, MAX_PREVIEW_BYTES).is_err());

        #[cfg(unix)]
        {
            let file = dir.join("real.txt");
            fs::write(&file, b"x").unwrap();
            let link = dir.join("link.txt");
            std::os::unix::fs::symlink(&file, &link).unwrap();
            assert!(read_preview(&link, MAX_PREVIEW_BYTES).is_err());
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_is_an_error() {
        let dir = scratch("missing");
        assert!(read_preview(&dir.join("nope.txt"), MAX_PREVIEW_BYTES).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
