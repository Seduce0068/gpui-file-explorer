// SPDX-License-Identifier: AGPL-3.0-or-later
//! System (freedesktop) icon theme path resolution.
//!
//! The rest of the app uses the bundled Lucide set via `gpui-kit-assets`, which
//! renders monochrome alpha masks. Dolphin-like *full-colour* file icons come
//! from the installed freedesktop icon theme instead: this module only answers
//! the question "which file on disk is the icon for this entry?". The caller
//! passes the returned [`std::path::Path`] to `gpui_kit::img(&Path)` (a `&str`
//! would be treated as an asset name and would not load an arbitrary file).
//!
//! Resolution is deliberately dependency-light: no D-Bus, no `gio`, no
//! `IconTheme` binding. We search the standard theme directories directly and
//! cache the result (including misses) per `(icon name, target size)`.

// `main.rs` resolves icons via `icon_path_for_entry`. The optional
// `icon_path_for_name` / `roots` helpers are test-only, so the allow stays
// scoped to this file under `-D warnings`.
#![allow(dead_code)]

use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

use crate::icons::{kind_for_name, FileKind};
use crate::model::Entry;

/// Contexts searched inside each theme, in priority order. `mimetypes` holds
/// `text-rust`/`inode-directory`, `places` holds folder variants, and `apps`
/// covers a few executable-specific icons.
const CONTEXTS: [&str; 4] = ["mimetypes", "places", "apps", "emblems"];
const EXTENSIONS: [&str; 2] = ["svg", "png"];

/// Common size directories used by freedesktop themes. Breeze uses plain
/// numbers (`16`, `22`, `24`, `32`, `64`), Adwaita/hicolor use `16x16` and
/// `scalable`. The list is only a preference ladder; existence is checked on
/// disk, so entries absent from a given theme are simply skipped.
const SIZE_LADDER: [u32; 10] = [256, 128, 96, 64, 48, 32, 24, 22, 16, 8];

