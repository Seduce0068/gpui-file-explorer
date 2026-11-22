// SPDX-License-Identifier: AGPL-3.0-or-later
//! Volume/mount discovery for the sidebar.
//!
//! Linux-first and dependency-free: `/proc/mounts` is parsed, pseudo
//! filesystems and system mounts are filtered out, and duplicate mount points
//! are collapsed. Other platforms return an empty list for now (documented in
//! the README); they can be added later without changing the UI.
//!
//! Windows drive discovery is split in two: the projection from the Win32
//! `GetLogicalDrives` mask / `GetVolumeInformationW` data into `MountPoint`s is
//! a pure, headlessly testable function, while the actual FFI call is kept in a
//! documented stub. The stub is intentionally empty because `windows-sys` is
//! only a *transitive* dependency (it cannot be `use`d directly) and this crate
//! sets `#![forbid(unsafe_code)]`; see the note on [`list_mounts`] below.
//!
//! This module is GPUI-free so the parser can be unit tested headlessly.

use std::path::PathBuf;

/// One mount point shown in the sidebar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MountPoint {
    /// Device/source as reported by the kernel (e.g. `/dev/nvme0n1p7`).
    /// On Windows this holds `C:` plus the volume label, e.g. `C: (System)`.
    pub source: String,
    /// Directory where the filesystem is mounted.
    pub path: PathBuf,
    /// Filesystem type (e.g. `ext4`, `vfat`).
    pub fs_type: String,
}

/// Pure: expand a Win32 `GetLogicalDrives` bitmask into drive letters `A`..`Z`.
///
/// Bit *i* (0-based) corresponds to `b'A' + i`; the remaining bits of the
/// `u32` are ignored. Keeping this conversion pure makes the Windows drive
/// layout regression-testable on Linux as well.
#[cfg(any(target_os = "windows", test))]
pub fn drive_letters_from_mask(mask: u32) -> Vec<char> {
    (0..26u32)
        .filter_map(|index| (mask & (1u32 << index) != 0).then_some(char::from(b'A' + index as u8)))
        .collect()
}

/// Pure: convert a drive mask and a per-letter volume-info lookup into sidebar
/// entries. `volume_for` receives the drive letter and returns
/// `(volume label, filesystem name)` as produced by `GetVolumeInformationW`.
///
/// The volume label is folded into `source` because the sidebar tooltip already
/// renders `path · fs_type · source`; this keeps the platform-neutral
/// `MountPoint` shape unchanged for the Linux parser.
#[cfg(any(target_os = "windows", test))]
pub fn mount_points_from_drive_mask(
    mask: u32,
    mut volume_for: impl FnMut(char) -> (String, String),
) -> Vec<MountPoint> {
    drive_letters_from_mask(mask)
        .into_iter()
        .map(|letter| {
            let (label, fs_type) = volume_for(letter);
            let source = if label.is_empty() {
                format!("{}:", letter)
            } else {
                format!("{}: ({label})", letter)
            };
            MountPoint {
                source,
                path: PathBuf::from(format!("{}:\\", letter)),
                fs_type,
            }
        })
        .collect()
}

/// Filesystem types that are useful in a file-manager sidebar. Pseudo
/// filesystems (`proc`, `sysfs`, `tmpfs`, `overlay`, `squashfs`, ...) are
/// deliberately excluded.
#[cfg(any(target_os = "linux", test))]
const ALLOWED_FS: &[&str] = &[
    "ext2",
    "ext3",
    "ext4",
    "btrfs",
    "xfs",
    "f2fs",
    "bcachefs",
    "zfs",
    "vfat",
    "exfat",
    "ntfs",
    "ntfs3",
    "fuseblk",
    "nfs",
    "nfs4",
    "cifs",
    "smb3",
    "9p",
    "fuse.sshfs",
    "fuse.rclone",
    "fuse.smbnetfs",
];

#[cfg(any(target_os = "linux", test))]
fn fs_is_visible(fs_type: &str) -> bool {
    ALLOWED_FS.contains(&fs_type)
}

