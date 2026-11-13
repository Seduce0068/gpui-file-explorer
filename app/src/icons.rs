// SPDX-License-Identifier: AGPL-3.0-or-later
//! Extension → icon mapping.
//!
//! The prototype does not extract OS-native file icons; it maps file kinds to
//! the bundled Lucide icon set shipped by `gpui-kit-assets`, which keeps the
//! design portable across Linux/Windows/macOS.

use gpui_kit::assets::IconName;

use crate::model::Entry;

/// Coarse file classification used for icons and (in the future) previews.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    Folder,
    Image,
    Code,
    Archive,
    Pdf,
    Text,
    Audio,
    Video,
    Unknown,
}

/// Classify a directory entry. Directories always win; symlinks that are not
/// directories get the symlink glyph.
pub fn kind_for_entry(entry: &Entry) -> FileKind {
    if entry.is_dir {
        FileKind::Folder
    } else {
        kind_for_name(&entry.name)
    }
}

/// Classify a file name by its extension (case insensitive).
pub fn kind_for_name(name: &str) -> FileKind {
    let Some((_, extension)) = name.rsplit_once('.') else {
        return FileKind::Unknown;
    };
    let extension = extension.to_ascii_lowercase();
    match extension.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" | "ico" | "tif" | "tiff"
        | "avif" | "heic" | "heif" | "raw" | "psd" | "xcf" => FileKind::Image,
        "rs" | "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "py" | "rb" | "go" | "java" | "kt"
        | "kts" | "c" | "cc" | "cpp" | "cxx" | "h" | "hpp" | "cs" | "swift" | "php" | "sh"
        | "bash" | "zsh" | "fish" | "lua" | "pl" | "r" | "scala" | "clj" | "ex" | "exs" | "erl"
        | "hs" | "ml" | "nim" | "zig" | "vue" | "svelte" | "html" | "htm" | "css" | "scss"
        | "sass" | "less" | "sql" | "toml" | "yaml" | "yml" | "json" | "json5" | "xml" | "ini"
        | "conf" | "cfg" | "cmake" | "mk" | "gradle" | "dockerfile" | "nix" | "proto"
        | "graphql" | "gql" | "sol" | "asm" | "v" | "sv" | "vhd" => FileKind::Code,
        "zip" | "tar" | "gz" | "tgz" | "xz" | "txz" | "bz2" | "tbz2" | "7z" | "rar" | "zst"
        | "lz4" | "lzma" | "deb" | "rpm" | "appimage" | "jar" | "war" | "iso" | "dmg" => {
            FileKind::Archive
        }
        "pdf" => FileKind::Pdf,
        "txt" | "md" | "markdown" | "rst" | "log" | "csv" | "tsv" | "tex" | "rtf" | "org"
        | "adoc" | "nfo" | "diff" | "patch" | "license" | "readme" => FileKind::Text,
        "mp3" | "flac" | "wav" | "ogg" | "oga" | "opus" | "m4a" | "aac" | "wma" | "aiff"
        | "mid" | "midi" | "mod" | "amr" => FileKind::Audio,
        "mp4" | "m4v" | "mkv" | "avi" | "mov" | "webm" | "flv" | "wmv" | "mpg" | "mpeg" | "ogv"
        | "3gp" | "m2ts" => FileKind::Video,
        // Executables and everything else get the generic file glyph.
        _ => FileKind::Unknown,
    }
}

/// Lucide icon for a file kind.
pub fn icon_for_kind(kind: FileKind) -> IconName {
    match kind {
        FileKind::Folder => IconName::Folder,
        FileKind::Image => IconName::FileImage,
        FileKind::Code => IconName::FileCode,
        FileKind::Archive => IconName::FileArchive,
        FileKind::Pdf => IconName::FileType,
        FileKind::Text => IconName::FileText,
        FileKind::Audio => IconName::FileMusic,
        FileKind::Video => IconName::FileVideoCamera,
        FileKind::Unknown => IconName::File,
    }
}

/// Lucide icon for an entry (directories, symlinks, then extension mapping).
pub fn icon_for_entry(entry: &Entry) -> IconName {
    if entry.is_dir {
        IconName::Folder
    } else if entry.is_symlink {
        IconName::FileSymlink
    } else {
        icon_for_kind(kind_for_name(&entry.name))
    }
}

#[cfg(test)]
mod tests {
    // Explicit imports only: `use gpui_kit::*` in the crate root would shadow
    // the built-in `#[test]` attribute.
    use super::{kind_for_entry, kind_for_name, FileKind};
    use crate::model::Entry;
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn entry(name: &str, is_dir: bool, is_symlink: bool) -> Entry {
        Entry {
            name: name.to_string(),
            path: PathBuf::from("/tmp").join(name),
            is_dir,
            is_symlink,
            size: 0,
            modified: Some(SystemTime::UNIX_EPOCH),
        }
    }

    #[test]
    fn directories_are_folders() {
        assert_eq!(
            kind_for_entry(&entry("Photos", true, false)),
            FileKind::Folder
        );
        assert_eq!(
            kind_for_entry(&entry("link-to-dir", true, true)),
            FileKind::Folder
        );
    }

    #[test]
    fn known_extensions_map_to_kinds() {
        assert_eq!(kind_for_name("photo.JPG"), FileKind::Image);
        assert_eq!(kind_for_name("main.rs"), FileKind::Code);
        assert_eq!(kind_for_name("archive.tar.gz"), FileKind::Archive);
        assert_eq!(kind_for_name("paper.pdf"), FileKind::Pdf);
        assert_eq!(kind_for_name("notes.md"), FileKind::Text);
        assert_eq!(kind_for_name("song.flac"), FileKind::Audio);
        assert_eq!(kind_for_name("clip.mkv"), FileKind::Video);
        assert_eq!(kind_for_name("weird.xyz"), FileKind::Unknown);
        assert_eq!(kind_for_name("Makefile"), FileKind::Unknown);
        assert_eq!(kind_for_name("no-dot"), FileKind::Unknown);
    }

    #[test]
    fn dotfiles_keep_their_extension_mapping() {
        // ".bashrc" has no extension after the leading dot: the whole name is
        // the "extension" part of rsplit_once? No: rsplit_once('.') on
        // ".bashrc" gives ("", "bashrc") -> treated as unknown extension.
        assert_eq!(kind_for_name(".bashrc"), FileKind::Unknown);
        assert_eq!(kind_for_name(".hidden.png"), FileKind::Image);
    }
}