/// Extension → candidate icon names (most preferred first). Only extensions
/// with a distinctive icon are listed; everything else falls back to the
/// generic icon for its [`FileKind`] (see [`icon_name_for_kind`]).
static EXTENSION_ICONS: &[(&str, &[&str])] = &[
    ("rs", &["text-rust", "text-x-rust"]),
    ("png", &["image-png"]),
    ("jpg", &["image-jpeg"]),
    ("jpeg", &["image-jpeg"]),
    ("gif", &["image-gif"]),
    ("bmp", &["image-bmp"]),
    ("webp", &["image-webp"]),
    ("svg", &["image-svg+xml", "image-x-generic"]),
    ("ico", &["image-x-icon", "image-x-generic"]),
    ("tif", &["image-tiff"]),
    ("tiff", &["image-tiff"]),
    ("pdf", &["application-pdf"]),
    ("zip", &["application-zip"]),
    ("tar", &["application-x-tar"]),
    (
        "gz",
        &["application-x-gzip", "application-x-compressed-tar"],
    ),
    ("tgz", &["application-x-compressed-tar"]),
    ("xz", &["application-x-xz", "application-x-compressed-tar"]),
    ("txz", &["application-x-compressed-tar"]),
    (
        "bz2",
        &["application-x-bzip", "application-x-compressed-tar"],
    ),
    ("tbz2", &["application-x-compressed-tar"]),
    ("7z", &["application-x-7z", "application-x-archive"]),
    ("rar", &["application-x-rar", "application-x-archive"]),
    ("zst", &["application-zstd", "application-x-archive"]),
    ("deb", &["application-x-deb"]),
    ("rpm", &["application-x-rpm"]),
    (
        "appimage",
        &["application-x-appimage", "application-x-executable"],
    ),
    ("jar", &["application-java", "application-zip"]),
    ("war", &["application-java", "application-zip"]),
    ("iso", &["application-x-iso", "application-x-cd-image"]),
    ("mp3", &["audio-mpeg"]),
    ("flac", &["audio-flac"]),
    ("wav", &["audio-x-wav", "audio-x-generic"]),
    ("ogg", &["audio-x-vorbis+ogg", "audio-x-generic"]),
    ("oga", &["audio-x-vorbis+ogg", "audio-x-generic"]),
    ("m4a", &["audio-mp4", "audio-x-generic"]),
    ("aac", &["audio-mp4", "audio-x-generic"]),
    ("mp4", &["video-mp4"]),
    ("m4v", &["video-mp4"]),
    ("mkv", &["video-x-matroska", "video-x-generic"]),
    ("avi", &["video-x-msvideo", "video-x-generic"]),
    ("mov", &["video-quicktime", "video-x-generic"]),
    ("webm", &["video-webm", "video-x-generic"]),
    ("txt", &["text-plain", "text-x-generic"]),
    ("md", &["text-x-markdown", "text-plain", "text-x-generic"]),
    ("markdown", &["text-x-markdown", "text-plain"]),
    ("log", &["text-x-log", "text-plain"]),
    ("csv", &["text-csv", "text-x-generic"]),
    ("diff", &["text-x-diff", "text-x-generic"]),
    ("patch", &["text-x-diff", "text-x-generic"]),
    ("html", &["text-html"]),
    ("htm", &["text-html"]),
    ("css", &["text-css"]),
    ("js", &["application-javascript", "text-x-script"]),
    ("mjs", &["application-javascript", "text-x-script"]),
    ("cjs", &["application-javascript", "text-x-script"]),
    ("ts", &["text-x-typescript", "text-x-generic"]),
    ("tsx", &["text-x-typescript", "text-x-generic"]),
    ("py", &["text-x-python", "text-x-script"]),
    ("rb", &["text-x-ruby", "text-x-script"]),
    ("sh", &["application-x-shellscript", "text-x-script"]),
    ("bash", &["application-x-shellscript", "text-x-script"]),
    ("zsh", &["application-x-shellscript", "text-x-script"]),
    ("fish", &["application-x-shellscript", "text-x-script"]),
    ("sql", &["text-x-sql", "text-x-generic"]),
    ("xml", &["text-xml", "text-x-generic"]),
    ("json", &["application-json", "text-x-generic"]),
    ("yaml", &["text-x-yaml", "text-x-generic"]),
    ("yml", &["text-x-yaml", "text-x-generic"]),
    ("toml", &["text-x-generic"]),
];

/// Resolves system icon paths. Cheap to construct; it only records search
/// roots and the theme preference order. Filesystem lookups happen lazily in
/// [`Self::icon_path_for_entry`] / [`Self::icon_path_for_name`] and are cached.
#[derive(Debug)]
pub struct SystemIconResolver {
    /// `(icon name, target size) -> resolved path (or a cached miss)`.
    cache: HashMap<(String, u32), Option<PathBuf>>,
    /// Search roots in priority order: user overrides first, then system.
    roots: Vec<PathBuf>,
    /// Theme directory names in priority order (Breeze first on KDE, or
    /// Breeze-dark first once [`Self::set_prefer_dark`] is enabled).
    themes: Vec<String>,
    /// Whether the dark sibling of a theme outranks its light variant. Kept
    /// separately so [`Self::set_prefer_dark`] can skip cache invalidation when
    /// the effective theme did not actually change.
    prefer_dark: bool,
}

