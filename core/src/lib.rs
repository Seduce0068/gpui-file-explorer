// SPDX-License-Identifier: AGPL-3.0-or-later
//! GPUI 非依存のコアロジック（Phase 1 spike）。
//!
//! `app` クレートは `use gfe_core as core;` でローカル別名を張り、
//! `core::{model, ops, search}` という参照形を維持する。
//! sysroot の `core` クレートとの名前衝突を避けるため lib 名は `gfe_core`。
//!
//! app 側の `#![forbid(unsafe_code)]` は crate 単位のため、ここでも明示する。

#![forbid(unsafe_code)]

pub mod error;
pub mod model;
pub mod ops;
pub mod ports;
pub mod search;
pub mod strings;
pub mod trash_view;
