// SPDX-License-Identifier: AGPL-3.0-or-later
//! # gfe-win32 — Win32 FFI を単一クレートへ隔離した境界層
//!
//! ## 理由
//!
//! `app` / `core` は `#![forbid(unsafe_code)]` を維持したまま、Windows の実
//! ドライブ列挙とシェルアイコン取得を必要としていた。Win32 API は
//! `unsafe extern "system"` であり、`app` 内で直接呼べない。そこで呼び出しを
//! 本クレートに閉じ込め、`windows-sys` への直接依存もここだけに置く（ADR 推奨
//! A2: unsafe は別 crate へ隔離し、アプリ側の forbid は維持）。
//!
//! ## 影響範囲
//!
//! * Windows: [`logical_drives`] が `GetLogicalDrives` /
//!   `GetVolumeInformationW` を、[`file_icon_index`] / [`file_icon_handle`] が
//!   `SHGetFileInfoW` を呼び出す。いずれも副作用は読み取りと一時的な COM
//!   アパートメントの確立／解除に限られる。
//! * Windows 以外: 同じ公開 API が空ベクタ / `None` を返す。FFI も `windows-sys`
//!   依存も有効にならないため、`app` の cross-compile がそのまま通る。
//! * `app` / `core` の `#![forbid(unsafe_code)]` は変更しない。本クレートから
//!   安全な関数のみが公開され、生の Win32 ハンドルは RAII ラッパー
//!   [`ScopedIcon`] の内部に隠される。
//!
//! ## 安全性の根拠（不変条件 / ADR:150-151）
//!
//! 1. **UTF-16 の NUL 終端とバッファ長** — [`wide_with_nul`] が先頭 NUL 以降を
//!    切り詰めたうえで末尾にちょうど 1 つ NUL を付与する。Win32 の W 系 APIへ
//!    渡す文字列は常にこの helper（パスは `wide_path_with_nul`）を通す。
//!    受け取りバッファは固定長配列で、
//!    長さ引数には要素数（WCHAR 単位・終端 NUL を含む）をそのまま渡すため
//!    容量超過書き込みは起こらない。読み出しは先頭 NUL までに限定する。
//! 2. **HICON の RAII 解放** — `SHGFI_ICON` が返した HICON は生成元所有権を
//!    [ScopedIcon] へ即座に移し、[`DestroyIcon`] を `Drop` で 1 回だけ呼ぶ。
//!    [ScopedIcon] は `PhantomData<*mut ()>` により `!Send + !Sync` で、生成
//!    スレッドを越えて運ばれず、二重解放・別スレッド解放が型レベルで不可能。
//! 3. **COM 初期化と STA** — シェル API の呼び出し前に [ComSta] が
//!    `CoInitializeEx(COINIT_APARTMENTTHREADED)` を行い、成功した回数だけ
//!    `Drop` で `CoUninitialize` を呼ぶ。既存 MTA
//!    (`RPC_E_CHANGED_MODE`) の場合は解除せず操作を断るため、他人の
//!    アパートメントを壊さない。[ComSta] も `!Send + !Sync` である。
//! 4. **HWND の有効性とスレッド親和性** — 本クレートは HWND を一切受け取らず、
//!    生成も保持もしない。UI スレッドから呼ばれても、
//!    `SHGetFileInfoW` にはパスのみを渡し、COM STA と HICON の生存期間は
//!    呼び出しスレッド上で完結する。したがって外部 HWND の有効性検証は不要。
//!
//! すべての `unsafe` ブロック直前に `// SAFETY:` コメントを置き、
//! `clippy::undocumented_unsafe_blocks` を `-D` でゲートする。
//!
//! ### なぜ ICO / PNG バイト列を返さないか
//!
//! HICON → ICO 変換には `GetIconInfo` + `GetDIBits` で DIB を取り出し、行順序
//! （bottom-up）と AND マスクのストライド境界を整え、`ICONDIR` /
//! `ICONDIRENTRY` を組み立てる処理が必要になる。PNG 化にはさらに画像
//! エンコーダ依存が加わる。いずれも実行時検証が Windows 実機でしかできない
//! unsafe を増やすため、ここではタスク指定のフォールバックである
//! 「システムイメージリストのインデックス返却」に留める。実描画側が
//! ImageList を保持する段階になったら、バイト列化を別タスクで検討する。