impl Default for SystemIconResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemIconResolver {
    /// Build a resolver using `XDG_CURRENT_DESKTOP` and the standard freedesktop
    /// data directories. No I/O other than reading environment variables.
    pub fn new() -> Self {
        let mut roots = Vec::new();
        if let Some(local_data) = dirs::data_local_dir() {
            push_unique_path(&mut roots, local_data.join("icons"));
        }
        push_unique_path(&mut roots, PathBuf::from("/usr/share/icons"));
        push_unique_path(&mut roots, PathBuf::from("/usr/local/share/icons"));

        let desktop = env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        let kde = desktop
            .split(':')
            .any(|part| part.eq_ignore_ascii_case("kde"));
        let theme_names: &[&str] = if kde {
            &["breeze", "breeze-dark", "hicolor", "Adwaita"]
        } else {
            &["Adwaita", "hicolor", "breeze", "breeze-dark"]
        };
        // Honour an explicit theme if the user set one via the conventional
        // environment variable, but keep the standard themes as fallbacks.
        let explicit = env::var("XDG_ICON_THEME").ok();
        let mut themes: Vec<String> = Vec::new();
        if let Some(name) = explicit {
            if !name.is_empty() {
                themes.push(name);
            }
        }
        for name in theme_names {
            if !themes.iter().any(|t| t == name) {
                themes.push((*name).to_string());
            }
        }

        Self {
            cache: HashMap::new(),
            roots,
            themes,
            prefer_dark: false,
        }
    }

    /// Prefer the dark sibling of a theme (`breeze-dark` before `breeze`) when
    /// the effective application theme is dark. Breeze-dark's
    /// `index.theme` inherits from `breeze`, so it still provides every icon
    /// the light theme has while returning the dark artwork.
    ///
    /// Reordering invalidates the lookup cache; calling this repeatedly with
    /// the same value is a no-op, so every theme-change notification can call
    /// it unconditionally.
    pub fn set_prefer_dark(&mut self, prefer_dark: bool) {
        if self.prefer_dark == prefer_dark {
            return;
        }
        self.prefer_dark = prefer_dark;
        reorder_breeze_themes(&mut self.themes, prefer_dark);
        self.cache.clear();
    }

    /// Resolve the system icon for a directory entry.
    ///
    /// Directories map to `inode-directory`, symlinks to `inode-symlink` (with
    /// a Breeze `emblem-symbolic-link` fallback); regular files first consult
    /// the extension table and then fall back to the icon for their
    /// [`FileKind`]. Returns `None` when no installed theme provides the icon.
    pub fn icon_path_for_entry(&mut self, entry: &Entry, size: u32) -> Option<PathBuf> {
        let kind = if entry.is_dir {
            FileKind::Folder
        } else if entry.is_symlink {
            FileKind::Symlink
        } else {
            kind_for_name(&entry.name_text)
        };
        let name = icon_name_for_kind(kind, &entry.name_text);
        self.icon_path_for_name(name, size)
    }

    /// Resolve an icon by its freedesktop icon name, e.g. `"text-rust"` or
    /// `"inode-directory"`. Results (including misses) are cached by
    /// `(name, size)`, so repeated calls for a file list are cheap.
    pub fn icon_path_for_name(&mut self, name: &str, size: u32) -> Option<PathBuf> {
        let size = size.max(1);
        let key = (name.to_string(), size);
        if let Some(cached) = self.cache.get(&key) {
            return cached.clone();
        }

        let variants = icon_name_variants(name);
        let size_dirs = size_dir_candidates(size);
        let mut found = None;

        'search: for root in &self.roots {
            for theme in &self.themes {
                let theme_dir = root.join(theme);
                if !theme_dir.is_dir() {
                    continue;
                }
                for context in CONTEXTS {
                    let context_dir = theme_dir.join(context);
                    if !context_dir.is_dir() {
                        continue;
                    }
                    for variant in &variants {
                        for size_dir in &size_dirs {
                            let dir = context_dir.join(size_dir);
                            if !dir.is_dir() {
                                continue;
                            }
                            for extension in EXTENSIONS {
                                let candidate = dir.join(format!("{variant}.{extension}"));
                                if candidate.is_file() {
                                    found = Some(candidate);
                                    break 'search;
                                }
                            }
                        }
                    }
                }
            }
        }

        self.cache.insert(key, found.clone());
        found
    }

    /// Configured theme search roots in priority order (user overrides first,
    /// then `/usr/share/icons`). Useful for diagnostics and host detection.
    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }
}

