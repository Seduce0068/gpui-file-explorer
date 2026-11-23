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

/// Width/height declared by an image header. Pure and I/O-free: callers run
/// this on the already-read leading bytes before GPUI starts a decode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageDimensions {
    pub width: u32,
    pub height: u32,
}

fn be_u16(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn le_u16(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn le_u24(bytes: &[u8], at: usize) -> Option<u32> {
    Some(
        u32::from(*bytes.get(at)?)
            | (u32::from(*bytes.get(at + 1)?) << 8)
            | (u32::from(*bytes.get(at + 2)?) << 16),
    )
}

fn le_u32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// Read PNG IHDR width/height (big-endian u32 at bytes 16..24).
fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((width, height))
}

/// GIF logical screen descriptor: two little-endian u16 after the 6-byte magic.
fn gif_dimensions(bytes: &[u8]) -> Option<ImageDimensions> {
    if bytes.len() < 10 || !(bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) {
        return None;
    }
    Some(ImageDimensions {
        width: u32::from(le_u16(bytes, 6)?),
        height: u32::from(le_u16(bytes, 8)?),
    })
}

/// Walk RIFF chunks (skipping ALPH/ICC/animation extras) until a VP8/VP8L/
/// VP8X payload declares the canvas size. Chunk sizes are untrusted, so every
/// offset is bounds-checked and the loop is bounded.
fn webp_dimensions(bytes: &[u8]) -> Option<ImageDimensions> {
    if bytes.len() < 20 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return None;
    }
    let mut offset = 12usize;
    for _ in 0..8 {
        let header_end = offset.checked_add(8)?;
        let size = le_u32(bytes, offset + 4)? as usize;
        let data = header_end;
        let end = data.checked_add(size)?;
        if end > bytes.len() {
            return None;
        }
        match &bytes[offset..offset + 4] {
            b"VP8 " => {
                // 3-byte frame tag + 0x9d 0x01 0x2a sync, then 14-bit W/H.
                if size < 10
                    || bytes[data + 3] != 0x9d
                    || bytes[data + 4] != 0x01
                    || bytes[data + 5] != 0x2a
                {
                    return None;
                }
                return Some(ImageDimensions {
                    width: u32::from(le_u16(bytes, data + 6)? & 0x3fff),
                    height: u32::from(le_u16(bytes, data + 8)? & 0x3fff),
                });
            }
            b"VP8L" => {
                // 0x2f signature + 4 bytes: 14 bits (w-1), 14 bits (h-1).
                if size < 5 || bytes[data] != 0x2f {
                    return None;
                }
                let bits = le_u32(bytes, data + 1)?;
                return Some(ImageDimensions {
                    width: (bits & 0x3fff) + 1,
                    height: ((bits >> 14) & 0x3fff) + 1,
                });
            }
            b"VP8X" => {
                // flags(1) + reserved(3) + canvas_w-1 (24) + canvas_h-1 (24).
                if size < 10 {
                    return None;
                }
                return Some(ImageDimensions {
                    width: le_u24(bytes, data + 4)? + 1,
                    height: le_u24(bytes, data + 7)? + 1,
                });
            }
            _ => {}
        }
        // RIFF chunks are padded to an even byte boundary.
        offset = end.checked_add(size & 1)?;
    }
    None
}

/// JPEG: scan markers after SOI for the first SOF segment. Standalone markers
/// have no length; DHT/JPG/DAC share the 0xC0..0xCF range but are not frames.
fn jpeg_dimensions(bytes: &[u8]) -> Option<ImageDimensions> {
    if !bytes.starts_with(&[0xff, 0xd8]) {
        return None;
    }
    let mut at = 2usize;
    for _ in 0..64 {
        while bytes.get(at) == Some(&0xff) {
            at += 1;
        }
        let marker = *bytes.get(at)?;
        at += 1;
        if marker == 0xd9 || marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
            continue;
        }
        let len = usize::from(be_u16(bytes, at)?);
        if len < 2 {
            return None;
        }
        let is_sof = (0xc0..=0xcf).contains(&marker) && !matches!(marker, 0xc4 | 0xc8 | 0xcc);
        if is_sof {
            if len < 7 {
                return None;
            }
            return Some(ImageDimensions {
                height: u32::from(be_u16(bytes, at + 3)?),
                width: u32::from(be_u16(bytes, at + 5)?),
            });
        }
        at = at.checked_add(len)?;
        if at > bytes.len() {
            return None;
        }
    }
    None
}

