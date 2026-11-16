// SPDX-License-Identifier: AGPL-3.0-or-later
//! Volume/mount discovery for the sidebar.
//!
//! Linux-first and dependency-free: `/proc/mounts` is parsed, pseudo
//! filesystems and system mounts are filtered out, and duplicate mount points
//! are collapsed. Other platforms return an empty list for now (documented in
//! the README); they can be added later without changing the UI.
//!
//! This module is GPUI-free so the parser can be unit tested headlessly.

use std::path::PathBuf;

/// One mount point shown in the sidebar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MountPoint {
    /// Device/source as reported by the kernel (e.g. `/dev/nvme0n1p7`).
    pub source: String,
    /// Directory where the filesystem is mounted.
    pub path: PathBuf,
    /// Filesystem type (e.g. `ext4`, `vfat`).
    pub fs_type: String,
}

/// Filesystem types that are useful in a file-manager sidebar. Pseudo
/// filesystems (`proc`, `sysfs`, `tmpfs`, `overlay`, `squashfs`, ...) are
/// deliberately excluded.
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

fn fs_is_visible(fs_type: &str) -> bool {
    ALLOWED_FS.contains(&fs_type)
}

/// System mount points that would only add noise to the sidebar.
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
fn unescape_mount_field(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(value.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\' && index + 3 < bytes.len() {
            let octal = &value[index + 1..index + 4];
            if octal.bytes().all(|byte| (b'0'..=b'7').contains(&byte)) {
                if let Ok(byte) = u8::from_str_radix(octal, 8) {
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

/// List the visible mounts on the current system. Blocking: call from the
/// background executor or during startup/reload (not from render).
#[cfg(target_os = "linux")]
pub fn list_mounts() -> Vec<MountPoint> {
    match std::fs::read_to_string("/proc/mounts") {
        Ok(text) => parse_mounts(&text),
        Err(_) => Vec::new(),
    }
}

/// Non-Linux platforms: no volume discovery yet (the sidebar simply omits the
/// "ドライブ" section).
#[cfg(not(target_os = "linux"))]
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
"#;

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
    fn unescapes_octal_fields() {
        assert_eq!(
            unescape_mount_field(r"/media/USB\040Drive"),
            "/media/USB Drive"
        );
        assert_eq!(unescape_mount_field(r"a\011b"), "a\tb");
        assert_eq!(unescape_mount_field(r"a\134b"), r"a\b");
        assert_eq!(unescape_mount_field("plain"), "plain");
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
}
