// SPDX-License-Identifier: AGPL-3.0-or-later
//! Read-only text preview for the selected file (Space / F3).
//!
//! Safety/conservatism:
//! - symlinks are refused (no reading through links),
//! - only regular files are previewed,
//! - at most `max_bytes` are read (larger files are truncated with a flag),
//! - files with a NUL byte in the bytes actually read are treated as binary
//!   and refused,
//! - invalid UTF-8 is replaced (lossy) instead of failing.
//!
//! GPUI-free so the logic can be unit tested headlessly.

use std::io::Read;
use std::path::Path;

use gpui_kit::ImageFormat;

use crate::strings::STATUS_SYMLINK_NO_PREVIEW;

/// Default maximum number of bytes read for a text preview.
pub const MAX_PREVIEW_BYTES: u64 = 256 * 1024;
/// Maximum image size loaded for an image preview (8 MiB).
pub const MAX_IMAGE_BYTES: u64 = 8 * 1024 * 1024;
/// Reject images whose declared pixel count exceeds this (DoS guard: note
/// limits are advisory in `image 0.25`, so check the header ourselves).
pub const MAX_IMAGE_PIXELS: u64 = 64 * 1024 * 1024;

/// Read PNG IHDR width/height (big-endian u32 at bytes 16..24). Other formats
/// are left to the decoder; PNG covers the common compressed-bomb case.
fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((width, height))
}

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
        return Err(STATUS_SYMLINK_NO_PREVIEW.to_string());
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
    // `metadata.len()` can lie on pseudo filesystems; cap the actual read too.
    let file = std::fs::File::open(path).map_err(|error| format!("開けません: {error}"))?;
    let mut bytes = Vec::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("読み取りに失敗しました: {error}"))?;
    if bytes.len() as u64 > max_bytes {
        return Err(format!(
            "画像が大きすぎます（上限 {}）",
            crate::model::human_size(max_bytes)
        ));
    }
    match sniff_image_format(&bytes) {
        Some(format) => {
            if let Some((width, height)) = png_dimensions(&bytes) {
                let pixels = u64::from(width) * u64::from(height);
                if pixels > MAX_IMAGE_PIXELS {
                    return Err(format!(
                        "画像のピクセル数が大きすぎます（{width}x{height}、上限 {MAX_IMAGE_PIXELS}）"
                    ));
                }
            }
            // Trust the content over the extension when they disagree.
            Ok(ImageBytes { format, bytes })
        }
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
        return Err(STATUS_SYMLINK_NO_PREVIEW.to_string());
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

// ---------------------------------------------------------------------------
// F6-34 workflow helpers (GPUI-free so the routing can be unit tested).
// ---------------------------------------------------------------------------

/// Whether a visible listing entry can be shown in the read-only preview.
/// Folders and symlinks are refused, matching `open_preview_selected`.
pub fn is_previewable(entry: &crate::model::Entry) -> bool {
    !entry.is_dir && !entry.is_symlink
}

/// Visible-listing indices of the entries that can be previewed.
pub fn previewable_indices(entries: &[crate::model::Entry]) -> Vec<usize> {
    entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| is_previewable(entry))
        .map(|(index, _)| index)
        .collect()
}

/// Step through previewable entries, wrapping at either end.
///
/// `current` is an index into the *visible* listing (the same slice the list
/// renders). `None` means no previewable entry is focused yet: a positive
/// `delta` starts at the first one and a negative one at the last. Folders and
/// symlinks are skipped so consecutive previewing never gets stuck.
pub fn step_preview_index(
    entries: &[crate::model::Entry],
    current: Option<usize>,
    delta: isize,
) -> Option<usize> {
    let positions = previewable_indices(entries);
    if positions.is_empty() {
        return None;
    }
    let len = positions.len() as isize;
    let next: usize =
        match current.and_then(|index| positions.iter().position(|&slot| slot == index)) {
            Some(position) => ((position as isize + delta).rem_euclid(len)) as usize,
            None => {
                // The focused row is not itself previewable (a folder/symlink, or
                // a stale index after a re-sort). Pick the nearest usable row in
                // the requested direction, wrapping at the edge.
                let index = current.unwrap_or(0);
                let candidate = if delta < 0 {
                    positions.iter().rposition(|&slot| slot < index)
                } else {
                    positions.iter().position(|&slot| slot > index)
                };
                candidate.unwrap_or_else(|| if delta < 0 { positions.len() - 1 } else { 0 })
            }
        };
    positions.get(next).copied()
}

