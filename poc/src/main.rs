//! Minimal GPUI file explorer PoC.
//!
//! Stack: gpui-kit 0.9.1 (facade over gpui-pre 0.3.12 = Zed gpui snapshot
//! zed@d2a4fb6 + gpui-pre-platform 0.3.12 with x11/wayland backends).
//!
//! Features demonstrated:
//! - window open via the documented gpui-kit entry point
//! - directory read on the GPUI background executor (no blocking in render)
//! - virtualized list via `uniform_list` + `UniformListScrollHandle`
//! - Up/Down selection, Enter opens dir, Backspace goes to parent, Esc closes
//! - status line with path, entry count, selected name

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use gpui_kit::prelude::*;
use gpui_kit::*;

#[derive(Clone)]
struct Entry {
    name: String,
    path: PathBuf,
    is_dir: bool,
    size: u64,
    modified: Option<SystemTime>,
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "K", "M", "G", "T"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn human_age(modified: Option<SystemTime>) -> String {
    let Some(modified) = modified else {
        return "?".to_string();
    };
    let Ok(elapsed) = SystemTime::now().duration_since(modified) else {
        return "future".to_string();
    };
    let secs = elapsed.as_secs();
    if secs < 60 {
        format!("{secs}s ago")
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86_400 {
        format!("{}h ago", secs / 3600)
    } else if secs < 86400 * 365 {
        format!("{}d ago", secs / 86_400)
    } else {
        format!("{}y ago", secs / (86400 * 365))
    }
}

/// Blocking directory read. Runs on the background executor.
fn read_entries(dir: &Path) -> Result<Vec<Entry>, String> {
    let dir_reader = std::fs::read_dir(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    let mut entries = Vec::new();
    for item in dir_reader {
        let Ok(item) = item else { continue };
        let path = item.path();
        let name = item.file_name().to_string_lossy().to_string();
        // `DirEntry::metadata` does not follow symlinks; use `fs::metadata` for
        // the directory test so symlinked dirs are enterable.
        let metadata = item.metadata().ok();
        let is_dir = std::fs::metadata(&path)
            .map(|meta| meta.is_dir())
            .unwrap_or(false);
        let size = metadata.as_ref().map(|meta| meta.len()).unwrap_or(0);
        let modified = metadata.and_then(|meta| meta.modified().ok());
        entries.push(Entry {
            name,
            path,
            is_dir,
            size,
            modified,
        });
    }
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

struct Explorer {
    cwd: PathBuf,
    entries: Vec<Entry>,
    selected: usize,
    status: String,
    loading: bool,
    generation: u64,
    focus_handle: FocusHandle,
    scroll_handle: UniformListScrollHandle,
}

impl Explorer {
    fn new(cx: &mut Context<Self>) -> Self {
        let cwd = std::env::args()
            .nth(1)
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("/"));
        let mut this = Self {
            cwd,
            entries: Vec::new(),
            selected: 0,
            status: "Loading…".to_string(),
            loading: true,
            generation: 0,
            focus_handle: cx.focus_handle(),
            scroll_handle: UniformListScrollHandle::new(),
        };
        this.reload(cx);
        this
    }

    /// Kick off an async directory load. The actual `read_dir` happens on the
    /// background executor; state is applied back on the foreground thread.
    fn reload(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let dir = self.cwd.clone();
        let task = cx.background_spawn(async move { read_entries(&dir) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                // Drop stale loads (fast navigation).
                if this.generation != generation {
                    return;
                }
                this.loading = false;
                match result {
                    Ok(entries) => {
                        this.status = format!("{} entries", entries.len());
                        this.entries = entries;
                        this.selected = 0;
                        this.scroll_handle.scroll_to_item(0, ScrollStrategy::Top);
                    }
                    Err(err) => {
                        this.entries.clear();
                        this.selected = 0;
                        this.status = format!("error: {err}");
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.entries.is_empty() {
            return;
        }
        let index = index.min(self.entries.len() - 1);
        if self.selected != index {
            self.selected = index;
            self.scroll_handle
                .scroll_to_item(index, ScrollStrategy::Nearest);
            cx.notify();
        }
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.entries.is_empty() {
            return;
        }
        let len = self.entries.len() as isize;
        let next = (self.selected as isize + delta).clamp(0, len - 1) as usize;
        self.select(next, cx);
    }

    fn open_selected(&mut self, cx: &mut Context<Self>) {
        let Some(entry) = self.entries.get(self.selected).cloned() else {
            return;
        };
        if entry.is_dir {
            self.cwd = entry.path;
            self.status = "Loading…".to_string();
            self.reload(cx);
            cx.notify();
        } else {
            self.status = format!("file: {} ({})", entry.name, human_size(entry.size));
            cx.notify();
        }
    }

    fn go_parent(&mut self, cx: &mut Context<Self>) {
        if let Some(parent) = self.cwd.parent() {
            self.cwd = parent.to_path_buf();
            self.status = "Loading…".to_string();
            self.reload(cx);
            cx.notify();
        }
    }

    fn key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "up" => self.move_selection(-1, cx),
            "down" => self.move_selection(1, cx),
            "enter" => self.open_selected(cx),
            "backspace" => self.go_parent(cx),
            "escape" => cx.quit(),
            _ => {}
        }
    }
}

impl Render for Explorer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let path_label = format!("{}", self.cwd.display());
        let count = self.entries.len();
        let selected_name = self
            .entries
            .get(self.selected)
            .map(|entry| {
                format!(
                    "{}  {}  {}",
                    entry.name,
                    if entry.is_dir {
                        "<dir>".to_string()
                    } else {
                        human_size(entry.size)
                    },
                    human_age(entry.modified)
                )
            })
            .unwrap_or_else(|| "-".to_string());

        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .py_2()
            .bg(rgb(0x181825))
            .border_b_1()
            .border_color(rgb(0x313244))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!("GPUI File Explorer PoC  —  {path_label}")),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x6c7086))
                    .child("gpui-kit 0.9.1 / gpui-pre 0.3.12"),
            );

