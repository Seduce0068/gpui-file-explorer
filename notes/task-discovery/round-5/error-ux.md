---
type: Reference
title: エラーの分類と回復可能性の残タスク調査（PASS 5）
description: >-
  gpui-file-explorer の DESIGN.md エラー契約・OpError / FsError の文言・main.rs の
  ステータス生成とログ出力を照合し、「失敗後にユーザーが何ができるか」の観点で
  回復導線の欠落を発掘したレン別調査。全所見はファイル:行で検証済み。
tags:
  - task-discovery
  - gpui-file-explorer
  - error-handling
  - ux
  - pass-5
generated: 2026-11-17T21:29:31+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 調査範囲と前提

`DESIGN.md:185-260`（状態表示・エラー契約）、`:615-642`（LoadState / FsError）、
`:738-775`（失敗の集約・権限・trash 失敗）、`app/src/ops.rs:64-129`（OpError と Display / io_error 変換）、
`app/src/model.rs:17-75,683-697`（FsError / create_unique_folder）、
`app/src/strings.rs:96-133`（STATUS_* / OP_* 定数）、
`app/src/main.rs` の `status = format!`（約 60 箇所）、`tracing!` / `eprintln!`、エラー分岐、
`app/README.md:351-382`（既知の制限）を照合した。

重複回避のため、open/reveal 失敗検知、監視エラー导致的 status 上書き、RUST_LOG フォールバック、
エラー文言のパス露出（セキュリティレンズ）、エラーステータス優先度（r1-qual-09）、
設定 read-only 文言、無効 CLI パス診断、case-only rename の O_EXCL、EXDEV フォールバックのテストは
本見出しでは扱わない。バッチの部分成功自体は既知として、その** precise な改善点**
（どの情報が落ちるか）のみを r5-err-01 に記録した。

## 発見事項

### r5-err-01 バッチ転送は最初のエラーしか保持せず失敗項目の一覧を捨てる

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-err-01 | `first_error` 1 個だけを保持し、2 件目以降の失敗パスと理由を破棄する | 残タスク | `app/src/main.rs:1885`（`let mut first_error`）, `:1926-1934`（`if first_error.is_none()` で上書きしない）, `:1945-1966`（`error_text` と status は 1 件目のみ）；`app/src/ops.rs:51-52`（UI は常に `Rename` のため `OpStats::skipped` は充実しない）, `:94-107` | 複数ファイルのコピー/移動で 3 件失敗しても、status と tracing ログには 1 件目のエラーだけが出る。残り 2 件のパス・理由は `Err(error)` のローカル変数として捨てられ、ユーザーはどれを再試行すればよいか分からない。DESIGN.md:742 は「1 件失敗しても全体を止めず、結果を集約」と明記しており、件数集約しか実装されていない。 | 失敗した項目を列挙して「これだけ再試行してください」を提示でき、再コピーの無駄と見落としを防ぐ。 | S | 中 | 高 |

### r5-err-02 キャンセルが部分完了を隠し「何が済んだか」を伝えない

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-err-02 | `OpError::Cancelled` の分岐が成功件数に関わらず「キャンセルしました」で上書きする | 残タスク | `app/src/main.rs:1889-1892`（ループ先頭で `Cancelled` を代入）, `:1927-1933`（キャンセルで break）, `:1958-1966`（`succeeded` を計算しながら `Some(OpError::Cancelled)` が最優先）, `app/README.md:379-380`（処理済み項目は元に戻らないと明記） | Esc でキャンセルすると、それまでにコピー済みのファイル数・バイト数が status に出ず「キャンセルしました」だけになる。README は「処理済みの項目は元に戻りません」と注意しているが、実際に何件処理済みかはログ（`stats.files`）を見ないと分からない。ユーザーは再貼り付けで重複を避けられない。 | 「N 件コピー済みでキャンセルしました」とするだけで、やり直し範囲の判断と重複コピーの回避ができる。 | S | 新規 | 高 |

### r5-err-03 クロスデバイス移動の「コピー成功・元削除失敗」に移動先が示されない

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-err-03 | EXDEV フォールバックの削除失敗が `OpError::Io` に潰れ、コピー済みの移動先パスが消える | 残タスク | `app/src/ops.rs:619-643`（`:624-627` コピー、`:633-640` 削除失敗時に `OpError::Io { path: src, message: "コピーは完了しましたが元を削除できませんでした…" }`）, `app/src/main.rs:1964`（`{verb}できません: {error}`）, `:76-88` の Display（`Io` は `{path}: {message}`） | 移動先 `dest` は `stats.created` に入るが、エラー経路では `Err` が返るため main.rs の `:1922-1924` を通らず、status には「元のパス: コピーは完了しましたが元を削除できませんでした」としか出ない。結果として「コピーはどこにあるか」「元を手動削除すればよいか」の判断材料が欠ける（安全設計上、自動削除はしない方針: DESIGN.md:768）。 | 移動先パスを併記し「元を手動で削除してください」と案内すれば、データ重複と領域圧迫をユーザーが解消できる。 | S | 新規 | 高 |

