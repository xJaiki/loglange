pub mod cli;
pub mod folding;
pub mod indexer;
pub mod search;
pub mod timeline;
pub mod watcher;

use folding::{FoldingManager, FoldingStatus};
use indexer::{FileOpenResult, LogIndex, LogLine};
use parking_lot::RwLock;
use search::{SearchManager, SearchOptions, SearchResult};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tempfile::NamedTempFile;
use watcher::LogWatcherHandle;

pub struct TabSession {
    pub tab_id: String,
    pub path: String,
    pub file_name: String,
    pub index: Arc<RwLock<LogIndex>>,
    pub watcher: Option<LogWatcherHandle>,
    pub search_manager: Arc<SearchManager>,
    pub folding_manager: Arc<FoldingManager>,
}

pub struct AppState {
    pub tabs: Arc<RwLock<HashMap<String, TabSession>>>,
    pub active_tab_id: Arc<RwLock<Option<String>>>,
    pub tab_counter: Arc<AtomicU64>,
    pub initial_file: Arc<RwLock<Option<String>>>,
    pub stdin_temp: Arc<RwLock<Option<NamedTempFile>>>,
}

impl AppState {
    pub fn get_tab<T, F>(&self, tab_id: Option<String>, f: F) -> Result<T, String>
    where
        F: FnOnce(&TabSession) -> Result<T, String>,
    {
        let tabs = self.tabs.read();
        let target_id = match tab_id {
            Some(id) => id,
            None => self
                .active_tab_id
                .read()
                .clone()
                .ok_or_else(|| "No active tab".to_string())?,
        };
        let session = tabs
            .get(&target_id)
            .ok_or_else(|| format!("Tab '{}' not found", target_id))?;
        f(session)
    }
}

#[tauri::command]
fn open_file(
    app: tauri::AppHandle,
    path: String,
    tab_id: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<FileOpenResult, String> {
    let (index, duration_ms) = LogIndex::open(&path).map_err(|e| e.to_string())?;
    let file_name = Path::new(&path)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.clone());
    let total_lines = index.total_lines();
    let file_size = index.file_size();

    let search_manager = Arc::new(SearchManager::new());
    let folding_manager = Arc::new(FoldingManager::new());
    folding_manager.scan_index(&index);
    let total_display_rows = folding_manager.total_display_rows();

    let id = tab_id.unwrap_or_else(|| {
        let n = state.tab_counter.fetch_add(1, Ordering::SeqCst);
        format!("tab-{}", n)
    });

    let index_arc = Arc::new(RwLock::new(index));

    let watcher_handle = match watcher::start_watching(
        app,
        id.clone(),
        path.clone(),
        index_arc.clone(),
        search_manager.clone(),
        folding_manager.clone(),
    ) {
        Ok(handle) => Some(handle),
        Err(e) => {
            eprintln!("Failed to start watcher for {}: {}", path, e);
            None
        }
    };

    let session = TabSession {
        tab_id: id.clone(),
        path: path.clone(),
        file_name: file_name.clone(),
        index: index_arc,
        watcher: watcher_handle,
        search_manager,
        folding_manager,
    };

    state.tabs.write().insert(id.clone(), session);
    *state.active_tab_id.write() = Some(id.clone());

    Ok(FileOpenResult {
        tab_id: id,
        path,
        file_name,
        file_size,
        total_lines,
        total_display_rows,
        index_time_ms: duration_ms,
    })
}

#[tauri::command]
fn close_tab(tab_id: String, state: tauri::State<'_, AppState>) -> Result<Option<String>, String> {
    let mut tabs = state.tabs.write();
    if tabs.remove(&tab_id).is_some() {
        let mut active = state.active_tab_id.write();
        if *active == Some(tab_id) {
            *active = tabs.keys().next().cloned();
        }
        Ok(active.clone())
    } else {
        Err(format!("Tab '{}' not found", tab_id))
    }
}

#[tauri::command]
fn switch_tab(tab_id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let tabs = state.tabs.read();
    if tabs.contains_key(&tab_id) {
        *state.active_tab_id.write() = Some(tab_id);
        Ok(())
    } else {
        Err(format!("Tab '{}' not found", tab_id))
    }
}