/// Pure mapping from a coarse [`FileKind`] plus a file name to the primary
/// freedesktop icon name to try. Extension matches win; otherwise the kind's
/// generic icon is used. No filesystem access, directly unit-testable.
pub fn icon_name_for_kind(kind: FileKind, name: &str) -> &'static str {
    match kind {
        FileKind::Folder => "inode-directory",
        FileKind::Symlink => "inode-symlink",
        _ => match extension_icon_names(name) {
            Some(names) => names[0],
            None => generic_icon_for_kind(kind),
        },
    }
}

fn generic_icon_for_kind(kind: FileKind) -> &'static str {
    match kind {
        FileKind::Image => "image-x-generic",
        FileKind::Code => "text-x-generic",
        FileKind::Archive => "application-x-archive",
        FileKind::Pdf => "application-pdf",
        FileKind::Text => "text-x-generic",
        FileKind::Audio => "audio-x-generic",
        FileKind::Video => "video-x-generic",
        FileKind::Unknown | FileKind::Folder | FileKind::Symlink => "text-x-generic",
    }
}

/// Return the extension-specific candidate names for a file name, if any.
fn extension_icon_names(name: &str) -> Option<&'static [&'static str]> {
    let (_, extension) = name.rsplit_once('.')?;
    let extension = extension.to_ascii_lowercase();
    EXTENSION_ICONS
        .iter()
        .find(|(ext, _)| *ext == extension.as_str())
        .map(|(_, names)| *names)
}

/// Build the alternative names to try for a given primary icon name.
fn icon_name_variants(name: &str) -> Vec<String> {
    let mut variants = vec![name.to_string()];
    let mut push = |candidate: &str| {
        if !variants.iter().any(|v| v == candidate) {
            variants.push(candidate.to_string());
        }
    };

    match name {
        "text-rust" => push("text-x-rust"),
        "text-x-rust" => push("text-rust"),
        // Breeze has no `inode-symlink`; its link emblem is the closest match.
        "inode-symlink" => push("emblem-symbolic-link"),
        _ => {}
    }

    // Freedesktop historically prefixes many names with `-x-`; try that form
    // as a last-resort variant within the same theme.
    if let Some(rest) = name.strip_prefix("text-") {
        if !rest.starts_with("x-") {
            push(&format!("text-x-{rest}"));
        }
    }
    if let Some(rest) = name.strip_prefix("image-") {
        if !rest.starts_with("x-") {
            push(&format!("image-x-{rest}"));
        }
    }
    variants
}

/// Ordered list of size directory names to try for a requested pixel size:
/// exact forms first, then the closest larger sizes, then smaller ones, then
/// theme-provided `scalable` vectors. Both Breeze (`16`, `16@2x`) and
/// hicolor/Adwaita (`16x16`, `scalable`) conventions are covered.
fn size_dir_candidates(size: u32) -> Vec<String> {
    let size = size.max(1);
    let mut dirs = Vec::new();

    dirs.push(format!("{size}"));
    dirs.push(format!("{size}x{size}"));
    dirs.push(format!("{size}@2x"));
    dirs.push(format!("{size}@3x"));
    dirs.push(format!("{size}x{size}@2"));

    let mut larger: Vec<u32> = SIZE_LADDER
        .iter()
        .copied()
        .filter(|candidate| *candidate > size)
        .collect();
    larger.sort_unstable();
    for candidate in larger {
        dirs.push(format!("{candidate}"));
        dirs.push(format!("{candidate}x{candidate}"));
    }

    let mut smaller: Vec<u32> = SIZE_LADDER
        .iter()
        .copied()
        .filter(|candidate| *candidate < size)
        .collect();
    smaller.sort_unstable_by(|a, b| b.cmp(a));
    for candidate in smaller {
        dirs.push(format!("{candidate}"));
        dirs.push(format!("{candidate}x{candidate}"));
    }

    dirs.push("scalable".to_string());
    dirs
}