        let list = uniform_list(
            "file-entries",
            count,
            cx.processor(
                |this: &mut Self, range: std::ops::Range<usize>, _window, cx| {
                    let mut items = Vec::with_capacity(range.len());
                    for index in range {
                        let Some(entry) = this.entries.get(index).cloned() else {
                            continue;
                        };
                        let selected = this.selected == index;
                        let click =
                            cx.listener(move |this: &mut Self, event: &ClickEvent, _window, cx| {
                                this.select(index, cx);
                                if event.click_count() >= 2 {
                                    this.open_selected(cx);
                                }
                            });
                        let row_bg = if selected {
                            rgb(0x45475a)
                        } else {
                            rgb(0x1e1e2e)
                        };
                        items.push(
                            div()
                                .id(index)
                                .flex()
                                .items_center()
                                .gap_2()
                                .px_3()
                                .h(px(26.0))
                                .bg(row_bg)
                                .text_sm()
                                .cursor_pointer()
                                .on_click(click)
                                .child(
                                    div()
                                        .w(px(24.0))
                                        .text_xs()
                                        .text_color(if entry.is_dir {
                                            rgb(0x89b4fa)
                                        } else {
                                            rgb(0x6c7086)
                                        })
                                        .child(if entry.is_dir { "DIR" } else { "   " }),
                                )
                                .child(div().flex_1().overflow_hidden().child(entry.name.clone()))
                                .child(div().w(px(88.0)).text_xs().text_color(rgb(0x6c7086)).child(
                                    if entry.is_dir {
                                        "-".to_string()
                                    } else {
                                        human_size(entry.size)
                                    },
                                ))
                                .child(
                                    div()
                                        .w(px(96.0))
                                        .text_xs()
                                        .text_color(rgb(0x6c7086))
                                        .child(human_age(entry.modified)),
                                )
                                .into_any_element(),
                        );
                    }
                    items
                },
            ),
        )
        .h_full()
        .track_scroll(&self.scroll_handle);

        let list_container = div().flex_1().min_h_0().bg(rgb(0x1e1e2e)).child(list);

        let status = div()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .py_1()
            .bg(rgb(0x181825))
            .border_t_1()
            .border_color(rgb(0x313244))
            .text_xs()
            .text_color(rgb(0xa6adc8))
            .child(div().child(format!("{count} entries  |  {selected_name}")))
            .child(div().child(if self.loading {
                "loading…".to_string()
            } else {
                self.status.clone()
            }));

        div()
            .id("explorer-root")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.key_down(event, window, cx)
            }))
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x1e1e2e))
            .text_color(rgb(0xcdd6f4))
            .child(header)
            .child(list_container)
            .child(status)
    }
}

fn main() {
    let app = gpui_kit::application();
    app.run(|cx: &mut App| {
        // Documented gpui-kit initialization (gpui-component + gpui-base).
        gpui_kit::init(cx);

        cx.on_window_closed(|cx, _window_id| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(760.0), px(520.0)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("GPUI File Explorer PoC".into()),
                ..Default::default()
            }),
            focus: true,
            show: true,
            app_id: Some("gpui-file-explorer-poc".to_string()),
            ..Default::default()
        };

        gpui_kit::open_window(options, cx, |window, cx| {
            let view = cx.new(Explorer::new);
            let focus_handle = view.read(cx).focus_handle.clone();
            focus_handle.focus(window, cx);
            view
        })
        .expect("failed to open window");

        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    // NOTE: do NOT `use super::*;` here. The crate root glob-imports
    // `gpui_kit::*`, which re-exports GPUI's own `test` proc-macro; a glob
    // would shadow the built-in `#[test]` attribute and hit a recursion limit.
    use super::{human_age, human_size, read_entries};
    use std::fs;
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn scratch_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("gpui-poc-{tag}-{}", std::process::id()))
    }

    #[test]
    fn read_entries_lists_dirs_first_with_metadata() {
        let dir = scratch_dir("read");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("zeta_dir")).unwrap();
        fs::create_dir_all(dir.join("alpha_dir")).unwrap();
        fs::write(dir.join("beta.txt"), b"hello world").unwrap();
        fs::write(dir.join("Alpha.txt"), b"x").unwrap();

        let entries = read_entries(&dir).expect("directory should be readable");
        let names: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["alpha_dir", "zeta_dir", "Alpha.txt", "beta.txt"]
        );
        assert!(entries[0].is_dir && entries[1].is_dir);
        assert!(!entries[2].is_dir);
        assert_eq!(entries[3].size, 11);
        assert!(entries[3].modified.is_some());
        assert!(entries[3].path.ends_with("beta.txt"));

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn read_entries_reports_errors() {
        let missing = scratch_dir("missing");
        let _ = fs::remove_dir_all(&missing);
        assert!(read_entries(&missing).is_err());
    }

    #[test]
    fn human_size_units() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(1023), "1023 B");
        assert_eq!(human_size(1024), "1.0 K");
        assert_eq!(human_size(1536), "1.5 K");
        assert_eq!(human_size(1024 * 1024), "1.0 M");
        assert_eq!(human_size(3 * 1024 * 1024 * 1024), "3.0 G");
    }

    #[test]
    fn human_age_handles_missing_time() {
        assert_eq!(human_age(None), "?");
        assert!(human_age(Some(SystemTime::now())).ends_with("s ago"));
    }
}
