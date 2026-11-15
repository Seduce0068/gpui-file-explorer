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

use gpui_kit::ImageFormat;

/// Default maximum number of bytes read for a text preview.
pub const MAX_PREVIEW_BYTES: u64 = 256 * 1024;
/// Maximum image size loaded for an image preview (8 MiB).
pub const MAX_IMAGE_BYTES: u64 = 8 * 1024 * 1024;

/// Image format guessed from a file name extension.
pub fn image_format_for_name(name: &str) -> Option<ImageFormat> {
    let extension = name.rsplit_once('.').map(|(_, extension)| extension)?;
    match extension.to_ascii_lowercase().as_str() {
        "png" => Some(ImageFormat::Png),
        "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
        "webp" => Some(ImageFormat::Webp),
        "gif" => Some(ImageFormat::Gif),
        "svg" => Some(ImageFormat::Svg),
        "bmp" => Some(ImageFormat::Bmp),
        "tif" | "tiff" => Some(ImageFormat::Tiff),
        "ico" => Some(ImageFormat::Ico),
        "pbm" | "ppm" | "pgm" | "pnm" => Some(ImageFormat::Pnm),
        _ => None,
    }
}

/// Read an image file for preview, at most `max_bytes`.
///
/// Safety: symlinks and non-files are refused, and the file size is checked
/// before reading so a huge image is never loaded into memory.
pub fn read_image_preview(path: &Path, max_bytes: u64) -> Result<Vec<u8>, String> {
    let link_metadata =
        std::fs::symlink_metadata(path).map_err(|error| format!("読めません: {error}"))?;
    if link_metadata.file_type().is_symlink() {
        return Err("シンボリックリンクはプレビューできません".to_string());
    }
    if !link_metadata.is_file() {
        return Err("ファイルではありません".to_string());
    }
    if link_metadata.len() > max_bytes {
        return Err(format!(
            "画像が大きすぎます（上限 {}）",
            crate::model::human_size(max_bytes)
        ));
    }
    std::fs::read(path).map_err(|error| format!("読み取りに失敗しました: {error}"))
}

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
mod image_tests {
    use super::{image_format_for_name, read_image_preview, MAX_IMAGE_BYTES};
    use gpui_kit::ImageFormat;
    use std::fs;
    use std::path::PathBuf;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gpui-fe-image-preview-{tag}-{}-{}",
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
    fn detects_formats_case_insensitively() {
        assert_eq!(image_format_for_name("a.PNG"), Some(ImageFormat::Png));
        assert_eq!(image_format_for_name("a.jpeg"), Some(ImageFormat::Jpeg));
        assert_eq!(image_format_for_name("a.JpG"), Some(ImageFormat::Jpeg));
        assert_eq!(image_format_for_name("a.svg"), Some(ImageFormat::Svg));
        assert_eq!(image_format_for_name("a.tiff"), Some(ImageFormat::Tiff));
        assert_eq!(image_format_for_name("a.txt"), None);
        assert_eq!(image_format_for_name("noextension"), None);
    }

    #[test]
    fn reads_within_limit_and_rejects_oversized() {
        let dir = scratch("limit");
        let path = dir.join("a.png");
        fs::write(&path, [0x89, b'P', b'N', b'G', 0, 1, 2, 3]).unwrap();
        assert_eq!(read_image_preview(&path, MAX_IMAGE_BYTES).unwrap().len(), 8);

        let big = dir.join("big.png");
        fs::write(&big, vec![0u8; 64]).unwrap();
        assert!(read_image_preview(&big, 8).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn refuses_symlinks_and_missing_files() {
        let dir = scratch("types");
        let file = dir.join("real.png");
        fs::write(&file, [1, 2, 3]).unwrap();
        #[cfg(unix)]
        {
            let link = dir.join("link.png");
            std::os::unix::fs::symlink(&file, &link).unwrap();
            assert!(read_image_preview(&link, MAX_IMAGE_BYTES).is_err());
        }
        assert!(read_image_preview(&dir.join("nope.png"), MAX_IMAGE_BYTES).is_err());
        let _ = fs::remove_dir_all(&dir);
    }
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
