// SPDX-License-Identifier: AGPL-3.0-or-later
package com.example.gfe

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            MaterialTheme {
                ExplorerScreen(entries = demoEntries())
            }
        }
    }
}

/** core の Entry を画面表示用に写す最小モデル（JNI 変換後の受け皿）。 */
data class EntryUiModel(
    val name: String,
    val path: String,
    val isDir: Boolean,
    val size: Long,
)

@Composable
fun ExplorerScreen(entries: List<EntryUiModel>) {
    var query by remember { mutableStateOf("") }
    // 選択状態は 1 画面（Activity の setContent 内容）で保持し、各行へ isSelected として渡す。
    var selected by remember { mutableStateOf<EntryUiModel?>(null) }
    var details by remember { mutableStateOf<String?>(null) }
    val snackbarHostState = remember { SnackbarHostState() }

    // long-press の詳細は Snackbar で表示する（Material3 の最小構成）。
    LaunchedEffect(details) {
        details?.let { snackbarHostState.showSnackbar(it) }
    }

    Scaffold(snackbarHost = { SnackbarHost(snackbarHostState) }) { innerPadding ->
        Column(modifier = Modifier.fillMaxSize().padding(innerPadding)) {
            OutlinedTextField(
                value = query,
                onValueChange = { query = it },
                label = { Text("検索") },
                singleLine = true,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(16.dp),
            )

            // TODO(P1-04): ここをコア API 経由に差し替える。
            //   JNI (jni-rs / cxx) で core の staticlib を呼び、
            //   CoreViewer::list_entries / search の戻り値を EntryUiModel へ変換する。
            //   受け入れは外観ではなくコア API 経由の一覧・検索表示であること。
            val visible = remember(entries, query) {
                entries.filter { it.name.contains(query, ignoreCase = true) }
            }

            LazyColumn(modifier = Modifier.fillMaxSize()) {
                items(items = visible, key = { it.path }) { entry ->
                    EntryRow(
                        entry = entry,
                        isSelected = selected?.path == entry.path,
                        onSelect = { selected = it },
                        onDetails = { details = it.detailText() },
                    )
                    HorizontalDivider()
                }
            }
        }
    }
}

private fun EntryUiModel.detailText(): String =
    "path=$path, size=$size, isDir=$isDir"

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun EntryRow(
    entry: EntryUiModel,
    isSelected: Boolean,
    onSelect: (EntryUiModel) -> Unit,
    onDetails: (EntryUiModel) -> Unit,
) {
    val background = if (isSelected) {
        MaterialTheme.colorScheme.primaryContainer
    } else {
        MaterialTheme.colorScheme.surface
    }
    ListItem(
        headlineContent = { Text(entry.name) },
        supportingContent = { Text(entry.path) },
        leadingContent = { Text(if (entry.isDir) "\uD83D\uDCC1" else "\uD83D\uDCC4") },
        trailingContent = { Text(if (entry.isDir) "—" else entry.size.toString()) },
        modifier = Modifier
            .padding(horizontal = 8.dp)
            .background(background)
            // combinedClickable が tap 選択と long-press 詳細の両方を担う（単体 clickable は不要）。
            .combinedClickable(
                onClick = { onSelect(entry) },
                onLongClick = { onDetails(entry) },
            ),
    )
}

private fun demoEntries() = listOf(
    EntryUiModel("README.md", "/sdcard/gfe/README.md", false, 2_048),
    EntryUiModel("src", "/sdcard/gfe/src", true, 0),
    EntryUiModel("Cargo.toml", "/sdcard/gfe/Cargo.toml", false, 330),
)