/// BMP: 40-byte BITMAPINFOHEADER uses signed i32 W/H; the 12-byte CORE header
/// uses u16. Negative heights (top-down DIBs) are reported as magnitudes.
fn bmp_dimensions(bytes: &[u8]) -> Option<ImageDimensions> {
    // The CORE header is only 22 bytes in total; the longer variants are
    // bounds-checked by the individual field reads below.
    if !bytes.starts_with(b"BM") || bytes.len() < 18 {
        return None;
    }
    match le_u32(bytes, 14)? {
        size if size >= 40 => {
            let width = i32::from_le_bytes(bytes.get(18..22)?.try_into().ok()?);
            let height = i32::from_le_bytes(bytes.get(22..26)?.try_into().ok()?);
            Some(ImageDimensions {
                width: width.unsigned_abs(),
                height: height.unsigned_abs(),
            })
        }
        12 => Some(ImageDimensions {
            width: u32::from(le_u16(bytes, 18)?),
            height: u32::from(le_u16(bytes, 20)?),
        }),
        _ => None,
    }
}

/// TIFF: follow the byte-order-mark and read tags 0x0100/0x0101 from the first
/// IFD. Both SHORT and LONG values fit inline in the 4-byte value field.
fn tiff_dimensions(bytes: &[u8]) -> Option<ImageDimensions> {
    let little = match bytes.get(..4)? {
        b"II*\0" => true,
        b"MM\0*" => false,
        _ => return None,
    };
    let u16_at = |at: usize| -> Option<u16> {
        let pair = bytes.get(at..at + 2)?;
        Some(if little {
            u16::from_le_bytes(pair.try_into().ok()?)
        } else {
            u16::from_be_bytes(pair.try_into().ok()?)
        })
    };
    let u32_at = |at: usize| -> Option<u32> {
        let quad = bytes.get(at..at + 4)?;
        Some(if little {
            u32::from_le_bytes(quad.try_into().ok()?)
        } else {
            u32::from_be_bytes(quad.try_into().ok()?)
        })
    };
    let ifd = u32_at(4)? as usize;
    let count = usize::from(u16_at(ifd)?);
    if count == 0 || count > 512 {
        return None;
    }
    let mut width = None;
    let mut height = None;
    for index in 0..count {
        let at = ifd.checked_add(2)?.checked_add(index.checked_mul(12)?)?;
        if at.checked_add(12)? > bytes.len() {
            return None;
        }
        match u16_at(at)? {
            0x0100 => {
                width = Some(match u16_at(at + 2)? {
                    3 => u32::from(u16_at(at + 8)?),
                    4 => u32_at(at + 8)?,
                    _ => return None,
                })
            }
            0x0101 => {
                height = Some(match u16_at(at + 2)? {
                    3 => u32::from(u16_at(at + 8)?),
                    4 => u32_at(at + 8)?,
                    _ => return None,
                })
            }
            _ => {}
        }
    }
    Some(ImageDimensions {
        width: width?,
        height: height?,
    })
}

/// ICO: the first 16-byte directory entry stores W/H in bytes (0 means 256).
fn ico_dimensions(bytes: &[u8]) -> Option<ImageDimensions> {
    if !bytes.starts_with(&[0x00, 0x00, 0x01, 0x00]) || bytes.len() < 24 {
        return None;
    }
    if le_u16(bytes, 4)? == 0 {
        return None;
    }
    let width = if bytes[6] == 0 {
        256
    } else {
        u32::from(bytes[6])
    };
    let height = if bytes[7] == 0 {
        256
    } else {
        u32::from(bytes[7])
    };
    Some(ImageDimensions { width, height })
}

/// PNM: ASCII width/height tokens after the P1..P6 magic, skipping comments.
fn pnm_dimensions(bytes: &[u8]) -> Option<ImageDimensions> {
    if bytes.len() < 2 || bytes[0] != b'P' || !(b'1'..=b'6').contains(&bytes[1]) {
        return None;
    }
    let mut at = 2usize;
    let mut tokens = [0u32; 2];
    for token in &mut tokens {
        loop {
            while bytes.get(at).is_some_and(u8::is_ascii_whitespace) {
                at += 1;
            }
            if bytes.get(at) == Some(&b'#') {
                while at < bytes.len() && !matches!(bytes[at], b'\n' | b'\r') {
                    at += 1;
                }
                continue;
            }
            break;
        }
        let start = at;
        while bytes.get(at).is_some_and(u8::is_ascii_digit) {
            at += 1;
        }
        if start == at {
            return None;
        }
        *token = std::str::from_utf8(bytes.get(start..at)?)
            .ok()?
            .parse()
            .ok()?;
    }
    Some(ImageDimensions {
        width: tokens[0],
        height: tokens[1],
    })
}

