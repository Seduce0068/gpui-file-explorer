// SPDX-License-Identifier: AGPL-3.0-or-later
package com.example.gfe

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
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
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
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

    Column(modifier = Modifier.fillMaxSize()) {
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
                EntryRow(entry = entry)
                HorizontalDivider()
            }
        }
    }
}

@Composable
private fun EntryRow(entry: EntryUiModel) {
    ListItem(
        headlineContent = { Text(entry.name) },
        supportingContent = { Text(entry.path) },
        leadingContent = { Text(if (entry.isDir) "\uD83D\uDCC1" else "\uD83D\uDCC4") },
        trailingContent = { Text(if (entry.isDir) "—" else entry.size.toString()) },
        modifier = Modifier.padding(horizontal = 8.dp),
    )
}

private fun demoEntries() = listOf(
    EntryUiModel("README.md", "/sdcard/gfe/README.md", false, 2_048),
    EntryUiModel("src", "/sdcard/gfe/src", true, 0),
    EntryUiModel("Cargo.toml", "/sdcard/gfe/Cargo.toml", false, 330),
)