fn push_unique_path(paths: &mut Vec<PathBuf>, candidate: PathBuf) {
    if !paths.iter().any(|path| path == &candidate) {
        paths.push(candidate);
    }
}

/// Swap the `breeze` / `breeze-dark` positions so the requested variant is
/// searched first. Swapping (instead of moving to the front) preserves any
/// explicit `XDG_ICON_THEME` entry and the rest of the fallback ladder.
fn reorder_breeze_themes(themes: &mut [String], prefer_dark: bool) {
    let (Some(light), Some(dark)) = (
        themes.iter().position(|theme| theme == "breeze"),
        themes.iter().position(|theme| theme == "breeze-dark"),
    ) else {
        return;
    };
    if (dark < light) != prefer_dark {
        themes.swap(light, dark);
    }
}

#[cfg(test)]
mod tests {
    use super::{icon_name_for_kind, icon_name_variants, size_dir_candidates, SystemIconResolver};
    use crate::icons::{kind_for_name, FileKind};

    #[test]
    fn icon_name_for_kind_maps_known_extensions() {
        assert_eq!(icon_name_for_kind(FileKind::Code, "main.rs"), "text-rust");
        assert_eq!(
            icon_name_for_kind(FileKind::Pdf, "paper.pdf"),
            "application-pdf"
        );
        assert_eq!(
            icon_name_for_kind(FileKind::Image, "photo.PNG"),
            "image-png"
        );
        assert_eq!(
            icon_name_for_kind(FileKind::Archive, "bundle.zip"),
            "application-zip"
        );
        assert_eq!(icon_name_for_kind(FileKind::Video, "clip.mp4"), "video-mp4");
    }

    #[test]
    fn icon_name_for_kind_prefers_directory_and_symlink_states() {
        assert_eq!(
            icon_name_for_kind(FileKind::Folder, "anything.rs"),
            "inode-directory"
        );
        assert_eq!(
            icon_name_for_kind(FileKind::Symlink, "anything.rs"),
            "inode-symlink"
        );
    }

    #[test]
    fn icon_name_for_kind_falls_back_to_kind_generic() {
        // No table entry for `.xyz`: fall back to the coarse classification.
        assert_eq!(
            icon_name_for_kind(FileKind::Image, "picture.xyz"),
            "image-x-generic"
        );
        assert_eq!(
            icon_name_for_kind(FileKind::Code, "module.xyz"),
            "text-x-generic"
        );
        assert_eq!(
            icon_name_for_kind(FileKind::Unknown, "no-extension"),
            "text-x-generic"
        );
        // The helper is independent of `kind_for_name` but accepts its result.
        let kind = kind_for_name("archive.weird-ext");
        assert_eq!(kind, FileKind::Unknown);
        assert_eq!(
            icon_name_for_kind(kind, "archive.weird-ext"),
            "text-x-generic"
        );
    }

    #[test]
    fn name_variants_cover_rust_and_symlink() {
        let rust = icon_name_variants("text-rust");
        assert_eq!(rust.first().map(String::as_str), Some("text-rust"));
        assert!(rust.iter().any(|v| v == "text-x-rust"));

        let link = icon_name_variants("inode-symlink");
        assert!(link.iter().any(|v| v == "emblem-symbolic-link"));

        let zip = icon_name_variants("application-zip");
        assert_eq!(zip.first().map(String::as_str), Some("application-zip"));
    }

    #[test]
    fn name_variants_cover_reverse_rust_alias() {
        let variants = icon_name_variants("text-x-rust");
        assert_eq!(variants.first().map(String::as_str), Some("text-x-rust"));
        assert!(
            variants.iter().any(|v| v == "text-rust"),
            "the x-prefixed Rust name must fall back to text-rust: {variants:?}"
        );
    }

