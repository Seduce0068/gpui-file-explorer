// SPDX-License-Identifier: AGPL-3.0-or-later
//! 旧 `error` モジュールは `gfe_core::error` へ移動した（R2-CORE-MOVE）。
//! `model` が `#[path = "error.rs"]` で内包していた頃の直接参照も
//! `gfe_core::model::error` の re-export で解決されるため、このファイルは
//! 互換用の薄い re-export のみ残す。

pub use gfe_core::error::*;
