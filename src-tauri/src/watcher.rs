use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::RwLock;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::indexer::LogIndex;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LogAppendedPayload {
    pub tab_id: String,
    pub total_lines: usize,
    pub file_size: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FilterUpdatedPayload {
    pub tab_id: String,
    pub total_matches: usize,
}

pub struct LogWatcherHandle {
    stop_signal: Arc<AtomicBool>,
    _watcher: RecommendedWatcher,
    thread_handle: Option<JoinHandle<()>>,
}

impl Drop for LogWatcherHandle {
    fn drop(&mut self) {
        self.stop_signal.store(true, Ordering::SeqCst);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

/// Internal watcher loop supporting arbitrary callbacks for testability and decoupling.
pub fn start_watching_internal<F, G>(
    tab_id: String,
    path: PathBuf,
    index_state: Arc<RwLock<LogIndex>>,
    search_manager: Option<Arc<crate::search::SearchManager>>,
    folding_manager: Option<Arc<crate::folding::FoldingManager>>,
    append_callback: F,
    filter_callback: G,
) -> std::io::Result<LogWatcherHandle>
where
    F: Fn(LogAppendedPayload) + Send + 'static,
    G: Fn(FilterUpdatedPayload) + Send + 'static,
{
    let (tx, rx) = channel();

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(_event) = res {
                let _ = tx.send(());
            }
        },
        Config::default(),
    )
    .map_err(|e| std::io::Error::other(e.to_string()))?;

    watcher
        .watch(&path, RecursiveMode::NonRecursive)
        .map_err(|e| std::io::Error::other(e.to_string()))?;

    if let Some(parent) = path.parent() {
        if parent.exists() {
            let _ = watcher.watch(parent, RecursiveMode::NonRecursive);
        }
    }

    let stop_signal = Arc::new(AtomicBool::new(false));
    let stop_signal_clone = stop_signal.clone();
    let thread_path = path;
    let tab_id_clone = tab_id;

    let thread_handle = thread::Builder::new()
        .name("log-watcher-thread".to_string())
        .spawn(move || {
            let mut last_known_size: u64 = {
                let guard = index_state.read();
                guard.file_size()
            };

            while !stop_signal_clone.load(Ordering::SeqCst) {
                // Wait for a file notification, or check every 100ms as fallback
                match rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(()) => {
                        // Rapid burst detected. Wait 16ms to coalesce consecutive writes into a single frame
                        thread::sleep(Duration::from_millis(16));
                        while rx.try_recv().is_ok() {}
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        // Heartbeat
                    }
                    Err(RecvTimeoutError::Disconnected) => {
                        break;
                    }
                }

                if stop_signal_clone.load(Ordering::SeqCst) {
                    break;
                }

                // Check file metadata on disk
                if let Ok(metadata) = std::fs::metadata(&thread_path) {
                    let current_size = metadata.len();
                    if current_size != last_known_size {
                        let mut idx = index_state.write();
                        let old_lines = idx.total_lines();
                        if let Ok(added) = idx.append_from_disk() {
                            let total_lines = idx.total_lines();
                            let file_size = idx.file_size();
                            last_known_size = file_size;

                            let mut updated_matches = None;
                            if added > 0 {
                                if let Some(ref sm) = search_manager {
                                    updated_matches = sm.append_lines(&idx, old_lines, total_lines);
                                }
                                if let Some(ref fm) = folding_manager {
                                    fm.append_lines(&idx, old_lines, total_lines);
                                }
                            }

                            // Release lock before calling callbacks
                            drop(idx);

                            append_callback(LogAppendedPayload {
                                tab_id: tab_id_clone.clone(),
                                total_lines,
                                file_size,
                            });

                            if let Some(matches) = updated_matches {
                                filter_callback(FilterUpdatedPayload {
                                    tab_id: tab_id_clone.clone(),
                                    total_matches: matches,
                                });
                            }
                        }
                    }
                }
            }
        })
        .map_err(|e| std::io::Error::other(e.to_string()))?;

    Ok(LogWatcherHandle {
        stop_signal,
        _watcher: watcher,
        thread_handle: Some(thread_handle),
    })
}

/// Start watching a log file for real-time appends and modifications.
/// Emits `log:appended` events throttled to at most once per 16ms (60/120Hz frame pacing).
pub fn start_watching(
    app_handle: AppHandle,
    tab_id: String,
    path_str: String,
    index_state: Arc<RwLock<LogIndex>>,
    search_manager: Arc<crate::search::SearchManager>,
    folding_manager: Arc<crate::folding::FoldingManager>,
) -> std::io::Result<LogWatcherHandle> {
    let path = PathBuf::from(&path_str);
    let app_clone = app_handle.clone();
    start_watching_internal(
        tab_id,
        path,
        index_state,
        Some(search_manager),
        Some(folding_manager),
        move |payload| {
            let _ = app_handle.emit("log:appended", payload);
        },
        move |payload| {
            let _ = app_clone.emit("log:filter_updated", payload);
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::mpsc::channel;
    use tempfile::NamedTempFile;

    #[test]
    fn test_watcher_detects_appends_and_throttles() {
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(temp, "Initial line 1").unwrap();
        writeln!(temp, "Initial line 2").unwrap();
        temp.flush().unwrap();

        let (index, _) = LogIndex::open(temp.path()).unwrap();
        assert_eq!(index.total_lines(), 2);

        let index_state = Arc::new(RwLock::new(index));
        let (event_tx, event_rx) = channel();

        let _handle = start_watching_internal(
            "test_tab".to_string(),
            temp.path().to_path_buf(),
            index_state.clone(),
            None,
            None,
            move |payload| {
                let _ = event_tx.send(payload);
            },
            |_| {},
        )
        .unwrap();

        // Write several rapid lines
        for i in 3..=10 {
            writeln!(temp, "Streaming line {}", i).unwrap();
        }
        temp.flush().unwrap();

        // Wait for payload from watcher
        let payload = event_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("Should receive log:appended payload within 2s");

        assert_eq!(payload.tab_id, "test_tab");
        assert_eq!(payload.total_lines, 10);
        assert_eq!(index_state.read().total_lines(), 10);

        // Verify content
        let guard = index_state.read();
        let slice = guard.get_lines_slice(0, 10);
        assert_eq!(slice.len(), 10);
        assert_eq!(slice[9].content, "Streaming line 10");
    }
}
