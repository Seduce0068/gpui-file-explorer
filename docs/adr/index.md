# ADR 索引

新要件（Windows Explorer / Double Commander 級 UX + モバイル対応）に伴う
個別 ADR の索引。DESIGN.md の確定済み ADR-001〜012 とは別系列。

<!-- 自動生成セクション: 手動で ADR を追加したらこの表も更新する -->

| ADR | 状態 | 概要 |
| --- | --- | --- |
| [モバイル対応](adr-20261122-mobile-android-ios-support.md) | approved（2026-11-23） | 選択肢B（Rust コア共有＋Compose / PWA）。iOS は PWA のみ |
| [デュアルペイン UI](adr-20261122-dual-pane-ui.md) | draft | PaneState 抽出による左右2画面の段階的導入（未承認） |
| [日本語照合](adr-20261123-japanese-collation.md) | draft | ICU4X collator による五十音順（未承認） |
| [OS ファイルクリップボード](adr-20261123-os-file-clipboard.md) | draft | CF_HDROP / text-uri-list の相互運用（未承認） |
| [Windows ネイティブ API 戦略](adr-20261123-windows-native-api-strategy.md) | draft | Win32 FFI 隔離（gfe-win32）と WinUI 3 不採用（未承認） |

承認済みはモバイル ADR のみ。他4件は設計判断として保留中。
