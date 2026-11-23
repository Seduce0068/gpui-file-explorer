// SPDX-License-Identifier: AGPL-3.0-or-later
//! GPUI アダプタの土台（P1-03）。
//!
//! core のポートは同期 API。Phase 1 では OS 連携（reveal / open-with /
//! picker / clipboard）をまだこのアダプタへ接続せず、既存の `main.rs` が
//! `cx.reveal_path` / `cx.open_with_system` を直接呼ぶ経路を維持する。
//! ここではプラットフォーム非依存の no-op 実装と、アダプタの境界を固定する。

use std::path::PathBuf;

use gfe_core::ports::{Clipboard, OpenRevealPrompt, PortError};

/// Phase 1 用の no-op プロンプト（アダプタの土台として保持）。
#[allow(dead_code)]
///
/// picker は UI を伴うため未接続（Toolbar 側の既存 prompt を維持）。
pub struct PlaceholderPrompt;

impl OpenRevealPrompt for PlaceholderPrompt {
    fn pick_paths(&self) -> Result<Vec<PathBuf>, PortError> {
        Err(PortError::message(
            "GPUI file picker is not wired in Phase 1",
        ))
    }

    fn reveal(&self, _path: &std::path::Path) -> Result<(), PortError> {
        // main.rs の cx.reveal_path を経由する既存経路を維持するため no-op。
        Ok(())
    }

    fn open_with_system(&self, _path: &std::path::Path) -> Result<(), PortError> {
        // 同上。
        Ok(())
    }
}

/// クリップボードは Phase 1 では app 内部スロットのみ（NW10 の ADR 参照）。
#[allow(dead_code)]
pub struct NoopClipboard;

impl Clipboard for NoopClipboard {
    fn read_uri_list(&self) -> Result<Vec<String>, PortError> {
        Ok(Vec::new())
    }

    fn write_uri_list(&self, uris: &[String]) -> Result<(), PortError> {
        if uris.is_empty() {
            Ok(())
        } else {
            Err(PortError::message(
                "OS clipboard write is not wired in Phase 1",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn placeholder_prompt_is_explicit_about_missing_picker() {
        let prompt = PlaceholderPrompt;
        assert!(prompt.pick_paths().is_err());
        assert!(prompt.reveal(Path::new("/tmp/a")).is_ok());
        assert!(prompt.open_with_system(Path::new("/tmp/a")).is_ok());
    }

    #[test]
    fn noop_clipboard_accepts_empty_write() {
        let clipboard = NoopClipboard;
        assert!(clipboard.read_uri_list().unwrap().is_empty());
        assert!(clipboard.write_uri_list(&[]).is_ok());
        assert!(clipboard.write_uri_list(&["file:///tmp/a".into()]).is_err());
    }
}