/// System mount points that would only add noise to the sidebar.
#[cfg(any(target_os = "linux", test))]
fn path_is_visible(path: &str) -> bool {
    if path == "/" {
        return true;
    }
    const HIDDEN_PREFIXES: &[&str] = &[
        "/proc",
        "/sys",
        "/dev",
        "/snap",
        "/run/credentials",
        "/var/lib/docker",
        "/var/lib/containers",
    ];
    !HIDDEN_PREFIXES
        .iter()
        .any(|prefix| path == *prefix || path.starts_with(&format!("{prefix}/")))
}

/// Decode the octal escapes used by `/proc/mounts` (`\040` = space,
/// `\011` = tab, `\012` = newline, `\134` = backslash).
#[cfg(any(target_os = "linux", test))]
fn unescape_mount_field(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(value.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\' && index + 3 < bytes.len() {
            // Slice the byte buffer, not `&str`: the next three bytes may end
            // inside a multi-byte UTF-8 character (e.g. `\aい`), and slicing
            // `&str` at a non-char boundary would panic.
            let octal = &bytes[index + 1..index + 4];
            if octal.iter().all(|byte| (b'0'..=b'7').contains(byte)) {
                if let Ok(byte) = u8::from_str_radix(
                    std::str::from_utf8(octal).expect("octal digits are ASCII"),
                    8,
                ) {
                    out.push(byte);
                    index += 4;
                    continue;
                }
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    // Decode once at the end so multi-byte UTF-8 paths are not mangled by
    // treating each byte as a separate char.
    String::from_utf8_lossy(&out).into_owned()
}

/// Parse `/proc/mounts` content into visible, de-duplicated, sorted mounts.
#[cfg(any(target_os = "linux", test))]
pub fn parse_mounts(text: &str) -> Vec<MountPoint> {
    let mut mounts: Vec<MountPoint> = Vec::new();
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        let Some(source) = fields.next() else {
            continue;
        };
        let Some(path) = fields.next() else {
            continue;
        };
        let Some(fs_type) = fields.next() else {
            continue;
        };
        let path = unescape_mount_field(path);
        if !path_is_visible(&path) || !fs_is_visible(fs_type) {
            continue;
        }
        let path = PathBuf::from(path);
        if mounts.iter().any(|mount| mount.path == path) {
            continue;
        }
        mounts.push(MountPoint {
            source: unescape_mount_field(source),
            path,
            fs_type: fs_type.to_string(),
        });
    }
    mounts.sort_by(|left, right| left.path.cmp(&right.path));
    mounts
}

/// Read a mounts-style file into a string.
///
/// Blocking: this touches the filesystem on the calling thread. UI code must
/// offload it (e.g. `cx.background_spawn(async { mounts::list_mounts() })`).
/// The path argument lets tests exercise the exact read-then-parse path used
/// by [`list_mounts`] without depending on the host's mount table.
#[cfg(target_os = "linux")]
fn read_mounts_file(path: impl AsRef<std::path::Path>) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// List the visible mounts on the current system. Blocking: call from the
/// background executor only — `/proc/mounts` is read on the calling thread.
#[cfg(target_os = "linux")]
pub fn list_mounts() -> Vec<MountPoint> {
    parse_mounts(&read_mounts_file("/proc/mounts"))
}

/// Windows: drive-letter discovery through the Win32 volume APIs.
///
/// STATUS — documented stub. `windows-sys` exists in `Cargo.lock` only as a
/// *transitive* dependency (0.48.0 / 0.52.0 / 0.60.2 / 0.61.2), so this crate
/// cannot `use` it without a direct dependency declaration in `Cargo.toml`.
/// `main.rs` also sets `#![forbid(unsafe_code)]`, whereas the Win32 entry points
/// are `unsafe extern "system"` functions. Once a direct dependency plus a
/// narrowly scoped, reviewed FFI module exist, replace the zero mask and empty
/// lookup below with the real calls:
///
/// ```text
/// let mask = unsafe { GetLogicalDrives() };
/// let volume_for = |letter: char| {
///     let root: Vec<u16> = format!("{}:\\\0", letter).encode_utf16().collect();
///     // unsafe GetVolumeInformationW(root.as_ptr(), ...) -> (label, fs_type)
/// };
/// mount_points_from_drive_mask(mask, volume_for)
/// ```
///
/// Signatures were verified from the locally cached `windows-sys 0.61.2`
/// source (feature `Win32_Storage_FileSystem`): `GetLogicalDrives() -> u32` and
/// `GetVolumeInformationW(PCWSTR, PWSTR, u32, *mut u32, *mut u32, *mut u32,
/// PWSTR, u32) -> BOOL`. `GetDiskFreeSpaceExW` would additionally provide the
/// total/free byte counts.
#[cfg(target_os = "windows")]
pub fn list_mounts() -> Vec<MountPoint> {
    mount_points_from_drive_mask(0, |_| (String::new(), String::new()))
}

/// Other non-Linux/non-Windows platforms: no volume discovery yet (the sidebar
/// simply omits the "ドライブ" section).
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub fn list_mounts() -> Vec<MountPoint> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
/dev/nvme0n1p7 / ext4 rw,relatime 0 0
proc /proc proc rw,nosuid,nodev,noexec,relatime 0 0
sysfs /sys sysfs rw,nosuid,nodev,noexec,relatime 0 0
tmpfs /run tmpfs rw,nosuid,nodev 0 0
/dev/nvme0n1p1 /boot/efi vfat rw,relatime,fmask=0077 0 0
/dev/sdb1 /media/koppe/USB\040Drive exfat rw,nosuid,nodev 0 0
/dev/nvme0n1p7 /mnt/timeshift ext4 rw,relatime 0 0
/snap/core24/1643 /snap/core24/1643 squashfs ro,nodev 0 0
overlay /var/lib/docker/overlay2/abc overlay rw 0 0
/dev/nvme0n1p7 /mnt/timeshift ext4 rw,relatime 0 0
/dev/sdc1 /snap/data ext4 rw,relatime 0 0
/dev/sdd1 /var/lib/docker/volumes ext4 rw,relatime 0 0
"#;
    // The last two lines use ALLOWED filesystems at hidden mount points, so
    // they pass `fs_is_visible` and can only be dropped by `path_is_visible`.
    // Without them, every hidden path in SAMPLE was also pseudo-fs and the
    // parser tests never exercised the path filter non-vacuously.

    #[test]
    fn filters_pseudo_filesystems_and_dedupes() {
        let mounts = parse_mounts(SAMPLE);
        let paths: Vec<String> = mounts
            .iter()
            .map(|mount| mount.path.display().to_string())
            .collect();
        assert_eq!(
            paths,
            vec!["/", "/boot/efi", "/media/koppe/USB Drive", "/mnt/timeshift"]
        );
        assert_eq!(mounts[0].fs_type, "ext4");
        assert_eq!(mounts[2].fs_type, "exfat");
    }

    #[test]
    fn path_is_visible_rejects_hidden_paths_even_with_allowed_filesystem() {
        // Positive control: ext4 is an ALLOWED filesystem, so the hidden paths
        // below are rejected by the path filter alone (not by fs filtering).
        assert!(fs_is_visible("ext4"));
        assert!(!path_is_visible("/snap/data"));
        assert!(!path_is_visible("/var/lib/docker/volumes"));

        // A mount table holding only allowed-fs mounts at hidden paths must
        // still parse to nothing, proving `parse_mounts` really consults the
        // path filter rather than getting the same result from `fs_is_visible`.
        let hidden_only = "/dev/sdc1 /snap/data ext4 rw 0 0\n\
                           /dev/sdd1 /var/lib/docker/volumes ext4 rw 0 0\n";
        assert!(parse_mounts(hidden_only).is_empty());
    }

    #[test]
    fn path_is_visible_uses_exact_hidden_prefixes_not_plain_string_prefixes() {
        assert!(path_is_visible("/"));
        assert!(!path_is_visible("/proc"));
        assert!(!path_is_visible("/proc/self"));
        // Boundary: "/procx" shares the prefix string but is not inside /proc.
        assert!(path_is_visible("/procx"));
        assert!(path_is_visible("/procfs/mounts"));
        // The same component-aware rule applies to the longer hidden prefix.
        assert!(!path_is_visible("/var/lib/docker"));
        assert!(path_is_visible("/var/lib/dockerized"));
    }

    #[test]
    fn unescapes_octal_fields() {
        assert_eq!(
            unescape_mount_field(r"/media/USB\040Drive"),
            "/media/USB Drive"
        );
        assert_eq!(unescape_mount_field(r"a\011b"), "a\tb");
        assert_eq!(unescape_mount_field(r"a\134b"), r"a\b");
        assert_eq!(unescape_mount_field("plain"), "plain");

        // Non-octal digits after a backslash: keep the escape verbatim.
        assert_eq!(unescape_mount_field(r"a\xyzb"), r"a\xyzb");
        // \777 = 511 does not fit in a byte: keep the escape verbatim.
        assert_eq!(unescape_mount_field(r"a\777b"), r"a\777b");
        // Fewer than three digits before the end: keep the escape verbatim.
        assert_eq!(unescape_mount_field(r"a\04"), r"a\04");
        assert_eq!(unescape_mount_field(r"a\0"), r"a\0");
        // A backslash followed by bytes that end inside a multi-byte UTF-8
        // character must not panic (the octal slice is taken from raw bytes).
        assert_eq!(unescape_mount_field("\\aい"), "\\aい");
        assert_eq!(unescape_mount_field("い\\a"), "い\\a");
    }

    #[test]
    fn unescape_preserves_utf8_and_octal_utf8() {
        assert_eq!(unescape_mount_field("/mnt/日本語"), "/mnt/日本語");
        // \303\251 is the UTF-8 byte sequence for 'é'.
        assert_eq!(unescape_mount_field(r"/mnt/caf\303\251"), "/mnt/café");
    }

    #[test]
    fn skips_malformed_lines() {
        let mounts = parse_mounts("garbage\n/dev/sda1\n/dev/sda1 /data ext4 rw 0 0\n");
        assert_eq!(mounts.len(), 1);
        assert_eq!(mounts[0].path, PathBuf::from("/data"));
    }

    #[test]
    fn non_linux_returns_empty() {
        if cfg!(not(target_os = "linux")) {
            assert!(list_mounts().is_empty());
        }
    }

    #[test]
    fn drive_letters_from_mask_maps_bits_to_letters() {
        // Bit i maps to 'A' + i; bit 26 is outside the A:..Z: range.
        assert_eq!(drive_letters_from_mask(0b0000_0101), vec!['A', 'C']);
        assert_eq!(drive_letters_from_mask(1 << 25), vec!['Z']);
        assert!(drive_letters_from_mask(1 << 26).is_empty());
        assert_eq!(drive_letters_from_mask((1 << 2) | 1), vec!['A', 'C']);
        assert!(drive_letters_from_mask(0).is_empty());
    }

    #[test]
    fn mount_points_from_drive_mask_builds_windows_roots_and_labels() {
        let mask = (1 << 0) | (1 << 2) | (1 << 25);
        let mounts = mount_points_from_drive_mask(mask, |letter| match letter {
            'C' => ("System".to_string(), "NTFS".to_string()),
            'Z' => (String::new(), "exFAT".to_string()),
            _ => (String::new(), String::new()),
        });
        assert_eq!(
            mounts
                .iter()
                .map(|mount| mount.path.display().to_string())
                .collect::<Vec<_>>(),
            vec![r"A:\", r"C:\", r"Z:\"]
        );
        assert_eq!(mounts[0].source, "A:");
        assert_eq!(mounts[1].source, "C: (System)");
        assert_eq!(mounts[1].fs_type, "NTFS");
        assert_eq!(mounts[2].source, "Z:");
        assert_eq!(mounts[2].fs_type, "exFAT");
    }

    #[test]
    fn empty_drive_mask_yields_no_mounts() {
        assert!(mount_points_from_drive_mask(0, |_| (String::new(), String::new())).is_empty());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn read_mounts_file_feeds_the_parser() {
        // The offloaded `list_mounts()` is read-then-parse; prove the two steps
        // still compose on a fixture instead of relying on the host mount table.
        use std::time::{SystemTime, UNIX_EPOCH};

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("gpui-fe-mounts-{}-{stamp}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("mounts");
        std::fs::write(&file, SAMPLE).unwrap();

        let mounts = parse_mounts(&read_mounts_file(&file));
        assert_eq!(
            mounts
                .iter()
                .map(|mount| mount.path.display().to_string())
                .collect::<Vec<_>>(),
            vec!["/", "/boot/efi", "/media/koppe/USB Drive", "/mnt/timeshift"]
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