### r5-err-04 ロードエラー表示に「F5 で再試行」「親フォルダへ」の導線が無い

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-err-04 | DESIGN が約束した再試行・親移動の affordance が実装されていない | 機能追加案 | `DESIGN.md:190`, `:253-255`（「再試行 F5」「親フォルダへ」ボタン）, `app/src/main.rs:199-201`（`empty_state_message` は `("エラー: {error}", None)` で hint 常になし）, `:4300-4335`（overlay は message + 任意 hint のみ、ボタンなし）, `:5067-5071`（status バーは赤文字のみ）, `app/src/strings.rs` に「再試行」定数なし（grep で main.rs / strings.rs に「再試行」「親フォルダへ」0 件） | 権限なし・削除済み・I/O エラーのいずれでも、一覧中央とステータスバーに文言が出るだけで、次に何を押せばよいかが書かれていない。F5 はグローバルに効くが、設計書どおりの可視化された回復操作が無い。 | 「F5 で再読込」「Alt+↑ で親へ」をエラー表示に添えるだけで、 PermissionDenied / NotFound の典型的な回避手順に到達できる。 | S | 新規 | 高 |

### r5-err-05 操作失敗を `LoadState::Error` に格上げして健全な一覧全体を隠す

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-err-05 | 新規フォルダ失敗・ゴミ箱全件失敗がディレクトリ読込エラーとして描画される | 構造改善 | `app/src/main.rs:1693-1698`（`create_unique_folder` 失敗で `LoadState::Error`）, `:3536-3546`（trash 全件失敗で `LoadState::Error` かつ `failures.len()` を破棄）, `:4300-4335`（`load.error_message()` があると overlay が一覧を覆う）, `:5067-5071`（status バーも赤文字で占有） | ディレクトリ自体は読めており一覧データは健在なのに、書き込み操作の失敗が `LoadState::Error` を汚染し、画面中央がエラー表示になる。ゴミ箱全件失敗では「何件失敗したか」も捨てられる。ユーザーは「一覧が壊れた」と誤認し、実際に必要なのは権限確認や件数把握なのに気付けない。 | 操作失敗は `status` または専用通知に分離し、`LoadState::Error` は読込失敗専用に保つだけで、状態の意味と回復手順が一致する。 | M | 新規 | 高 |

### r5-err-06 Undo の事前チェック失敗で undo 履歴項目が消えて再試行できない

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-err-06 | `pop_undo()` 後に早期 return する分岐が action を戻さない | 残タスク | `app/src/main.rs:2298`（`let Some(action) = self.undo.pop_undo()`）, `:2313-2317`（`!current.exists()` で return）, `:2319-2323`（`target.exists()` で return）, 対比: `:2347-2349` / `:2386-2388` の非同期失敗では `push_undo` で戻す | 「元に戻せません: … が存在しません」「… が既に存在します」の事前チェックに引っかかった場合、UndoAction はスタックから popped のまま失われる。競合を解消（例: 対象名を別名に変更）して Ctrl+Z を再試行しても「元に戻せる操作がありません」になる。非同期失敗では保持されるため挙動も一貫しない。 | 早期 return 前に `push_undo(action)` するだけで、競合解消後の再試行が可能になり、操作履歴の信頼性が保たれる。 | S | 新規 | 高 |

### r5-err-07 エラー種別ごとの文言粒度が揃わず回復手順が伝わらない

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-err-07 | PermissionDenied / NotFound / InvalidTarget が「アクセスが拒否されました」等に畳まれ、設計書の具体的案内と乖離 | 残タスク | `app/src/ops.rs:79-88`（Display は「見つかりません: path」「アクセスが拒否されました: path」「操作できません: reason」）, `:109-129`（io_error のマッピング）, `app/src/model.rs:54-67`（`FsErrorKind::Io` → 「読み取れませんでした」、`Unknown` → 「エラーが発生しました」）, `DESIGN.md:767-769`（「権限がないため操作できません」「属性解除を自動でやらない」）, `app/src/strings.rs:126`（`STATUS_UNDO_FAILED` の汎用接頭辞） | 設計書は権限不足・削除済みで「親へ移動」を、EACCES/EPERM で「権限がないため手動で属性解除」を想定しているが、実装は理由語＋パスだけで次の操作を示さない。`STATUS_UNDO_FAILED: {error}` のように接頭辞を重ねると「元に戻せません: 操作できません: …」と二重の否定文言になる。 | 代表的な 3 種別に「親へ」「権限を確認」「名前を変更」の固定ヒントを対応付けるだけで、エラーの種類が行動に変換できる。 | S | 中 | 高 |