#![deny(unsafe_op_in_unsafe_fn)]

use std::marker::PhantomData;
use std::path::Path;

#[cfg(windows)]
use windows_sys::Win32::Foundation::RPC_E_CHANGED_MODE;
#[cfg(windows)]
use windows_sys::Win32::Storage::FileSystem::{
    GetLogicalDrives, GetVolumeInformationW, FILE_ATTRIBUTE_NORMAL,
};
#[cfg(windows)]
use windows_sys::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
#[cfg(windows)]
use windows_sys::Win32::UI::Shell::{
    SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_SMALLICON, SHGFI_SYSICONINDEX,
    SHGFI_USEFILEATTRIBUTES,
};
#[cfg(windows)]
use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, HICON};

/// サイドバー 1 行分の、所有権付き単純値。
///
/// `app::mounts::MountPoint` とフィールド名・意味が一致するため、変換は
/// `fs_type` 等を移し替えるだけ（[`Self::source`] は `C: (System)` 形式、
/// [`Self::path`] は `C:\` 形式）で済む。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MountInfo {
    /// ドライブ文字 + ボリュームラベル（例: `C: (System)`、ラベル無しは `C:`）。
    pub source: String,
    /// ドライブルート（例: `C:\`）。
    pub path: String,
    /// `GetVolumeInformationW` が返したファイルシステム名（例: `NTFS`）。
    pub fs_type: String,
}

/// `text` を「内部に NUL を持たず、末尾にちょうど 1 つの NUL を持つ」UTF-16
/// ベクタへ変換する（safe / pure）。先頭 NUL 以降は Win32 から不可視なので
/// 切り詰め、PATH の意味を勝手に変えないようにする。
// Windows の実 API 呼び出しと、非 Windows での单元测试の両方から使われるため、
// 片方だけをコンパイルするターゲットでは dead_code に見える。
#[allow(dead_code)]
fn wide_with_nul(text: &str) -> Vec<u16> {
    let mut wide: Vec<u16> = text.encode_utf16().take_while(|unit| *unit != 0).collect();
    wide.push(0);
    wide
}

/// Windows の `Path` を UTF-16 へ変換し、先頭 NUL 以降を切り詰めたうえで末尾に
/// ちょうど 1 つ NUL を付与する（safe）。`to_string_lossy` と違い、対にならない
/// サロゲートを含む実在の Windows パスもビット単位でそのまま渡せる。
#[cfg(windows)]
fn wide_path_with_nul(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    let mut wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .take_while(|unit| *unit != 0)
        .collect();
    wide.push(0);
    wide
}

/// NUL で終わる（終端が無ければ末尾まで）UTF-16 スライスを lossy にデコードする。
#[allow(dead_code)]
fn string_from_wide(wide: &[u16]) -> String {
    let end = wide
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(wide.len());
    String::from_utf16_lossy(&wide[..end])
}

/// ドライブ文字・ラベル・FS 名から [`MountInfo`] を組み立てる（pure）。
///
/// `app::mounts::mount_points_from_drive_mask` の変換規則と同じ出力を保証する。
#[allow(dead_code)]
fn mount_info_from_volume(letter: char, label: &str, fs_type: &str) -> MountInfo {
    let source = if label.is_empty() {
        format!("{letter}:")
    } else {
        format!("{letter}: ({label})")
    };
    MountInfo {
        source,
        path: format!("{letter}:\\"),
        fs_type: fs_type.to_string(),
    }
}

