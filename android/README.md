<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# android/ — Compose 1 画面ビューア scaffold

Android は **Compose アプリ + PWA** の両方に対応する方針のうち、ネイティブ側の
1 画面ビューア scaffold。iOS は Apple Developer Program の年会費が必ずかかるため
アプリ版を出さず PWA のみ（`pwa/`）に対応する。

## この環境ではビルド検証不可

- `java`: 未インストール（`java -version` → `command not found`）
- Android SDK: `ANDROID_HOME` / `ANDROID_SDK_ROOT` 未設定、`~/Android/Sdk` なし
- よって `./gradlew build` は実行せず、**構成のみ**を作成している

依存の解決やコンパイルは試していない。Gradle / AGP / Kotlin / Compose のバージョンは
scaffold 用の例示であり、実機ビルド時に利用可能なツールチェーンへ合わせること。

## 受け入れ基準

**Phase 1 spike の受け入れは外観ではなくコア API 経由の一覧・検索表示。**
現状の `MainActivity.kt` は検索フィールドと `LazyColumn` の骨格だけで、
一覧データはサンプル。`TODO(P1-04)` をコア API 呼び出しに置き換えるまで受け入れではない。

## 構成

| ファイル | 役割 |
| --- | --- |
| `settings.gradle.kts` | Kotlin DSL、`pluginManagement` / `dependencyResolutionManagement`、`rootProject.name = "gfe-android"` |
| `build.gradle.kts` | AGP / Kotlin Android / Compose compiler plugin を `apply false` で宣言 |
| `app/build.gradle.kts` | Compose BOM + `activity-compose` + `material3`、namespace `com.example.gfe` |
| `app/src/main/AndroidManifest.xml` | `MainActivity` をランチャー Activity として登録 |
| `app/src/main/res/values/themes.xml` | Compose 用の NoActionBar テーマ |
| `app/src/main/java/com/example/gfe/MainActivity.kt` | 検索ボックス + `LazyColumn` の 1 画面骨格 |

`android/` と `pwa/` はルート `Cargo.toml` の workspace members には入れず、
既存のデスクトップ用ビルドを壊さない。

## core staticlib を JNI で呼ぶ方針

1. Rust 側: `core/` を Android ターゲット（`aarch64-linux-android` /
   `armv7-linux-androideabi`）向けに `crate-type = ["staticlib"]` でビルドし
   `libgfe_core.a` を得る。現行ターゲットは `rustup target list --installed` で確認済み。
2. JNI 側: `jni-rs`（または `cxx`）で `libgfe_core.a` をリンクした
   `libgfe_jni.so` を作り、`Java_com_example_gfe_…` の extern 関数として
   `CoreViewer::list_entries` / `search` を公開する。
3. Kotlin 側: `external fun listEntries(): Array<EntryDto>` 等を宣言し、
   `EntryDto` を `EntryUiModel` に変換して `LazyColumn` へ渡す。
   `MainActivity.kt` の `TODO(P1-04)` がこの差し替え箇所。

`unsafe` は FFI 境界のみで必要になるが、プロジェクト規約（共通 AGENTS.md §15）により
事前に理由・影響範囲・安全性の根拠を文書化しレビュー合意を得るまで使用しない。
null ポインタや所有権の受け渡しを安易に `unsafe` で済ませないこと。

## ビルド手順（SDK / JDK を用意した環境で実行）

```bash
echo '--- Rust staticlib ---'
cargo build --release --target aarch64-linux-android --manifest-path core/Cargo.toml

echo '--- JNI 共有ライブラリを CMake でビルド ---'
# TODO(P1-04): CMakeLists.txt と jni/ crate を追加する

echo '--- Gradle ---'
cd android && ./gradlew assembleDebug
```

`gradlew` / CMake 設定は JNI 実装と同時に追加する予定で、この scaffold には含まない。