    #[test]
    fn size_candidates_prefer_exact_then_larger_then_smaller() {
        let dirs = size_dir_candidates(32);
        assert_eq!(dirs.first().map(String::as_str), Some("32"));
        assert!(dirs.iter().any(|d| d == "32x32"));
        let larger_pos = dirs.iter().position(|d| d == "48").unwrap();
        let smaller_pos = dirs.iter().position(|d| d == "24").unwrap();
        assert!(larger_pos < smaller_pos, "larger sizes must be tried first");
        assert_eq!(dirs.last().map(String::as_str), Some("scalable"));
    }

    #[test]
    fn zero_and_one_sizes_produce_identical_size_dirs() {
        // Both arguments go through `size.max(1)`, so they must expand to the
        // exact same preference ladder.
        assert_eq!(size_dir_candidates(0), size_dir_candidates(1));
    }

    #[test]
    fn set_prefer_dark_reorders_themes_and_invalidates_cache() {
        let mut resolver = SystemIconResolver::new();
        // Fixed ladder: the host's XDG_* values must not affect this test.
        resolver.themes = vec![
            "breeze".to_string(),
            "breeze-dark".to_string(),
            "hicolor".to_string(),
        ];
        let light_order = resolver.themes.clone();

        resolver.cache.insert(("icon".to_string(), 16), None);
        resolver.set_prefer_dark(true);
        assert_eq!(
            resolver.themes,
            vec!["breeze-dark", "breeze", "hicolor"],
            "the dark theme must be searched before the light one"
        );
        assert!(
            resolver.cache.is_empty(),
            "cached paths must be invalidated"
        );

        // A repeated call with the same value must leave a fresh cache alone.
        resolver.cache.insert(("icon".to_string(), 16), None);
        resolver.set_prefer_dark(true);
        assert_eq!(resolver.cache.len(), 1, "unchanged preference is a no-op");

        resolver.set_prefer_dark(false);
        assert_eq!(resolver.themes, light_order, "light order must be restored");
        assert!(resolver.cache.is_empty());

        // An explicit theme and unrelated fallbacks keep their positions.
        resolver.themes = vec![
            "custom".to_string(),
            "hicolor".to_string(),
            "breeze".to_string(),
            "breeze-dark".to_string(),
        ];
        resolver.set_prefer_dark(true);
        assert_eq!(
            resolver.themes,
            vec!["custom", "hicolor", "breeze-dark", "breeze"]
        );
    }

    #[test]
    fn set_prefer_dark_same_value_false_is_a_no_op() {
        let mut resolver = SystemIconResolver::new();
        resolver.themes = vec![
            "breeze".to_string(),
            "breeze-dark".to_string(),
            "hicolor".to_string(),
        ];
        let original = resolver.themes.clone();
        resolver.cache.insert(("icon".to_string(), 16), None);

        // A fresh resolver starts at false. Repeating false must hit the
        // early return: no reorder, no flag change, no cache invalidation.
        assert!(!resolver.prefer_dark);
        resolver.set_prefer_dark(false);
        assert_eq!(resolver.themes, original, "theme order must not change");
        assert!(!resolver.prefer_dark);
        assert_eq!(resolver.cache.len(), 1, "cache must survive a no-op");
    }

    #[test]
    fn set_prefer_dark_without_breeze_themes_is_safe() {
        let mut resolver = SystemIconResolver::new();
        resolver.themes = vec![
            "Adwaita".to_string(),
            "hicolor".to_string(),
            "custom".to_string(),
        ];
        let original = resolver.themes.clone();
        resolver.cache.insert(("icon".to_string(), 16), None);

        // `reorder_breeze_themes` returns early when either sibling is absent.
        // The flag and cache still update, but the ladder must not move or
        // panic (the old code indexed into the vector unconditionally).
        resolver.set_prefer_dark(true);
        assert_eq!(resolver.themes, original, "theme order must be untouched");
        assert!(resolver.prefer_dark);
        assert!(
            resolver.cache.is_empty(),
            "a real preference change still clears the cache"
        );
    }