/// Declared raster dimensions, or `None` when the header is truncated or uses
/// a format without a fixed-size dimension field.
///
/// SVG is intentionally excluded: its size is expressed as CSS lengths or a
/// `viewBox` in arbitrary text, and GPUI's vector rasterizer applies its own
/// limits; parsing it here would require a full XML parser and a new
/// dependency. All raster formats supported by [`image_format_for_name`] are
/// covered, so the 64M-pixel guard can no longer be bypassed by changing the
/// file extension.
pub fn declared_dimensions(bytes: &[u8]) -> Option<ImageDimensions> {
    match sniff_image_format(bytes)? {
        ImageFormat::Png => {
            png_dimensions(bytes).map(|(width, height)| ImageDimensions { width, height })
        }
        ImageFormat::Jpeg => jpeg_dimensions(bytes),
        ImageFormat::Webp => webp_dimensions(bytes),
        ImageFormat::Gif => gif_dimensions(bytes),
        ImageFormat::Bmp => bmp_dimensions(bytes),
        ImageFormat::Tiff => tiff_dimensions(bytes),
        ImageFormat::Ico => ico_dimensions(bytes),
        ImageFormat::Pnm => pnm_dimensions(bytes),
        ImageFormat::Svg => None,
    }
}

/// Common pixel-budget predicate for every declared raster size.
pub fn pixels_exceed_limit(width: u32, height: u32) -> bool {
    u64::from(width).saturating_mul(u64::from(height)) > MAX_IMAGE_PIXELS
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

/// Pure, I/O-free decision for NW09 list/grid thumbnails: may this entry be
/// rendered with `gpui_kit::img(path)` instead of a generic file icon?
///
/// The allowlist is [`image_format_for_name`], i.e. the same extensions the
/// existing decoder handles; `.heic` / `.psd` are image *kinds* according to
/// `icons::kind_for_name` but are not decodable here. The reverse gap is real
/// too: `kind_for_name` classifies `.pbm` / `.ppm` / `.pgm` / `.pnm` as
/// `Unknown` even though this module and `gpui_kit::img` support them, so it is
/// deliberately not used as a gate here (follow-up: align the icon mapping).
/// Symlinks are refused just like in [`read_image_preview`], so a thumbnail
/// never reads through a link. `entry_is_file` must come from a metadata stat
/// that rejects FIFOs/sockets/devices: `gpui_kit::img` reads path resources with
/// an unbounded `fs::read`, which never returns for a pipe named `*.png` even
/// when its reported length is `0`. Content is not sniffed: that needs bytes
/// and hence fs I/O. `gpui_kit::img` detects the format asynchronously and
/// calls `.with_fallback(...)` if the contents do not match the extension,
/// leaving the ordinary icon visible. The caller also checks the entry size
/// against [`MAX_IMAGE_BYTES`] before using this helper's result.
pub fn is_thumbnailable(name: &str, entry_is_symlink: bool, entry_is_file: bool) -> bool {
    !entry_is_symlink && entry_is_file && image_format_for_name(name).is_some()
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

/// Pure predicate for the byte-limit guard shared by the pre-read and
/// post-read checks in [`read_image_preview`]. Kept standalone so the
/// post-read branch can be tested directly without a pseudo-filesystem fixture.
fn bytes_exceed_limit(len: u64, max_bytes: u64) -> bool {
    len > max_bytes
}

/// Compose the user-visible error for an image over the byte limit.
///
/// Shared by the pre-read and post-read guards so both speak with one voice,
/// and kept pure so the wording can be asserted without a pseudo-filesystem
/// that lies about its reported size (the normal pre-read guard fires first).
fn image_too_large_error(max_bytes: u64) -> String {
    format!(
        "画像が大きすぎます（上限 {}）",
        crate::model::human_size(max_bytes)
    )
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
    if bytes_exceed_limit(link_metadata.len(), max_bytes) {
        return Err(image_too_large_error(max_bytes));
    }
    // `metadata.len()` can lie on pseudo filesystems; cap the actual read too.
    let file = std::fs::File::open(path).map_err(|error| format!("開けません: {error}"))?;
    let mut bytes = Vec::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("読み取りに失敗しました: {error}"))?;
    if bytes_exceed_limit(bytes.len() as u64, max_bytes) {
        return Err(image_too_large_error(max_bytes));
    }
    match sniff_image_format(&bytes) {
        Some(format) => {
            // BC11: the guard is format-agnostic. A 100000x100000 JPEG/WebP/
            // GIF header must be rejected just like the PNG bomb case.
            if let Some(dims) = declared_dimensions(&bytes) {
                if pixels_exceed_limit(dims.width, dims.height) {
                    return Err(format!(
                        "画像のピクセル数が大きすぎます（{}x{}、上限 {MAX_IMAGE_PIXELS}）",
                        dims.width, dims.height
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
    let file = std::fs::File::open(path).map_err(|error| format!("開けません: {error}"))?;
    let mut bytes = Vec::new();
    // BC12: read one byte past the limit and decide `truncated` from the
    // bytes actually returned. `metadata.len()` lies on pseudo filesystems
    // and may change between the stat and the read (TOCTOU).
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("読み取りに失敗しました: {error}"))?;
    let truncated = bytes_exceed_limit(bytes.len() as u64, max_bytes);
    if truncated {
        let keep = usize::try_from(max_bytes).unwrap_or(usize::MAX);
        bytes.truncate(keep);
    }
    if bytes.contains(&0) {
        return Err("バイナリファイルはプレビューできません".to_string());
    }
    // Prefer the measured lower bound when the reported size is wrong.
    let size = link_metadata.len().max(bytes.len() as u64);
    Ok(Preview {
        text: String::from_utf8_lossy(&bytes).into_owned(),
        truncated,
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
        bytes_exceed_limit, declared_dimensions, image_format_for_name, image_too_large_error,
        is_thumbnailable, pixels_exceed_limit, png_dimensions, read_image_preview,
        sniff_image_format, ImageDimensions, MAX_IMAGE_BYTES, MAX_IMAGE_PIXELS,
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
        assert_eq!(image_format_for_name("a.tif"), Some(ImageFormat::Tiff));
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
    fn thumbnail_decision_accepts_decodable_image_names_only() {
        // Every format listed by `image_format_for_name` is thumbnailable,
        // including hidden files and compound names (the last dot wins).
        for name in [
            "photo.png",
            "PHOTO.JPG",
            "photo.jpeg",
            "picture.webp",
            "anim.GIF",
            "vector.svg",
            "scan.tiff",
            "icon.ico",
            "img.pbm",
            ".hidden.png",
            "holiday.photo.PNG",
        ] {
            assert!(is_thumbnailable(name, false, true), "{name}");
        }
        // Non-images and image *kinds* the decoder cannot handle stay on the
        // ordinary icon path.
        for name in [
            "archive.zip",
            "notes.md",
            "noextension",
            "Makefile",
            "photo.heic",
            "photo.avif",
            "photo.psd",
            "photo.raw",
        ] {
            assert!(!is_thumbnailable(name, false, true), "{name}");
        }
    }

    #[test]
    fn thumbnail_decision_covers_pnm_despite_the_icon_kind_gap() {
        // `icons::kind_for_name` still classifies these as `Unknown`, but both
        // this module's format allowlist and `gpui_kit::img` support them.
        // Pin the expectation here so aligning the icon mapping later cannot
        // silently take their thumbnails away.
        for name in ["img.pbm", "img.ppm", "img.pgm", "img.pnm"] {
            assert!(is_thumbnailable(name, false, true), "{name}");
            assert!(!is_thumbnailable(name, true, true), "{name}");
        }
    }

    #[test]
    fn thumbnail_decision_refuses_symlinks() {
        // A symlink may point outside the listed directory; the preview
        // reader rejects links for the same reason.
        assert!(!is_thumbnailable("photo.png", true, true));
        assert!(is_thumbnailable("photo.png", false, true));
        assert!(!is_thumbnailable("notes.md", true, true));
    }

    #[test]
    fn thumbnail_decision_refuses_non_regular_files() {
        // BC02: a FIFO/socket/device with an image extension and a zero
        // reported size passes the byte cap, but `gpui_kit::img` would block
        // forever on `fs::read`. The caller's metadata stat is the only guard.
        for name in ["photo.png", "anim.GIF", "vector.svg"] {
            assert!(!is_thumbnailable(name, false, false), "{name}");
        }
        assert!(is_thumbnailable("photo.png", false, true));
    }

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
        // A RIFF container is not automatically WebP: only the bytes at 8..12
        // decide that, so a WAVE container must not be guessed as an image.
        let mut wave = b"RIFF".to_vec();
        wave.extend_from_slice(&[0, 0, 0, 0]);
        wave.extend_from_slice(b"WAVEfmt ");
        assert!(wave.len() >= 12);
        assert_eq!(sniff_image_format(&wave), None);
        // An 11-byte buffer has no complete 8..12 form tag: the `len() >= 12`
        // guard must reject it instead of slicing out of bounds.
        let mut short_riff = b"RIFF".to_vec();
        short_riff.extend_from_slice(&[0, 0, 0, 0]);
        short_riff.extend_from_slice(b"WAV");
        assert_eq!(short_riff.len(), 11);
        assert_eq!(sniff_image_format(&short_riff), None);
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
        // One byte below the real size trips the *pre-read* guard here:
        // `metadata.len()` (8) is already greater than `max_bytes` (7), so the
        // file is rejected before any bytes are read. The post-read guard uses
        // the same predicate and is covered directly by `bytes_exceed_limit`.
        let error = read_image_preview(&path, ImageFormat::Png, 7).unwrap_err();
        assert!(error.contains("画像が大きすぎます"), "{error}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn bytes_exceed_limit_covers_post_read_boundary() {
        // Directly tests the predicate used by the post-read guard, including
        // the exact-limit case that `reads_image_at_exact_limit` cannot reach.
        assert!(!bytes_exceed_limit(0, 0));
        assert!(bytes_exceed_limit(1, 0));
        assert!(!bytes_exceed_limit(7, 8));
        assert!(!bytes_exceed_limit(8, 8));
        assert!(bytes_exceed_limit(9, 8));
    }

    #[test]
    fn image_too_large_error_uses_human_size_in_the_wording() {
        // The message composer is shared by the pre-read and post-read guards.
        // A real file cannot reach the post-read branch (metadata.len() would
        // trip first), so assert the exact user-visible wording directly,
        // including its `human_size` rendering.
        assert_eq!(
            image_too_large_error(MAX_IMAGE_BYTES),
            "画像が大きすぎます（上限 8.0 M）"
        );
        assert_eq!(
            image_too_large_error(1024),
            "画像が大きすぎます（上限 1.0 K）"
        );
        assert_eq!(image_too_large_error(0), "画像が大きすぎます（上限 0 B）");
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

    fn webp_chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(id);
        bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(data);
        if data.len() % 2 == 1 {
            bytes.push(0);
        }
        bytes
    }

    fn webp_header(chunk: Vec<u8>) -> Vec<u8> {
        let mut bytes = b"RIFF".to_vec();
        bytes.extend_from_slice(&(chunk.len() as u32 + 4).to_le_bytes());
        bytes.extend_from_slice(b"WEBP");
        bytes.extend_from_slice(&chunk);
        bytes
    }

    /// 40-byte BITMAPINFOHEADER: width/height are signed i32, so a top-down
    /// DIB stores its height as a negative value whose magnitude is the size.
    fn bmp_info_header(width: i32, height: i32) -> Vec<u8> {
        let mut bmp = b"BM".to_vec();
        bmp.extend_from_slice(&[0u8; 8]);
        bmp.extend_from_slice(&0u32.to_le_bytes());
        bmp.extend_from_slice(&40u32.to_le_bytes());
        bmp.extend_from_slice(&width.to_le_bytes());
        bmp.extend_from_slice(&height.to_le_bytes());
        bmp
    }

    /// `marker` is the SOF marker byte; callers pass the frame variants they
    /// want the scanner to recognise (0xc0 baseline, 0xc2 progressive, ...).
    fn jpeg_header(marker: u8, width: u16, height: u16) -> Vec<u8> {
        let mut bytes = vec![0xff, 0xd8, 0xff, marker, 0x00, 0x11, 0x08];
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&[0x01, 0x01, 0x11, 0x00]);
        bytes
    }

    /// `value_type` is the TIFF field type: 3 (SHORT) stores the value inline
    /// in the first two bytes of the value field, 4 (LONG) uses all four.
    fn tiff_header_with_type(little: bool, value_type: u16, width: u32, height: u32) -> Vec<u8> {
        let mut bytes = if little {
            b"II*\0".to_vec()
        } else {
            b"MM\0*".to_vec()
        };
        let word = |value: u16| -> Vec<u8> {
            if little {
                value.to_le_bytes().to_vec()
            } else {
                value.to_be_bytes().to_vec()
            }
        };
        let dword = |value: u32| -> Vec<u8> {
            if little {
                value.to_le_bytes().to_vec()
            } else {
                value.to_be_bytes().to_vec()
            }
        };
        bytes.extend_from_slice(&dword(8)); // IFD offset
        bytes.extend_from_slice(&word(2)); // two entries: width, height
        for (tag, value) in [(0x0100u16, width), (0x0101u16, height)] {
            bytes.extend_from_slice(&word(tag));
            bytes.extend_from_slice(&word(value_type));
            bytes.extend_from_slice(&dword(1)); // count
            if value_type == 3 {
                // SHORT is left-aligned in the 4-byte inline value field.
                bytes.extend_from_slice(&word(value as u16));
                bytes.extend_from_slice(&[0, 0]);
            } else {
                bytes.extend_from_slice(&dword(value));
            }
        }
        bytes
    }

    fn tiff_header(little: bool, width: u32, height: u32) -> Vec<u8> {
        tiff_header_with_type(little, 4, width, height)
    }

    #[test]
    fn declared_dimensions_cover_all_supported_raster_formats() {
        // PNG
        let mut png = PNG_HEADER.to_vec();
        png.extend_from_slice(&13u32.to_be_bytes());
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&1000u32.to_be_bytes());
        png.extend_from_slice(&2000u32.to_be_bytes());
        assert_eq!(
            declared_dimensions(&png),
            Some(ImageDimensions {
                width: 1000,
                height: 2000
            })
        );

        // GIF87a / GIF89a
        let mut gif = b"GIF89a".to_vec();
        gif.extend_from_slice(&640u16.to_le_bytes());
        gif.extend_from_slice(&480u16.to_le_bytes());
        assert_eq!(
            declared_dimensions(&gif),
            Some(ImageDimensions {
                width: 640,
                height: 480
            })
        );

        // WebP: VP8 (lossy), VP8L (lossless), VP8X (extended), and an ALPH
        // chunk before VP8 (alpha channel in extended files).
        let mut vp8 = vec![0u8; 3];
        vp8.extend_from_slice(&[0x9d, 0x01, 0x2a]);
        vp8.extend_from_slice(&640u16.to_le_bytes());
        vp8.extend_from_slice(&480u16.to_le_bytes());
        assert_eq!(
            declared_dimensions(&webp_header(webp_chunk(b"VP8 ", &vp8))),
            Some(ImageDimensions {
                width: 640,
                height: 480
            })
        );
        let mut vp8l = vec![0x2f];
        let bits = (639u32) | (479u32 << 14);
        vp8l.extend_from_slice(&bits.to_le_bytes());
        assert_eq!(
            declared_dimensions(&webp_header(webp_chunk(b"VP8L", &vp8l))),
            Some(ImageDimensions {
                width: 640,
                height: 480
            })
        );
        let mut vp8x = vec![0u8; 4];
        vp8x.extend_from_slice(&639u32.to_le_bytes()[..3]);
        vp8x.extend_from_slice(&479u32.to_le_bytes()[..3]);
        assert_eq!(
            declared_dimensions(&webp_header(webp_chunk(b"VP8X", &vp8x))),
            Some(ImageDimensions {
                width: 640,
                height: 480
            })
        );
        let mut with_alpha = webp_chunk(b"ALPH", &[0u8; 3]);
        with_alpha.extend_from_slice(&webp_chunk(b"VP8 ", &vp8));
        assert_eq!(
            declared_dimensions(&webp_header(with_alpha)),
            Some(ImageDimensions {
                width: 640,
                height: 480
            })
        );

        // JPEG SOF0: width/height are big-endian u16 after the precision byte.
        assert_eq!(
            declared_dimensions(&jpeg_header(0xc0, 640, 480)),
            Some(ImageDimensions {
                width: 640,
                height: 480
            })
        );

        // BMP both DIB header variants.
        for (dib_size, header) in [
            (40u32, bmp_info_header(640, 480)),
            (12u32, {
                let mut bmp = b"BM".to_vec();
                bmp.extend_from_slice(&[0u8; 8]);
                bmp.extend_from_slice(&0u32.to_le_bytes());
                bmp.extend_from_slice(&12u32.to_le_bytes());
                bmp.extend_from_slice(&640u16.to_le_bytes());
                bmp.extend_from_slice(&480u16.to_le_bytes());
                bmp
            }),
        ] {
            let dims = declared_dimensions(&header).unwrap_or_else(|| panic!("BMP {dib_size}"));
            assert_eq!(
                dims,
                ImageDimensions {
                    width: 640,
                    height: 480
                }
            );
        }

        // TIFF: both byte orders.
        for little in [true, false] {
            assert_eq!(
                declared_dimensions(&tiff_header(little, 640, 480)),
                Some(ImageDimensions {
                    width: 640,
                    height: 480
                })
            );
        }

        // ICO: a zero byte means 256.
        let mut ico = vec![0, 0, 1, 0, 1, 0, 0, 0];
        ico.extend_from_slice(&[0u8; 16]);
        assert_eq!(
            declared_dimensions(&ico),
            Some(ImageDimensions {
                width: 256,
                height: 256
            })
        );

        // PNM: ASCII tokens with a comment.
        assert_eq!(
            declared_dimensions(b"P6\n# generated\n640 480\n255\n"),
            Some(ImageDimensions {
                width: 640,
                height: 480
            })
        );

        // SVG has no fixed-size header: it is intentionally not guarded here.
        assert_eq!(declared_dimensions(br#"<svg width="1" height="1"/>"#), None);
    }

    #[test]
    fn declared_dimensions_reads_top_down_bmp_negative_height() {
        // A top-down DIB stores height as a negative i32. A regression from
        // `unsigned_abs()` to `as u32` turns -480 into 4294966816, which would
        // both report a bogus size and trip the pixel guard on a normal image.
        assert_eq!(
            declared_dimensions(&bmp_info_header(640, -480)),
            Some(ImageDimensions {
                width: 640,
                height: 480
            })
        );
        // The negative magnitude must stay inside the shared pixel budget:
        // this is the exact file `read_image_preview` would accept.
        assert!(!pixels_exceed_limit(640, 480));
    }

    #[test]
    fn declared_dimensions_reads_every_jpeg_sof_marker() {
        // Every SOF frame variant carries the same big-endian H/W fields.
        // The old helper only emitted SOF0, leaving progressive (0xc2) and
        // lossless (0xc3/0xc5/...) JPEGs untested.
        for marker in [
            0xc0u8, 0xc1, 0xc2, 0xc3, 0xc5, 0xc6, 0xc7, 0xc9, 0xca, 0xcb, 0xcd, 0xce, 0xcf,
        ] {
            assert_eq!(
                declared_dimensions(&jpeg_header(marker, 640, 480)),
                Some(ImageDimensions {
                    width: 640,
                    height: 480
                }),
                "SOF marker 0x{marker:02x} must be read"
            );
        }
        // DHT / JPG / DAC share the SOF marker range but are not frames, so
        // they must not be mistaken for a dimension-carrying segment.
        for marker in [0xc4u8, 0xc8, 0xcc] {
            assert_eq!(
                declared_dimensions(&jpeg_header(marker, 640, 480)),
                None,
                "0x{marker:02x} is not a SOF marker"
            );
        }
    }

    #[test]
    fn declared_dimensions_reads_tiff_short_and_long_tag_types() {
        for little in [true, false] {
            // Type 3 (SHORT): the value is left-aligned in the 4-byte field.
            assert_eq!(
                declared_dimensions(&tiff_header_with_type(little, 3, 640, 480)),
                Some(ImageDimensions {
                    width: 640,
                    height: 480
                }),
                "SHORT TIFF (little={little})"
            );
            // Type 4 (LONG) stays the reference case.
            assert_eq!(
                declared_dimensions(&tiff_header_with_type(little, 4, 640, 480)),
                Some(ImageDimensions {
                    width: 640,
                    height: 480
                }),
                "LONG TIFF (little={little})"
            );
            // An unsupported type has no inline width/height to read.
            assert_eq!(
                declared_dimensions(&tiff_header_with_type(little, 5, 640, 480)),
                None,
                "RATIONAL TIFF (little={little}) must be refused"
            );
        }
    }

    #[test]
    fn declared_dimensions_refuse_truncated_and_lying_headers() {
        assert_eq!(declared_dimensions(&PNG_HEADER), None);
        assert_eq!(declared_dimensions(b"GIF89a\x80\x02"), None);
        assert_eq!(declared_dimensions(&jpeg_header(0xc0, 1, 1)[..10]), None);
        // A RIFF container that is not WebP must not be parsed as one.
        let mut wave = b"RIFF".to_vec();
        wave.extend_from_slice(&4u32.to_le_bytes());
        wave.extend_from_slice(b"WAVE");
        wave.extend_from_slice(&webp_chunk(b"VP8 ", &[0u8; 10]));
        assert_eq!(declared_dimensions(&wave), None);
    }

    #[test]
    fn huge_declared_dimensions_are_rejected_for_every_format() {
        // BC11: changing the extension (or the actual format) must not bypass
        // the 64M-pixel cap. Build valid-signature headers that only declare
        // an oversized canvas; the sniffer and the guard must both fire.
        let mut png = PNG_HEADER.to_vec();
        png.extend_from_slice(&13u32.to_be_bytes());
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&100_000u32.to_be_bytes());
        png.extend_from_slice(&100_000u32.to_be_bytes());

        let mut gif = b"GIF89a".to_vec();
        gif.extend_from_slice(&30_000u16.to_le_bytes());
        gif.extend_from_slice(&30_000u16.to_le_bytes());

        let mut vp8 = vec![0u8; 3];
        vp8.extend_from_slice(&[0x9d, 0x01, 0x2a]);
        vp8.extend_from_slice(&10_000u16.to_le_bytes());
        vp8.extend_from_slice(&10_000u16.to_le_bytes());
        let webp = webp_header(webp_chunk(b"VP8 ", &vp8));

        let mut bmp = b"BM".to_vec();
        bmp.extend_from_slice(&[0u8; 8]);
        bmp.extend_from_slice(&0u32.to_le_bytes());
        bmp.extend_from_slice(&40u32.to_le_bytes());
        bmp.extend_from_slice(&100_000u32.to_le_bytes());
        bmp.extend_from_slice(&100_000u32.to_le_bytes());

        for (name, bytes) in [
            ("png.png", png),
            ("gif.gif", gif),
            ("webp.webp", webp),
            ("jpeg.jpg", jpeg_header(0xc0, 30_000, 30_000)),
            ("bmp.bmp", bmp),
            ("tiff.tiff", tiff_header(true, 100_000, 100_000)),
            ("pnm.pnm", b"P6\n100000 100000\n255\n".to_vec()),
        ] {
            let dir = scratch("huge-all-formats");
            let path = dir.join(name);
            fs::write(&path, &bytes).unwrap();
            let error =
                read_image_preview(&path, image_format_for_name(name).unwrap(), MAX_IMAGE_BYTES)
                    .unwrap_err();
            assert!(
                error.contains("ピクセル数が大きすぎます"),
                "{name}: {error}"
            );
            let _ = fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn pixels_exceed_limit_uses_the_shared_exact_boundary() {
        assert_eq!(MAX_IMAGE_PIXELS, 8192 * 8192);
        assert!(!pixels_exceed_limit(8192, 8192));
        assert!(pixels_exceed_limit(8192, 8193));
        assert!(pixels_exceed_limit(u32::MAX, u32::MAX));
        assert!(!pixels_exceed_limit(0, u32::MAX));
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
    fn thumbnail_and_image_byte_limits_match_core() {
        // core 側の 8 MiB 上限と app 側の画像プレビュー上限は同じ値でなければならない。
        assert_eq!(
            gfe_core::wasm_view::MAX_THUMBNAIL_BYTES,
            crate::preview::MAX_IMAGE_BYTES
        );
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
    fn truncation_uses_the_bytes_actually_read_at_the_boundary() {
        // BC12: `metadata.len()` is no longer the truncation oracle. Exactly
        // `max_bytes` is complete; one more byte truncates the displayed text
        // to the limit while retaining the full size for the status line.
        let dir = scratch("boundary");
        let exact = dir.join("exact.txt");
        fs::write(&exact, vec![b'a'; 32]).unwrap();
        let preview = read_preview(&exact, 32).unwrap();
        assert_eq!(preview.text.len(), 32);
        assert!(!preview.truncated);
        assert_eq!(preview.size, 32);

        let over = dir.join("over.txt");
        fs::write(&over, vec![b'b'; 33]).unwrap();
        let preview = read_preview(&over, 32).unwrap();
        assert_eq!(preview.text.len(), 32);
        assert!(preview.truncated);
        assert_eq!(preview.size, 33);

        // A NUL in the extra byte is never displayed (it is dropped before the
        // binary check), so an otherwise-text file stays previewable.
        let mut with_nul = vec![b'c'; 32];
        with_nul.push(0);
        let nul_path = dir.join("nul.txt");
        fs::write(&nul_path, with_nul).unwrap();
        assert!(read_preview(&nul_path, 32).is_ok());

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