/// 論理ドライブを列挙する。ブロッキング API なので呼び出し側は background
/// executor から呼ぶこと（Linux の `list_mounts` と同じ契約）。
#[cfg(windows)]
pub fn logical_drives() -> Vec<MountInfo> {
    // SAFETY: `GetLogicalDrives` は引数を取らず、呼び出し前条件は存在しない。
    // 戻り値 0 は失敗を示すため空ベクタへ変換する。ビット 26 以降はドライブ
    // 文字ではないためループ範囲を 0..26 に固定し、シフト量も 25 以下である。
    let mask = unsafe { GetLogicalDrives() };
    if mask == 0 {
        return Vec::new();
    }

    let mut drives = Vec::with_capacity(mask.count_ones() as usize);
    for index in 0..26u32 {
        if mask & (1u32 << index) == 0 {
            continue;
        }
        let letter = char::from(b'A' + index as u8);
        let root = wide_with_nul(&format!("{letter}:\\"));
        let mut label_buf = [0u16; 261];
        let mut fs_buf = [0u16; 261];

        // SAFETY: `root` は `wide_with_nul` を通したので末尾にだけ NUL があり、
        // 内部に NUL は無い。第 3 / 第 8 引数にはスタック配列の要素数
        // （WCHAR 単位、終端 NUL の分を含む）を渡すので Windows は容量を
        // 越えて書き込まない。残りの 3 つのポインタ引数は API 上省略可能で
        // null を渡してよい。`label_buf` / `fs_buf` は呼び出し後に
        // `string_from_wide` が先頭 NUL までだけを読む。
        let ok = unsafe {
            GetVolumeInformationW(
                root.as_ptr(),
                label_buf.as_mut_ptr(),
                label_buf.len() as u32,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                fs_buf.as_mut_ptr(),
                fs_buf.len() as u32,
            )
        };

        // ボリューム情報の取得失敗は「ラベル・FS 名不明」として扱い、ドライブ
        // 自体は残す（サイドバーから消すとユーザーがアクセスできなくなる）。
        let (label, fs_type) = if ok == 0 {
            (String::new(), String::new())
        } else {
            (string_from_wide(&label_buf), string_from_wide(&fs_buf))
        };
        drives.push(mount_info_from_volume(letter, &label, &fs_type));
    }
    drives
}

/// Windows 以外: 実ドライブ列挙 API は存在しないため空ベクタを返す。
#[cfg(not(windows))]
pub fn logical_drives() -> Vec<MountInfo> {
    Vec::new()
}

/// 一時的な COM アパートメント (STA) を確立する RAII ガード。
#[cfg(windows)]
struct ComSta {
    uninitialize: bool,
    // COM アパートメントはスレッド局所。移送を型レベルで禁止する。
    _thread: PhantomData<*mut ()>,
}

#[cfg(windows)]
impl ComSta {
    fn enter() -> Option<Self> {
        // SAFETY: `CoInitializeEx` の第 1 引数は予約済みで NULL 固定。第 2 引数は
        // `COINIT_APARTMENTTHREADED` を u32 へキャストした値で、型エイリアス
        // `COINIT = i32` のビットパターンを保つ。呼び出しは現在スレッドの
        // アパートメントだけを変更し、S_OK / S_FALSE (>= 0) ならこの呼び出しが
        // 参照カウントを増やしたことを意味するため `Drop` で必ず対消する。
        // 既存 MTA (RPC_E_CHANGED_MODE) やその他の失敗では解除せず `None` を返し、
        // 呼び出し元はシェル API を実行しない。
        let hr = unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) };
        if hr == RPC_E_CHANGED_MODE || hr < 0 {
            return None;
        }
        Some(Self {
            uninitialize: true,
            _thread: PhantomData,
        })
    }
}

#[cfg(windows)]
impl Drop for ComSta {
    fn drop(&mut self) {
        if self.uninitialize {
            // SAFETY: 直前の `enter()` で成功した `CoInitializeEx` と 1:1 で対応
            // する。`ComSta` は `PhantomData<*mut ()>` で !Send / !Sync のため、
            // 初期化と解除は必ず同じスレッドで起こる。重複解除や未初期化解除は
            // `uninitialize` フラグと Drop の一回性により起こらない。
            unsafe { CoUninitialize() };
        }
    }
}

/// `SHGFI_ICON` が返した HICON の唯一の所有者。ドロップ時に必ず解放する。
pub struct ScopedIcon {
    #[cfg(windows)]
    handle: HICON,
    // GDI / USER ハンドルは作成スレッドに結びつくため移送を禁止する。
    _thread: PhantomData<*mut ()>,
}