    #[test]
    fn set_prefer_dark_already_dark_order_is_not_swapped_twice() {
        let mut resolver = SystemIconResolver::new();
        resolver.themes = vec![
            "breeze-dark".to_string(),
            "breeze".to_string(),
            "hicolor".to_string(),
        ];
        let dark_order = resolver.themes.clone();

        // Dark already outranks light: the swap branch must be skipped, so a
        // repeated "prefer dark" call cannot flip the ladder back to light.
        resolver.set_prefer_dark(true);
        assert_eq!(resolver.themes, dark_order, "no double swap");
        assert!(resolver.prefer_dark);

        // Flipping to light swaps exactly once; re-enabling dark restores the
        // original order without an additional swap.
        resolver.set_prefer_dark(false);
        assert_eq!(resolver.themes, vec!["breeze", "breeze-dark", "hicolor"]);
        resolver.set_prefer_dark(true);
        assert_eq!(resolver.themes, dark_order, "order must be restored");
    }

    #[test]
    fn resolver_caches_misses_by_name_and_size() {
        let mut resolver = SystemIconResolver::new();
        let missing = "definitely-not-a-real-icon-name-gpui-test";
        assert!(resolver.icon_path_for_name(missing, 16).is_none());
        assert_eq!(resolver.cache.len(), 1);
        assert!(resolver.icon_path_for_name(missing, 16).is_none());
        assert_eq!(resolver.cache.len(), 1, "repeat lookup must hit the cache");
        assert!(resolver.icon_path_for_name(missing, 24).is_none());
        assert_eq!(resolver.cache.len(), 2, "size is part of the cache key");
    }

    #[test]
    fn zero_size_shares_cache_key_with_one() {
        use std::path::PathBuf;

        let mut resolver = SystemIconResolver::new();
        let name = "text-rust";
        let sentinel = PathBuf::from("/nonexistent-sentinel/icon.svg");
        // `icon_path_for_name` normalizes the size with `max(1)` before
        // building the `(name, size)` key, so 0 and 1 must be interchangeable.
        resolver
            .cache
            .insert((name.to_string(), 1), Some(sentinel.clone()));
        assert_eq!(resolver.icon_path_for_name(name, 0), Some(sentinel.clone()));
        assert_eq!(resolver.icon_path_for_name(name, 1), Some(sentinel));
        assert_eq!(
            resolver.cache.len(),
            1,
            "sizes 0 and 1 must resolve to the same cache key"
        );
    }

    #[test]
    fn inode_directory_resolves_on_this_host() {
        let mut resolver = SystemIconResolver::new();
        if resolver.roots().is_empty() {
            eprintln!("skip: no icon theme roots on this host");
            return;
        }
        let Some(path) = resolver.icon_path_for_name("inode-directory", 16) else {
            // A machine with no freedesktop icon theme installed is valid;
            // skip rather than fail the suite there.
            eprintln!("skip: inode-directory icon not found in any theme");
            return;
        };
        assert!(
            path.is_file(),
            "resolved icon must exist: {}",
            path.display()
        );
        assert!(
            path.to_string_lossy().contains("inode-directory"),
            "unexpected icon path: {}",
            path.display()
        );
    }

    #[test]
    fn common_file_types_resolve_on_this_host() {
        let mut resolver = SystemIconResolver::new();
        if resolver.roots().is_empty() {
            eprintln!("skip: no icon theme roots on this host");
            return;
        }
        if resolver.icon_path_for_name("inode-directory", 16).is_none() {
            eprintln!("skip: no usable icon theme on this host");
            return;
        }
        for name in [
            "text-rust",
            "application-pdf",
            "image-png",
            "application-zip",
        ] {
            let path = resolver.icon_path_for_name(name, 16);
            assert!(
                path.as_ref().is_some_and(|p| p.is_file()),
                "expected {name} to resolve, got {path:?}"
            );
        }
    }
}