/// Directory to use when opening a previewed file in a new tab.
///
/// Tabs own directories (not files), so the file's parent is opened and the
/// file itself is selected there. Paths without a parent (a bare file name or
/// a filesystem root) have no usable tab target.
pub fn preview_parent(path: &Path) -> Option<&Path> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
}

/// A key press while the preview overlay owns the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewKeyAction {
    Close,
    Next,
    Prev,
    OpenNewTab,
    None,
}

/// Map a keystroke to a preview action without touching the normal keymap.
/// Escape/Space/F3 keep their existing close behaviour; Up/Down and
/// Ctrl+Enter are new and were previously swallowed while previewing.
pub fn preview_key_action(key: &str, control: bool) -> PreviewKeyAction {
    match (key, control) {
        ("escape" | "space" | "f3", _) => PreviewKeyAction::Close,
        ("down", false) => PreviewKeyAction::Next,
        ("up", false) => PreviewKeyAction::Prev,
        ("enter", true) => PreviewKeyAction::OpenNewTab,
        _ => PreviewKeyAction::None,
    }
}

#[cfg(test)]
mod image_tests {
    use super::{
        image_format_for_name, png_dimensions, read_image_preview, sniff_image_format,
        MAX_IMAGE_BYTES,
    };
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
        assert_eq!(image_format_for_name("a.webp"), Some(ImageFormat::Webp));
        assert_eq!(image_format_for_name("a.WEBP"), Some(ImageFormat::Webp));
        assert_eq!(image_format_for_name("a.gif"), Some(ImageFormat::Gif));
        assert_eq!(image_format_for_name("a.GIF"), Some(ImageFormat::Gif));
        assert_eq!(image_format_for_name("a.bmp"), Some(ImageFormat::Bmp));
        assert_eq!(image_format_for_name("a.BMP"), Some(ImageFormat::Bmp));
        assert_eq!(image_format_for_name("a.ico"), Some(ImageFormat::Ico));
        assert_eq!(image_format_for_name("a.ICO"), Some(ImageFormat::Ico));
        assert_eq!(image_format_for_name("a.pbm"), Some(ImageFormat::Pnm));
        assert_eq!(image_format_for_name("a.PPM"), Some(ImageFormat::Pnm));
        assert_eq!(image_format_for_name("a.pgm"), Some(ImageFormat::Pnm));
        assert_eq!(image_format_for_name("a.PNM"), Some(ImageFormat::Pnm));
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
        assert_eq!(sniff_image_format(b"GIF87a..."), Some(ImageFormat::Gif));
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
        // Big-endian TIFF uses the "MM" byte-order mark with magic 0x2a00.
        assert_eq!(
            sniff_image_format(&[b'M', b'M', 0x00, 0x2a]),
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
    fn sniffs_pnm_magic_p1_through_p6_only() {
        // PNM's on-disk magic is P1..P6 (PBM/PGM/PPM in ASCII and raw forms);
        // P0 and P7 are not PNM magics and must not be guessed as any image.
        let cases: &[(&[u8], Option<ImageFormat>)] = &[
            (b"P1", Some(ImageFormat::Pnm)),
            (b"P2", Some(ImageFormat::Pnm)),
            (b"P3", Some(ImageFormat::Pnm)),
            (b"P4", Some(ImageFormat::Pnm)),
            (b"P5", Some(ImageFormat::Pnm)),
            (b"P6", Some(ImageFormat::Pnm)),
            (b"P0", None),
            (b"P7", None),
        ];
        for (magic, expected) in cases {
            assert_eq!(
                sniff_image_format(magic).as_ref(),
                expected.as_ref(),
                "magic {magic:?}"
            );
        }
    }

    #[test]
    fn sniff_image_format_ignores_svg_past_the_first_kib() {
        // The sniffer only decodes the leading 1024 bytes. Keep the positive
        // control on one side of that boundary and the negative case exactly on
        // the other, with otherwise identical ASCII fixtures.
        let mut within = vec![b'a'; 1020];
        within.extend_from_slice(br#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#);
        assert_eq!(sniff_image_format(&within), Some(ImageFormat::Svg));

        let mut beyond = vec![b'a'; 1024];
        beyond.extend_from_slice(br#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#);
        assert_eq!(sniff_image_format(&beyond), None);
    }

    #[test]
    fn sniff_image_format_rejects_svg_with_an_invalid_utf8_header() {
        // The SVG branch decodes only the leading KiB as UTF-8. Invalid bytes
        // there must make the sniffer fall through to None, not guess a format
        // from the remaining ASCII "<svg" text.
        let mut bytes = vec![0xff, 0xfe, 0x80];
        bytes.extend_from_slice(br#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#);
        assert_eq!(sniff_image_format(&bytes), None);
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
    fn reads_image_at_exact_limit() {
        let dir = scratch("exact-limit");
        let path = dir.join("exact.png");
        fs::write(&path, PNG_HEADER).unwrap(); // exactly 8 bytes
        let read = read_image_preview(&path, ImageFormat::Png, 8).unwrap();
        assert_eq!(read.bytes.len(), 8);
        // One byte below the real size trips the post-read guard even though
        // `metadata.len()` reported 8 (which passed the pre-read check).
        let error = read_image_preview(&path, ImageFormat::Png, 7).unwrap_err();
        assert!(error.contains("画像が大きすぎます"), "{error}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_huge_declared_png_dimensions() {
        let dir = scratch("huge-dims");
        let path = dir.join("bomb.png");
        // Valid signature + IHDR chunk declaring 100000x100000 pixels.
        let mut bytes = PNG_HEADER.to_vec();
        bytes.extend_from_slice(&13u32.to_be_bytes()); // IHDR length
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&100_000u32.to_be_bytes());
        bytes.extend_from_slice(&100_000u32.to_be_bytes());
        fs::write(&path, &bytes).unwrap();
        let error = read_image_preview(&path, ImageFormat::Png, 1024).unwrap_err();
        assert!(error.contains("ピクセル数が大きすぎます"), "{error}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn png_dimensions_ignores_non_ihdr_chunks() {
        // A 24-byte PNG-shaped buffer whose 12..16 is not "IHDR": the guard
        // must return None instead of misreading a neighbouring chunk.
        let mut wrong_chunk = PNG_HEADER.to_vec();
        wrong_chunk.extend_from_slice(&13u32.to_be_bytes()); // chunk length
        wrong_chunk.extend_from_slice(b"IDAT"); // not IHDR
        wrong_chunk.extend_from_slice(&10u32.to_be_bytes()); // fake width
        wrong_chunk.extend_from_slice(&10u32.to_be_bytes()); // fake height
        assert_eq!(wrong_chunk.len(), 24);
        assert_eq!(png_dimensions(&wrong_chunk), None);

        // A too-short buffer is likewise refused without panicking.
        assert_eq!(png_dimensions(&wrong_chunk[..23]), None);
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
        let error = read_image_preview(&dir, ImageFormat::Png, MAX_IMAGE_BYTES).unwrap_err();
        assert_eq!(error, "ファイルではありません");
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

#[cfg(test)]
mod workflow_tests {
    use super::{
        preview_key_action, preview_parent, previewable_indices, step_preview_index,
        PreviewKeyAction,
    };
    use crate::model::Entry;
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

    fn entry(name: &str, is_dir: bool, is_symlink: bool) -> Entry {
        Entry {
            name: OsString::from(name),
            name_text: name.to_string(),
            sort_key: name.to_lowercase(),
            path: PathBuf::from(format!("/work/{name}")),
            is_dir,
            is_symlink,
            size: 1,
            modified: None,
        }
    }

    fn listing() -> Vec<Entry> {
        vec![
            entry("sub", true, false),
            entry("a.txt", false, false),
            entry("link.txt", false, true),
            entry("b.txt", false, false),
        ]
    }

    #[test]
    fn previewable_indices_skip_folders_and_symlinks() {
        let entries = listing();
        assert_eq!(previewable_indices(&entries), vec![1, 3]);
        assert!(previewable_indices(&[]).is_empty());
        assert!(previewable_indices(&entries[..1]).is_empty());
    }

    #[test]
    fn preview_navigation_wraps_and_skips_unpreviewable_rows() {
        let entries = listing();
        assert_eq!(step_preview_index(&entries, None, 1), Some(1));
        assert_eq!(step_preview_index(&entries, None, -1), Some(3));
        assert_eq!(step_preview_index(&entries, Some(1), 1), Some(3));
        assert_eq!(step_preview_index(&entries, Some(3), 1), Some(1));
        assert_eq!(step_preview_index(&entries, Some(3), -1), Some(1));
        assert_eq!(step_preview_index(&entries, Some(1), -1), Some(3));
        // Focusing a folder/symlink still lands on a usable neighbour.
        assert_eq!(step_preview_index(&entries, Some(0), 1), Some(1));
        assert_eq!(step_preview_index(&entries, Some(2), -1), Some(1));
        assert_eq!(step_preview_index(&entries, Some(2), 1), Some(3));
    }

    #[test]
    fn preview_navigation_zero_delta_keeps_current_or_falls_forward() {
        let entries = listing();
        // A previewable current row comes back unchanged: with delta 0 the
        // wrapped position arithmetic is a no-op.
        assert_eq!(step_preview_index(&entries, Some(1), 0), Some(1));
        assert_eq!(step_preview_index(&entries, Some(3), 0), Some(3));
        // A non-previewable current row (folder/symlink/stale index) takes the
        // fallback branch. delta 0 is not < 0, so it lands on the first
        // previewable slot after the focused row, wrapping to the first one
        // when the focused row is at or past the last previewable slot.
        assert_eq!(step_preview_index(&entries, Some(0), 0), Some(1));
        assert_eq!(step_preview_index(&entries, Some(2), 0), Some(3));
        assert_eq!(step_preview_index(&entries, Some(9), 0), Some(1));
    }

    #[test]
    fn preview_navigation_without_previewable_entries_is_noop() {
        let entries = vec![entry("dir", true, false), entry("link", false, true)];
        assert_eq!(step_preview_index(&entries, Some(0), 1), None);
        assert_eq!(step_preview_index(&[], None, -1), None);
    }

    #[test]
    fn preview_navigation_forward_wraps_to_first_from_unpreviewable_tail() {
        // The only previewable row ends before the last entry (a trailing
        // symlink). Focusing that tail row and stepping forward must fall back
        // to the first previewable row (the delta >= 0 branch at the edge).
        let entries = vec![entry("a.txt", false, false), entry("link", false, true)];
        assert_eq!(step_preview_index(&entries, Some(1), 1), Some(0));
        // A stale index past the listing wraps the same way.
        assert_eq!(step_preview_index(&entries, Some(9), 1), Some(0));
    }

    #[test]
    fn preview_new_tab_target_is_the_files_parent() {
        assert_eq!(
            preview_parent(Path::new("/work/docs/a.txt")),
            Some(Path::new("/work/docs"))
        );
        assert_eq!(preview_parent(Path::new("/a.txt")), Some(Path::new("/")));
        // A bare file name has no directory to open in a tab.
        assert_eq!(preview_parent(Path::new("a.txt")), None);
        assert_eq!(preview_parent(Path::new("/")), None);
    }

    #[test]
    fn preview_keys_keep_close_and_add_navigation_and_new_tab() {
        assert_eq!(preview_key_action("escape", false), PreviewKeyAction::Close);
        assert_eq!(preview_key_action("space", false), PreviewKeyAction::Close);
        assert_eq!(preview_key_action("f3", false), PreviewKeyAction::Close);
        assert_eq!(preview_key_action("down", false), PreviewKeyAction::Next);
        assert_eq!(preview_key_action("up", false), PreviewKeyAction::Prev);
        assert_eq!(
            preview_key_action("enter", true),
            PreviewKeyAction::OpenNewTab
        );
        // Unmodified Enter and modified arrows stay inert: the normal keymap
        // (Enter = open with the system app, Ctrl+arrows = history) is untouched.
        assert_eq!(preview_key_action("enter", false), PreviewKeyAction::None);
        assert_eq!(preview_key_action("down", true), PreviewKeyAction::None);
        assert_eq!(preview_key_action("f2", false), PreviewKeyAction::None);
    }
}