impl ScopedIcon {
    /// 保持しているアイコンが有効かどうか。Windows 以外では常に `false`。
    pub fn is_valid(&self) -> bool {
        #[cfg(windows)]
        {
            !self.handle.is_null()
        }
        #[cfg(not(windows))]
        {
            false
        }
    }
}

impl Drop for ScopedIcon {
    fn drop(&mut self) {
        #[cfg(windows)]
        if !self.handle.is_null() {
            // SAFETY: `handle` は `SHGFI_ICON` 付きの `SHGetFileInfoW` が返した
            // 所有権付き HICON をそのまま移したもので、他に所有者はいない。
            // `ScopedIcon` は !Send / !Sync なので作成スレッドと同じスレッドで
            // 解放される。null でないことと Drop の一回性により `DestroyIcon` は
            // 同じハンドルに 1 回だけ適用される。
            unsafe { DestroyIcon(self.handle) };
        }
    }
}

/// パスに対応するシステムイメージリスト内インデックスを返す（Windows）。
///
/// `SHGFI_SYSICONINDEX` を用いるため HICON は生成されず、RAII で解放する
/// GDI リソースは存在しない。パスが実在しない場合は拡張子のみを手がかりに
/// 再問い合わせする。COM は呼び出しの間だけ STA で初期化される。
#[cfg(windows)]
pub fn file_icon_index(path: &Path) -> Option<u32> {
    let _sta = ComSta::enter()?;
    let wide = wide_path_with_nul(path);
    let mut info = SHFILEINFOW::default();
    let info_size = std::mem::size_of::<SHFILEINFOW>() as u32;

    // SAFETY: `wide` は `wide_path_with_nul` が生成した NUL 終端 UTF-16、
    // `info` は有効な `SHFILEINFOW` への
    // 可変参照、`info_size` はその実際のサイズ。第 2 引数は
    // `SHGFI_USEFILEATTRIBUTES` が無い場合は無視されるので 0 を渡す。STA は
    // `_sta` が生きている間確立済み。HWND は渡さないため有効性検証は不要。
    let mut result =
        unsafe { SHGetFileInfoW(wide.as_ptr(), 0, &mut info, info_size, SHGFI_SYSICONINDEX) };

    if result == 0 {
        // 存在しないパス向けのフォールバック。属性に `FILE_ATTRIBUTE_NORMAL`
        // を明示することで、拡張子から汎用アイコンを引く。
        info = SHFILEINFOW::default();
        // SAFETY: 上記と同じく `wide` / `info` / `info_size` は有効。`flags` に
        // `SHGFI_USEFILEATTRIBUTES` を加えたので第 2 引数は属性として解釈され、
        // `FILE_ATTRIBUTE_NORMAL` を渡す。呼び出し前に `info` をゼロ初期化し直
        // しているため前回の結果は残らない。STA は依然として生きている。
        result = unsafe {
            SHGetFileInfoW(
                wide.as_ptr(),
                FILE_ATTRIBUTE_NORMAL,
                &mut info,
                info_size,
                SHGFI_SYSICONINDEX | SHGFI_USEFILEATTRIBUTES,
            )
        };
    }

    // `SHGFI_SYSICONINDEX` の戻り値はシステムイメージリストのハンドル。
    // 0 は失敗。`iIcon` は符号付きなので負値をインデックスに変換しない。
    if result == 0 || info.iIcon < 0 {
        return None;
    }
    Some(info.iIcon as u32)
}