### r5-err-08 生の `io::Error` 文言がユーザー画面に混入し内部識別子を漏らす

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-err-08 | `err.to_string()` / `format!("{}: {error}", …)` が OS エラー番号・内部語をそのまま表示する | 調査項目 | `app/src/ops.rs:124-127`（`OpError::Io { message: err.to_string() }`）, `:87`（`{path}: {message}`）, `app/src/model.rs:49,62-65`（`os_message` を `(message)` として連結）, `app/src/model.rs:690`（`create_unique_folder` の `format!("{}: {error}", …)`）, `DESIGN.md:860`（文言規約は日本語・統一語彙） | 「No such file or directory (os error 2)」「Permission denied (os error 13)」等の英語・errno が、日本語の固定文言に続いてそのまま出る。エラー種別が `NotFound` / `PermissionDenied` に分かれているのに重複して詳細が出るため、ユーザーにはノイズになり、内部実装の識別子も露出する（パス自体の露出は他レンズ対象のためここでは扱わない）。 | `io::Error::raw_os_error()` を既存の日本語理由へ吸収し、詳細は折りたたみ表示にすれば、文言の一貫性と可読性が上がる。詳細を残すかは設計判断が必要。 | S | 中 | 中 |

### r5-err-09 ステータスとログの記録先が非対称で、消えるエラーと UI に出ないエラーが混在

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-err-09 | UI に出るエラーの多くがログに残らず、逆に eprintln だけのエラーもある | 構造改善 | `app/src/main.rs:1949-1957`（転送完了は `tracing::info!` に `error` フィールド付き）, `:1693-1698`（フォルダ作成失敗はログ無し）, `:3326-3328`（rename 失敗はログ無し）, `:4052-4055`（復元失敗はログ無し）, `:2347-2349,2386-2388`（Undo 失敗はログ無し）, `:2263,2282,2620`（`eprintln!` のみで UI に出ない）, `app/README.md:75`（tracing 構造化ログを謳う） | status は次の操作で上書きされる一時表示なので、ログに残らないエラーは後から原因を辿れない。一方、`preview-report: error …` や debug 用 `undo-report` の失敗は stderr にしか出ず、通常ユーザーには見えない。同じ「失敗」が記録先によって扱いが異なる。 | エラーを status に出す経路へ一律 `tracing::warn!` を 1 行足すだけで、再現・支援時の手掛かりが残る（既存の eprintln は debug フラグ配下のまま維持可能）。 | S | 新規 | 高 |

### r5-err-10 「このエラーのときは何をするか」のユーザー向け説明が無い

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-err-10 | エラー文言と対処手順の対応表がドキュメントにも UI にも存在しない | 残タスク | `app/README.md:351-382`（「既知の制限」に open/reveal 失敗やバッチcancel の注意はあるが、権限・不在・衝突時の手順はなし）, `DESIGN.md:253-256,767-773`（対処方針は設計側にのみ記載）, `app/src/strings.rs:7-77`（F1 ヘルプはショートカット一覧のみでエラー対処なし）, `app/src/main.rs` / `app/src/strings.rs` に「再試行」「親フォルダへ」0 件（grep 確認） | 設計書には回復方針が書かれているが、利用者が読む README と F1 ヘルプには反映されていない。実際に出る文言（「アクセスが拒否されました」「既に存在します」「ゴミ箱へ移動できません」等）から、次に F5 を押す／親へ移動する／名前を変える、を引ける手段が無い。 | 既存文言を見出しにした短い「エラーが出たら」節を README と F1 に追記するだけ（実装変更不要）で、サポート問い合わせと誤操作を減らせる。 | S | 新規 | 高 |

## レンズ総評

エラー型自体は `OpError`（7 バリアント）と `FsError`（5 バリアント）で用途別に分かれており、
Display も日本語化されているため「何が起きたか」は概ね伝わる。問題はその**先**にある。

- **集約の粒度**: r5-err-01 / 02 のように、設計は「失敗を集約する」と書きながら実装は
  `first_error` 1 個＋成功件数に留まっており、回復に必要な「どれ／いくつ残ったか」が落ちる。
- **状態の意味**: r5-err-05 のように操作失敗が `LoadState::Error` に流入し、健全な一覧を隠す。
  読込失敗と操作失敗を分けるだけで、多くの画面が自己矛盾しなくなる。
- **導線の欠落**: r5-err-04 / 06 / 10 は、 DESIGN が明示した「F5」「親へ」「再試行」が
  実装・ドキュメントの両方で消えている典型。特に Undo の早期 return は再試行手段そのものを
  奪うため、優先度が高い。
- **記録の非対称**: r5-err-09 のように一時 status と永続ログの使い分けが unstructured で、
  消えるエラーと stderr だけのエラーが混在している。

いずれも broad rewrite を要さず、`first_error` を `Vec` にする／キャンセル分岐に件数を足す／
早期 return 前に push 戻す／文字列を 1 行足す、といった小さな変更で検証できる。
まず r5-err-06（データ損失に近い）→ r5-err-01 / 02（回復判断の材料）→ r5-err-04 / 10（導線）
の順で手を付けるのが費用対効果が高い。