#[tauri::command]
fn get_lines_slice(
    tab_id: Option<String>,
    start_line: usize,
    count: usize,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<LogLine>, String> {
    state.get_tab(tab_id, |session| {
        let guard = session.index.read();
        if session.folding_manager.get_status().auto_collapse_enabled {
            Ok(session.folding_manager.get_display_slice(&guard, start_line, count))
        } else {
            Ok(guard.get_lines_slice(start_line, count))
        }
    })
}

#[tauri::command]
fn get_total_lines(tab_id: Option<String>, state: tauri::State<'_, AppState>) -> Result<usize, String> {
    state.get_tab(tab_id, |session| Ok(session.index.read().total_lines()))
}

#[tauri::command]
fn get_display_row_count(
    tab_id: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<usize, String> {
    state.get_tab(tab_id, |session| Ok(session.folding_manager.total_display_rows()))
}

#[tauri::command]
fn get_folding_status(
    tab_id: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<FoldingStatus, String> {
    state.get_tab(tab_id, |session| Ok(session.folding_manager.get_status()))
}

#[tauri::command]
fn toggle_stacktrace_group(
    tab_id: Option<String>,
    group_id: usize,
    state: tauri::State<'_, AppState>,
) -> Result<(bool, usize), String> {
    state.get_tab(tab_id, |session| {
        if let Some(expanded) = session.folding_manager.toggle_group(group_id) {
            let total_rows = session.folding_manager.total_display_rows();
            Ok((expanded, total_rows))
        } else {
            Err(format!("Stacktrace group {} not found", group_id))
        }
    })
}

#[tauri::command]
fn set_auto_collapse_stacktraces(
    tab_id: Option<String>,
    enabled: bool,
    state: tauri::State<'_, AppState>,
) -> Result<usize, String> {
    state.get_tab(tab_id, |session| {
        session.folding_manager.set_auto_collapse(enabled);
        Ok(session.folding_manager.total_display_rows())
    })
}

#[tauri::command]
fn expand_all_stacktraces(
    tab_id: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<usize, String> {
    state.get_tab(tab_id, |session| {
        session.folding_manager.expand_all();
        Ok(session.folding_manager.total_display_rows())
    })
}

#[tauri::command]
fn collapse_all_stacktraces(
    tab_id: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<usize, String> {
    state.get_tab(tab_id, |session| {
        session.folding_manager.collapse_all();
        Ok(session.folding_manager.total_display_rows())
    })
}

#[tauri::command]
fn stop_watching(tab_id: Option<String>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut tabs = state.tabs.write();
    let target_id = match tab_id {
        Some(id) => id,
        None => state
            .active_tab_id
            .read()
            .clone()
            .ok_or_else(|| "No active tab".to_string())?,
    };
    if let Some(session) = tabs.get_mut(&target_id) {
        session.watcher = None;
    }
    Ok(())
}

#[tauri::command]
fn search_logs(
    tab_id: Option<String>,
    options: SearchOptions,
    state: tauri::State<'_, AppState>,
) -> Result<SearchResult, String> {
    state.get_tab(tab_id, |session| {
        let guard = session.index.read();
        session.search_manager.search(&guard, options)
    })
}

#[tauri::command]
fn get_filtered_lines_slice(
    tab_id: Option<String>,
    start_row: usize,
    count: usize,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<LogLine>, String> {
    state.get_tab(tab_id, |session| {
        let guard = session.index.read();
        Ok(session
            .search_manager
            .get_filtered_lines_slice(&guard, start_row, count))
    })
}

#[tauri::command]
fn get_next_match(
    tab_id: Option<String>,
    current_line: usize,
    state: tauri::State<'_, AppState>,
) -> Result<Option<usize>, String> {
    state.get_tab(tab_id, |session| Ok(session.search_manager.next_match(current_line)))
}

#[tauri::command]
fn get_prev_match(
    tab_id: Option<String>,
    current_line: usize,
    state: tauri::State<'_, AppState>,
) -> Result<Option<usize>, String> {
    state.get_tab(tab_id, |session| Ok(session.search_manager.prev_match(current_line)))
}

#[tauri::command]
fn clear_search(tab_id: Option<String>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.get_tab(tab_id, |session| {
        session.search_manager.clear();
        Ok(())
    })
}

#[tauri::command]
fn get_initial_file(state: tauri::State<'_, AppState>) -> Result<Option<String>, String> {
    Ok(state.initial_file.read().clone())
}

#[tauri::command]
fn find_timestamp_line(
    tab_id: Option<String>,
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<Option<usize>, String> {
    state.get_tab(tab_id, |session| {
        let guard = session.index.read();
        Ok(guard.find_timestamp_line(&query))
    })
}

#[tauri::command]
fn map_line_to_display_row(
    tab_id: Option<String>,
    line_number: usize,
    state: tauri::State<'_, AppState>,
) -> Result<usize, String> {
    state.get_tab(tab_id, |session| {
        Ok(session.folding_manager.line_to_display_row(line_number))
    })
}

#[tauri::command]
fn export_filtered_lines(
    tab_id: Option<String>,
    dest_path: String,
    state: tauri::State<'_, AppState>,
) -> Result<usize, String> {
    state.get_tab(tab_id, |session| {
        let guard = session.index.read();
        session
            .search_manager
            .export_filtered_to_file(&guard, Path::new(&dest_path))
    })
}

#[tauri::command]
fn get_filtered_content(
    tab_id: Option<String>,
    max_lines: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    state.get_tab(tab_id, |session| {
        let guard = session.index.read();
        session
            .search_manager
            .get_all_filtered_content(&guard, max_lines.unwrap_or(10_000))
    })
}

#[tauri::command]
fn get_lines_content(
    tab_id: Option<String>,
    line_numbers: Vec<usize>,
    with_line_numbers: Option<bool>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    state.get_tab(tab_id, |session| {
        let guard = session.index.read();
        let with_num = with_line_numbers.unwrap_or(false);
        // Safety cap: up to 50,000 lines
        let capped: Vec<usize> = line_numbers.into_iter().take(50_000).collect();
        let lines = guard.get_lines_by_numbers(&capped);

        if with_num {
            let mut output = String::with_capacity(lines.iter().map(|l| l.len() + 16).sum());
            for (i, line) in lines.iter().enumerate() {
                if let Some(&num) = capped.get(i) {
                    output.push_str(&format!("{}: {}\n", num, line));
                } else {
                    output.push_str(line);
                    output.push('\n');
                }
            }
            if output.ends_with('\n') {
                output.pop();
            }
            Ok(output)
        } else {
            Ok(lines.join("\n"))
        }
    })
}

#[tauri::command]
fn get_time_histogram(
    tab_id: Option<String>,
    bucket_count: Option<usize>,
    filtered: Option<bool>,
    state: tauri::State<'_, AppState>,
) -> Result<timeline::TimeHistogram, String> {
    state.get_tab(tab_id, |session| {
        let index_guard = session.index.read();
        let target_buckets = bucket_count.unwrap_or(75);

        if filtered == Some(true) {
            let hist = session.search_manager.with_filtered_indices(|indices| {
                timeline::build_histogram(&index_guard, Some(indices), target_buckets)
            });
            match hist {
                Some(h) => Ok(h),
                None => Ok(timeline::build_histogram(&index_guard, None, target_buckets)),
            }
        } else {
            Ok(timeline::build_histogram(&index_guard, None, target_buckets))
        }
    })
}

#[tauri::command]
fn get_severity_heatmap(
    tab_id: Option<String>,
    num_bins: Option<usize>,
    custom_error_keywords: Option<Vec<String>>,
    custom_warn_keywords: Option<Vec<String>>,
    state: tauri::State<'_, AppState>,
) -> Result<timeline::SeverityHeatmap, String> {
    state.get_tab(tab_id, |session| {
        let index_guard = session.index.read();
        let target_bins = num_bins.unwrap_or(250);
        Ok(timeline::build_severity_heatmap(
            &index_guard,
            target_bins,
            custom_error_keywords.as_deref(),
            custom_warn_keywords.as_deref(),
        ))
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }

    let cli_init = cli::process_cli_args();

    let state = AppState {
        tabs: Arc::new(RwLock::new(HashMap::new())),
        active_tab_id: Arc::new(RwLock::new(None)),
        tab_counter: Arc::new(AtomicU64::new(1)),
        initial_file: Arc::new(RwLock::new(cli_init.initial_file)),
        stdin_temp: Arc::new(RwLock::new(cli_init.stdin_temp)),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            open_file,
            close_tab,
            switch_tab,
            get_lines_slice,
            get_total_lines,
            get_display_row_count,
            get_folding_status,
            toggle_stacktrace_group,
            set_auto_collapse_stacktraces,
            expand_all_stacktraces,
            collapse_all_stacktraces,
            stop_watching,
            search_logs,
            get_filtered_lines_slice,
            get_next_match,
            get_prev_match,
            clear_search,
            get_initial_file,
            find_timestamp_line,
            map_line_to_display_row,
            export_filtered_lines,
            get_filtered_content,
            get_lines_content,
            get_time_histogram,
            get_severity_heatmap
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_tab_session_management() {
        let mut f1 = NamedTempFile::new().unwrap();
        writeln!(f1, "file1 line 1\nfile1 line 2").unwrap();
        f1.flush().unwrap();

        let mut f2 = NamedTempFile::new().unwrap();
        writeln!(f2, "file2 alpha\nfile2 beta\nfile2 gamma").unwrap();
        f2.flush().unwrap();

        let state = AppState {
            tabs: Arc::new(RwLock::new(HashMap::new())),
            active_tab_id: Arc::new(RwLock::new(None)),
            tab_counter: Arc::new(AtomicU64::new(1)),
            initial_file: Arc::new(RwLock::new(None)),
            stdin_temp: Arc::new(RwLock::new(None)),
        };

        let (idx1, _) = LogIndex::open(f1.path()).unwrap();
        let (idx2, _) = LogIndex::open(f2.path()).unwrap();

        let session1 = TabSession {
            tab_id: "tab-1".to_string(),
            path: f1.path().to_string_lossy().to_string(),
            file_name: "file1.log".to_string(),
            index: Arc::new(RwLock::new(idx1)),
            watcher: None,
            search_manager: Arc::new(SearchManager::new()),
            folding_manager: Arc::new(FoldingManager::new()),
        };

        let session2 = TabSession {
            tab_id: "tab-2".to_string(),
            path: f2.path().to_string_lossy().to_string(),
            file_name: "file2.log".to_string(),
            index: Arc::new(RwLock::new(idx2)),
            watcher: None,
            search_manager: Arc::new(SearchManager::new()),
            folding_manager: Arc::new(FoldingManager::new()),
        };

        state.tabs.write().insert("tab-1".to_string(), session1);
        state.tabs.write().insert("tab-2".to_string(), session2);
        *state.active_tab_id.write() = Some("tab-1".to_string());

        // Verify active tab reading
        let count1 = state
            .get_tab(None, |s| Ok(s.index.read().total_lines()))
            .unwrap();
        assert_eq!(count1, 2);

        // Verify explicit tab reading
        let count2 = state
            .get_tab(Some("tab-2".to_string()), |s| {
                Ok(s.index.read().total_lines())
            })
            .unwrap();
        assert_eq!(count2, 3);

        // Switch active tab
        *state.active_tab_id.write() = Some("tab-2".to_string());
        let count_active = state
            .get_tab(None, |s| Ok(s.index.read().total_lines()))
            .unwrap();
        assert_eq!(count_active, 3);

        // Close tab 1
        state.tabs.write().remove("tab-1");
        let err = state.get_tab(Some("tab-1".to_string()), |s| {
            Ok(s.index.read().total_lines())
        });
        assert!(err.is_err());
        assert_eq!(state.tabs.read().len(), 1);
    }
}