/// パスに対応する小さい HICON を取得し、RAII ラッパーで返す（Windows）。
///
/// 返された [`ScopedIcon`] は `!Send + !Sync` で、ドロップ時に同じスレッド上で
/// `DestroyIcon` が呼ばれる。呼び出し元はバイト列を直接触らず、必要になった
/// 段階で描画側がインデックス（[`file_icon_index`]）を用いること。
#[cfg(windows)]
pub fn file_icon_handle(path: &Path) -> Option<ScopedIcon> {
    let _sta = ComSta::enter()?;
    let wide = wide_path_with_nul(path);
    let mut info = SHFILEINFOW::default();
    let info_size = std::mem::size_of::<SHFILEINFOW>() as u32;

    // SAFETY: `wide` は `wide_path_with_nul` が生成した NUL 終端 UTF-16、
    // `info` は有効な `SHFILEINFOW` 参照、
    // `info_size` はそのサイズ。`SHGFI_ICON | SHGFI_SMALLICON` により
    // `info.hIcon` は所有権付き HICON として返る。成功直後に `ScopedIcon` へ
    // 移すため early return でもリークしない。STA は `_sta` の生存期間中だけ
    // 有効で、HWND は一切渡していない。
    let mut result = unsafe {
        SHGetFileInfoW(
            wide.as_ptr(),
            0,
            &mut info,
            info_size,
            SHGFI_ICON | SHGFI_SMALLICON,
        )
    };

    if result == 0 || info.hIcon.is_null() {
        info = SHFILEINFOW::default();
        // SAFETY: 同上。存在しないパスでは属性指定で再問い合わせする。`info` を
        // 作り直すので前回のハンドル値は残らず、二重所有権は生じない。
        result = unsafe {
            SHGetFileInfoW(
                wide.as_ptr(),
                FILE_ATTRIBUTE_NORMAL,
                &mut info,
                info_size,
                SHGFI_ICON | SHGFI_SMALLICON | SHGFI_USEFILEATTRIBUTES,
            )
        };
    }

    if result == 0 || info.hIcon.is_null() {
        return None;
    }
    Some(ScopedIcon {
        handle: info.hIcon,
        _thread: PhantomData,
    })
}

/// Windows 以外: アイコン API は存在しないため常に `None`。
#[cfg(not(windows))]
pub fn file_icon_index(_path: &Path) -> Option<u32> {
    None
}

/// Windows 以外: アイコン API は存在しないため常に `None`。
#[cfg(not(windows))]
pub fn file_icon_handle(_path: &Path) -> Option<ScopedIcon> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_with_nul_is_exactly_once_terminated() {
        let wide = wide_with_nul("C:\\");
        assert_eq!(wide, vec![b'C' as u16, b':' as u16, b'\\' as u16, 0]);
        assert_eq!(wide.iter().filter(|unit| **unit == 0).count(), 1);
    }

    #[test]
    fn wide_with_nul_truncates_at_interior_nul_and_keeps_unicode() {
        // 先頭 NUL 以降は Win32 から見えないため、取り除いて連結するのではなく
        // 切り詰める。Unicode はそのまま UTF-16 に変換される。
        let wide = wide_with_nul("日\0本");
        assert_eq!(wide, vec!['日' as u16, 0]);
        assert_eq!(string_from_wide(&wide), "日");
        assert_eq!(
            wide_with_nul("日本語"),
            vec!['日' as u16, '本' as u16, '語' as u16, 0]
        );
    }

    #[test]
    fn string_from_wide_stops_at_first_nul_and_is_lossy() {
        assert_eq!(string_from_wide(&[b'A' as u16, 0, b'B' as u16]), "A");
        assert_eq!(string_from_wide(&[b'A' as u16, b'B' as u16]), "AB");
        assert_eq!(string_from_wide(&[]), "");
        // 単独サロゲートはlossy置換され、panic しない。
        assert_eq!(string_from_wide(&[0xD800, 0]), "\u{FFFD}");
    }

    #[test]
    fn mount_info_matches_app_projection_rules() {
        let with_label = mount_info_from_volume('C', "System", "NTFS");
        assert_eq!(with_label.source, "C: (System)");
        assert_eq!(with_label.path, "C:\\");
        assert_eq!(with_label.fs_type, "NTFS");

        let no_label = mount_info_from_volume('Z', "", "exFAT");
        assert_eq!(no_label.source, "Z:");
        assert_eq!(no_label.path, "Z:\\");
        assert_eq!(no_label.fs_type, "exFAT");
    }

    #[test]
    #[cfg(not(windows))]
    fn non_windows_api_is_inert() {
        assert!(logical_drives().is_empty());
        assert!(file_icon_index(Path::new("C:\\test.rs")).is_none());
        assert!(file_icon_handle(Path::new("C:\\test.rs")).is_none());
    }
}
