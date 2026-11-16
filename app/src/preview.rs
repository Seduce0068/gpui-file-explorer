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

/// An image ready to render: the format that matches the **content**, plus the
/// bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageBytes {
    pub format: ImageFormat,
    pub bytes: Vec<u8>,
}

/// Detect an image format from the file's leading bytes (magic numbers).
///
/// This is deliberately content-based: a file whose extension lies about its
/// contents is still classified correctly, so the renderer never receives
/// bytes in the wrong format (which would fail asynchronously in GPUI and leave
/// the preview stuck on "loading").
pub fn sniff_image_format(bytes: &[u8]) -> Option<ImageFormat> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        return Some(ImageFormat::Png);
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some(ImageFormat::Jpeg);
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some(ImageFormat::Gif);
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some(ImageFormat::Webp);
    }
    if bytes.starts_with(b"BM") {
        return Some(ImageFormat::Bmp);
    }
    if bytes.starts_with(&[b'I', b'I', 0x2a, 0x00]) || bytes.starts_with(&[b'M', b'M', 0x00, 0x2a])
    {
        return Some(ImageFormat::Tiff);
    }
    if bytes.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
        return Some(ImageFormat::Ico);
    }
    if bytes.len() >= 2 && bytes[0] == b'P' && matches!(bytes[1], b'1'..=b'6') {
        return Some(ImageFormat::Pnm);
    }
    // SVG is text; look for the root element in the first KiB (it may follow an
    // XML declaration or comments).
    let head = &bytes[..bytes.len().min(1024)];
    if let Ok(text) = std::str::from_utf8(head) {
        if text.contains("<svg") {
            return Some(ImageFormat::Svg);
        }
    }
    None
}

/// Read an image file for preview, at most `max_bytes`.
///
/// Safety: symlinks and non-files are refused, and the file size is checked
/// before reading so a huge image is never loaded into memory.
pub fn read_image_preview(
    path: &Path,
    expected: ImageFormat,
    max_bytes: u64,
) -> Result<ImageBytes, String> {
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
    let bytes = std::fs::read(path).map_err(|error| format!("読み取りに失敗しました: {error}"))?;
    match sniff_image_format(&bytes) {
        // Trust the content over the extension when they disagree.
        Some(format) => Ok(ImageBytes { format, bytes }),
        None => Err(format!(
            "画像として読み取れません（拡張子は .{} ですが中身が一致しません）",
            expected.extension()
        )),
    }
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
    if bytes.contains(&0) {
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
    use super::{image_format_for_name, read_image_preview, sniff_image_format, MAX_IMAGE_BYTES};
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

    const PNG_HEADER: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

    #[test]
    fn sniffs_magic_numbers() {
        assert_eq!(sniff_image_format(&PNG_HEADER), Some(ImageFormat::Png));
        assert_eq!(
            sniff_image_format(&[0xff, 0xd8, 0xff, 0xe0]),
            Some(ImageFormat::Jpeg)
        );
        assert_eq!(sniff_image_format(b"GIF89a..."), Some(ImageFormat::Gif));
        let mut webp = b"RIFF".to_vec();
        webp.extend_from_slice(&[0, 0, 0, 0]);
        webp.extend_from_slice(b"WEBP");
        assert_eq!(sniff_image_format(&webp), Some(ImageFormat::Webp));
        assert_eq!(sniff_image_format(b"BM....."), Some(ImageFormat::Bmp));
        assert_eq!(
            sniff_image_format(&[b'I', b'I', 0x2a, 0x00]),
            Some(ImageFormat::Tiff)
        );
        assert_eq!(
            sniff_image_format(&[0x00, 0x00, 0x01, 0x00]),
            Some(ImageFormat::Ico)
        );
        assert_eq!(
            sniff_image_format(
                b"P6
1 1
255
"
            ),
            Some(ImageFormat::Pnm)
        );
        assert_eq!(
            sniff_image_format(br#"<?xml version="1.0"?><svg xmlns="..."></svg>"#),
            Some(ImageFormat::Svg)
        );
        assert_eq!(sniff_image_format(b"hello world"), None);
        assert_eq!(sniff_image_format(b""), None);
    }

    #[test]
    fn content_wins_over_a_wrong_extension() {
        let dir = scratch("mismatch");
        // A PNG that claims to be a .jpg.
        let path = dir.join("actually-png.jpg");
        fs::write(&path, PNG_HEADER).unwrap();
        let read = read_image_preview(&path, ImageFormat::Jpeg, MAX_IMAGE_BYTES).unwrap();
        assert_eq!(read.format, ImageFormat::Png);
        assert_eq!(read.bytes.len(), 8);

        // A text file that claims to be an image.
        let fake = dir.join("not-an-image.png");
        fs::write(&fake, b"just text, no magic").unwrap();
        assert!(read_image_preview(&fake, ImageFormat::Png, MAX_IMAGE_BYTES).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_within_limit_and_rejects_oversized() {
        let dir = scratch("limit");
        let path = dir.join("a.png");
        fs::write(&path, PNG_HEADER).unwrap();
        assert_eq!(
            read_image_preview(&path, ImageFormat::Png, MAX_IMAGE_BYTES)
                .unwrap()
                .bytes
                .len(),
            8
        );

        let big = dir.join("big.png");
        let mut bytes = PNG_HEADER.to_vec();
        bytes.extend(std::iter::repeat_n(0u8, 56));
        fs::write(&big, bytes).unwrap();
        assert!(read_image_preview(&big, ImageFormat::Png, 8).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn refuses_symlinks_and_missing_files() {
        let dir = scratch("types");
        let file = dir.join("real.png");
        fs::write(&file, PNG_HEADER).unwrap();
        #[cfg(unix)]
        {
            let link = dir.join("link.png");
            std::os::unix::fs::symlink(&file, &link).unwrap();
            assert!(read_image_preview(&link, ImageFormat::Png, MAX_IMAGE_BYTES).is_err());
        }
        assert!(
            read_image_preview(&dir.join("nope.png"), ImageFormat::Png, MAX_IMAGE_BYTES).is_err()
        );
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
    fn refuses_binary_with_nul_after_first_8kib() {
        let dir = scratch("binary-late");
        let path = dir.join("late.bin");
        let mut bytes = vec![b'a'; 8 * 1024];
        bytes.extend_from_slice(b"\0tail");
        fs::write(&path, bytes).unwrap();
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
