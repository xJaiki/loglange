use memchr::memmem;
use parking_lot::RwLock;
use rayon::prelude::*;
use regex::bytes::RegexBuilder;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crate::indexer::{LogIndex, LogLine};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchOptions {
    pub query: String,
    pub is_regex: bool,
    pub case_sensitive: bool,
    pub invert: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub query_id: u64,
    pub total_matches: usize,
    pub total_lines: usize,
    pub search_time_ms: f64,
    pub match_lines: Vec<u32>, // 1-based line numbers for minimap and quick navigation (sampled if large)
    pub error: Option<String>,
}

#[derive(Clone)]
enum Matcher {
    LiteralSensitive(Vec<u8>),
    Regex(regex::bytes::Regex),
}

impl Matcher {
    fn is_match(&self, haystack: &[u8]) -> bool {
        match self {
            Matcher::LiteralSensitive(needle) => {
                let finder = memmem::Finder::new(needle);
                finder.find(haystack).is_some()
            }
            Matcher::Regex(re) => re.is_match(haystack),
        }
    }
}

pub struct ActiveSearch {
    pub query_id: u64,
    pub options: SearchOptions,
    pub filtered_indices: Vec<u32>, // 0-based indices into LogIndex.offsets
    matcher: Option<Matcher>,
}

pub struct SearchManager {
    current_query_id: AtomicU64,
    cancel_token: RwLock<Option<Arc<AtomicBool>>>,
    active_search: RwLock<Option<ActiveSearch>>,
}

impl Default for SearchManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchManager {
    pub fn new() -> Self {
        Self {
            current_query_id: AtomicU64::new(0),
            cancel_token: RwLock::new(None),
            active_search: RwLock::new(None),
        }
    }

    /// Cancel any currently active search immediately.
    pub fn cancel_active(&self) {
        let guard = self.cancel_token.read();
        if let Some(ref token) = *guard {
            token.store(true, Ordering::Relaxed);
        }
    }

    /// Clear the active search results.
    pub fn clear(&self) {
        self.cancel_active();
        *self.active_search.write() = None;
    }

    /// Check if a filter is currently active.
    pub fn has_active_search(&self) -> bool {
        self.active_search.read().is_some()
    }

    /// Get total filtered matches currently stored.
    pub fn total_matches(&self) -> usize {
        let guard = self.active_search.read();
        guard.as_ref().map(|s| s.filtered_indices.len()).unwrap_or(0)
    }

    /// Execute a closure borrowing the active filtered line indices without cloning.
    pub fn with_filtered_indices<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&[u32]) -> R,
    {
        let guard = self.active_search.read();
        guard.as_ref().map(|s| f(&s.filtered_indices))
    }

    /// Execute search over the LogIndex in background parallel threads using SIMD.
    /// Can be cancelled at any time by calling `cancel_active()` or starting a new search.
    pub fn search(&self, index: &LogIndex, options: SearchOptions) -> Result<SearchResult, String> {
        let start_time = Instant::now();
        let query_id = self.current_query_id.fetch_add(1, Ordering::SeqCst) + 1;

        // Cancel any previous search
        self.cancel_active();

        // Create new cancellation token for this search
        let cancel_flag = Arc::new(AtomicBool::new(false));
        *self.cancel_token.write() = Some(cancel_flag.clone());

        let total_lines = index.total_lines();

        // If query is empty, clear active search and return empty result
        if options.query.trim().is_empty() {
            *self.active_search.write() = None;
            return Ok(SearchResult {
                query_id,
                total_matches: 0,
                total_lines,
                search_time_ms: start_time.elapsed().as_secs_f64() * 1000.0,
                match_lines: Vec::new(),
                error: None,
            });
        }

        // Compile matcher
        let matcher = if options.is_regex {
            let re = RegexBuilder::new(&options.query)
                .case_insensitive(!options.case_sensitive)
                .build()
                .map_err(|e| format!("Invalid regex: {}", e))?;
            Matcher::Regex(re)
        } else if options.case_sensitive {
            Matcher::LiteralSensitive(options.query.as_bytes().to_vec())
        } else {
            // Case-insensitive literal search via regex byte compiler (uses Teddy / SIMD)
            let escaped = regex::escape(&options.query);
            let re = RegexBuilder::new(&escaped)
                .case_insensitive(true)
                .build()
                .map_err(|e| format!("Failed to build literal matcher: {}", e))?;
            Matcher::Regex(re)
        };

        if total_lines == 0 {
            *self.active_search.write() = Some(ActiveSearch {
                query_id,
                options: options.clone(),
                filtered_indices: Vec::new(),
                matcher: Some(matcher),
            });
            return Ok(SearchResult {
                query_id,
                total_matches: 0,
                total_lines: 0,
                search_time_ms: start_time.elapsed().as_secs_f64() * 1000.0,
                match_lines: Vec::new(),
                error: None,
            });
        }

        // Execute parallel scan with Rayon
        // Process in chunks of 4096 lines
        const CHUNK_SIZE: usize = 4096;
        let chunk_ranges: Vec<(usize, usize)> = (0..total_lines)
            .step_by(CHUNK_SIZE)
            .map(|start| (start, (start + CHUNK_SIZE).min(total_lines)))
            .collect();

        let invert = options.invert;

        let chunk_results: Result<Vec<Vec<u32>>, ()> = chunk_ranges
            .into_par_iter()
            .map(|(start_idx, end_idx)| {
                if cancel_flag.load(Ordering::Relaxed) {
                    return Err(());
                }

                let mut local_matches = Vec::new();

                for line_idx in start_idx..end_idx {
                    // Periodic cancellation check every 512 lines
                    if (line_idx & 511) == 0 && cancel_flag.load(Ordering::Relaxed) {
                        return Err(());
                    }

                    if let Some(line_bytes) = index.get_line_raw(line_idx) {
                        let mut raw = line_bytes;
                        if raw.ends_with(b"\n") {
                            raw = &raw[..raw.len() - 1];
                        }
                        if raw.ends_with(b"\r") {
                            raw = &raw[..raw.len() - 1];
                        }

                        let is_match = matcher.is_match(raw);
                        let matched = if invert { !is_match } else { is_match };

                        if matched {
                            local_matches.push(line_idx as u32);
                        }
                    }
                }

                Ok(local_matches)
            })
            .collect();

        // Check if search was cancelled while processing
        let chunk_results = match chunk_results {
            Ok(res) => res,
            Err(_) => return Err("Search cancelled".to_string()),
        };

        if cancel_flag.load(Ordering::Relaxed) {
            return Err("Search cancelled".to_string());
        }

        let total_matches: usize = chunk_results.iter().map(|c| c.len()).sum();
        let mut filtered_indices = Vec::with_capacity(total_matches);
        for chunk in chunk_results {
            filtered_indices.extend(chunk);
        }

        // Build sample match lines for minimap (max 10,000 items to avoid IPC overhead)
        let match_lines: Vec<u32> = if filtered_indices.len() <= 10_000 {
            filtered_indices.iter().map(|&idx| idx + 1).collect()
        } else {
            // Downsample evenly
            let step = filtered_indices.len() as f64 / 10_000.0;
            (0..10_000)
                .map(|i| {
                    let idx = (i as f64 * step) as usize;
                    filtered_indices[idx] + 1
                })
                .collect()
        };

        let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;

        // Store active search results
        *self.active_search.write() = Some(ActiveSearch {
            query_id,
            options,
            filtered_indices,
            matcher: Some(matcher),
        });

        Ok(SearchResult {
            query_id,
            total_matches,
            total_lines,
            search_time_ms: duration_ms,
            match_lines,
            error: None,
        })
    }

    /// Incrementally tests newly appended lines from `start_line` to `end_line` against the active search query.
    /// Returns the updated total matches, or None if no active search is running.
    pub fn append_lines(&self, index: &LogIndex, start_line: usize, end_line: usize) -> Option<usize> {
        let mut guard = self.active_search.write();
        let search = guard.as_mut()?;
        let matcher = search.matcher.as_ref()?;
        let invert = search.options.invert;

        for line_idx in start_line..end_line {
            if let Some(line_bytes) = index.get_line_raw(line_idx) {
                let mut raw = line_bytes;
                if raw.ends_with(b"\n") {
                    raw = &raw[..raw.len() - 1];
                }
                if raw.ends_with(b"\r") {
                    raw = &raw[..raw.len() - 1];
                }

                let is_match = matcher.is_match(raw);
                let matched = if invert { !is_match } else { is_match };

                if matched {
                    search.filtered_indices.push(line_idx as u32);
                }
            }
        }

        Some(search.filtered_indices.len())
    }

    /// Retrieve a slice of matching lines for virtual scrolling in Filter Mode.
    pub fn get_filtered_lines_slice(
        &self,
        index: &LogIndex,
        start_row: usize,
        count: usize,
    ) -> Vec<LogLine> {
        let guard = self.active_search.read();
        let Some(search) = guard.as_ref() else {
            return Vec::new();
        };

        if search.filtered_indices.is_empty() || start_row >= search.filtered_indices.len() || count == 0 {
            return Vec::new();
        }

        let end_row = (start_row + count).min(search.filtered_indices.len());
        let mut lines = Vec::with_capacity(end_row - start_row);

        for row in start_row..end_row {
            let line_idx = search.filtered_indices[row] as usize;
            if let Some(raw) = index.get_line_raw(line_idx) {
                let mut content_slice = raw;
                if content_slice.ends_with(b"\n") {
                    content_slice = &content_slice[..content_slice.len() - 1];
                }
                if content_slice.ends_with(b"\r") {
                    content_slice = &content_slice[..content_slice.len() - 1];
                }

                let byte_offset = index.get_line_offset(line_idx).unwrap_or(0);
                lines.push(LogLine {
                    line_number: line_idx + 1, // original 1-based line number!
                    content: String::from_utf8_lossy(content_slice).into_owned(),
                    byte_offset,
                    is_stacktrace: false,
                    is_collapsed_group: false,
                    group_id: None,
                    group_line_count: None,
                });
            }
        }

        lines
    }

    /// Find the next matching line number after `current_line` (1-based).
    pub fn next_match(&self, current_line: usize) -> Option<usize> {
        let guard = self.active_search.read();
        let search = guard.as_ref()?;
        if search.filtered_indices.is_empty() {
            return None;
        }

        let target_idx = current_line as u32; // index = line - 1, so next match is >= current_line
        let pos = search.filtered_indices.partition_point(|&idx| idx < target_idx);
        if pos < search.filtered_indices.len() {
            Some(search.filtered_indices[pos] as usize + 1)
        } else {
            // Wrap around to first match
            Some(search.filtered_indices[0] as usize + 1)
        }
    }

    /// Find the previous matching line number before `current_line` (1-based).
    pub fn prev_match(&self, current_line: usize) -> Option<usize> {
        let guard = self.active_search.read();
        let search = guard.as_ref()?;
        if search.filtered_indices.is_empty() {
            return None;
        }

        let target_idx = (current_line.saturating_sub(1)) as u32;
        let pos = search.filtered_indices.partition_point(|&idx| idx < target_idx);
        if pos > 0 {
            Some(search.filtered_indices[pos - 1] as usize + 1)
        } else {
            // Wrap around to last match
            Some(*search.filtered_indices.last()? as usize + 1)
        }
    }

    /// Export all matching filtered lines to a specified file path.
    pub fn export_filtered_to_file(&self, index: &LogIndex, dest_path: &std::path::Path) -> Result<usize, String> {
        use std::fs::File;
        use std::io::{BufWriter, Write};

        let guard = self.active_search.read();
        let Some(search) = guard.as_ref() else {
            return Err("No active search filter to export".to_string());
        };

        if search.filtered_indices.is_empty() {
            return Ok(0);
        }

        let file = File::create(dest_path).map_err(|e| format!("Failed to create destination file: {}", e))?;
        let mut writer = BufWriter::with_capacity(256 * 1024, file);

        let mut count = 0;
        for &line_idx in &search.filtered_indices {
            if let Some(raw) = index.get_line_raw(line_idx as usize) {
                writer.write_all(raw).map_err(|e| format!("Write error: {}", e))?;
                if !raw.ends_with(b"\n") {
                    writer.write_all(b"\n").map_err(|e| format!("Write error: {}", e))?;
                }
                count += 1;
            }
        }
        writer.flush().map_err(|e| format!("Flush error: {}", e))?;
        Ok(count)
    }

    /// Retrieve all filtered line contents as a single string (capped at max_lines for memory safety).
    pub fn get_all_filtered_content(&self, index: &LogIndex, max_lines: usize) -> Result<String, String> {
        let guard = self.active_search.read();
        let Some(search) = guard.as_ref() else {
            return Err("No active search filter".to_string());
        };

        let total = search.filtered_indices.len().min(max_lines);
        let mut out = String::with_capacity(total * 100);

        for i in 0..total {
            let line_idx = search.filtered_indices[i] as usize;
            if let Some(raw) = index.get_line_raw(line_idx) {
                let mut content_slice = raw;
                if content_slice.ends_with(b"\n") {
                    content_slice = &content_slice[..content_slice.len() - 1];
                }
                if content_slice.ends_with(b"\r") {
                    content_slice = &content_slice[..content_slice.len() - 1];
                }
                out.push_str(&String::from_utf8_lossy(content_slice));
                out.push('\n');
            }
        }

        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_file(content: &str) -> (NamedTempFile, LogIndex) {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file.flush().unwrap();
        let (index, _) = LogIndex::open(file.path()).unwrap();
        (file, index)
    }

    #[test]
    fn test_substring_search_case_sensitive() {
        let log = "2026-10-27 INFO Application started\n2026-10-27 ERROR Connection refused\n2026-10-27 error ignored\n2026-10-27 DEBUG Heartbeat ok\n";
        let (_file, index) = create_test_file(log);

        let manager = SearchManager::new();
        let res = manager
            .search(
                &index,
                SearchOptions {
                    query: "ERROR".to_string(),
                    is_regex: false,
                    case_sensitive: true,
                    invert: false,
                },
            )
            .unwrap();

        assert_eq!(res.total_matches, 1);
        assert_eq!(res.match_lines, vec![2]);

        let slice = manager.get_filtered_lines_slice(&index, 0, 10);
        assert_eq!(slice.len(), 1);
        assert_eq!(slice[0].line_number, 2);
        assert_eq!(slice[0].content, "2026-10-27 ERROR Connection refused");
    }

    #[test]
    fn test_substring_search_case_insensitive() {
        let log = "2026-10-27 INFO Application started\n2026-10-27 ERROR Connection refused\n2026-10-27 error ignored\n2026-10-27 DEBUG Heartbeat ok\n";
        let (_file, index) = create_test_file(log);

        let manager = SearchManager::new();
        let res = manager
            .search(
                &index,
                SearchOptions {
                    query: "error".to_string(),
                    is_regex: false,
                    case_sensitive: false,
                    invert: false,
                },
            )
            .unwrap();

        assert_eq!(res.total_matches, 2);
        assert_eq!(res.match_lines, vec![2, 3]);
    }

    #[test]
    fn test_regex_search() {
        let log = "user_id=123 status=200\nuser_id=456 status=500\nuser_id=789 status=404\n";
        let (_file, index) = create_test_file(log);

        let manager = SearchManager::new();
        let res = manager
            .search(
                &index,
                SearchOptions {
                    query: r"status=[45]\d{2}".to_string(),
                    is_regex: true,
                    case_sensitive: true,
                    invert: false,
                },
            )
            .unwrap();

        assert_eq!(res.total_matches, 2);
        assert_eq!(res.match_lines, vec![2, 3]);
    }

    #[test]
    fn test_custom_log_levels_regex_search() {
        let log = "2026-10-28 [INFO] avvio\n2026-10-28 [ATTENZIONE] query lenta\n2026-10-28 [ERRORE] connessione persa\n2026-10-28 [AVVISO] disco quasi pieno\n";
        let (_file, index) = create_test_file(log);

        let manager = SearchManager::new();
        // Query generata dinamicamente dal frontend per i Warning personalizzati
        let res = manager
            .search(
                &index,
                SearchOptions {
                    query: r"\b(ATTENZIONE|WARNING|AVVISO|WARN)\b".to_string(),
                    is_regex: true,
                    case_sensitive: false,
                    invert: false,
                },
            )
            .unwrap();

        assert_eq!(res.total_matches, 2);
        assert_eq!(res.match_lines, vec![2, 4]);
    }

    #[test]
    fn test_invalid_regex_error() {
        let log = "hello world\n";
        let (_file, index) = create_test_file(log);

        let manager = SearchManager::new();
        let res = manager.search(
            &index,
            SearchOptions {
                query: "[invalid".to_string(),
                is_regex: true,
                case_sensitive: true,
                invert: false,
            },
        );

        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Invalid regex"));
    }

    #[test]
    fn test_next_and_prev_navigation() {
        let log = "line 1\nmatch 1\nline 3\nmatch 2\nline 5\nmatch 3\n";
        let (_file, index) = create_test_file(log);

        let manager = SearchManager::new();
        manager
            .search(
                &index,
                SearchOptions {
                    query: "match".to_string(),
                    is_regex: false,
                    case_sensitive: true,
                    invert: false,
                },
            )
            .unwrap();

        // matches at line 2, 4, 6
        assert_eq!(manager.next_match(1), Some(2));
        assert_eq!(manager.next_match(2), Some(4));
        assert_eq!(manager.next_match(4), Some(6));
        // wrap around
        assert_eq!(manager.next_match(6), Some(2));

        assert_eq!(manager.prev_match(6), Some(4));
        assert_eq!(manager.prev_match(4), Some(2));
        // wrap around
        assert_eq!(manager.prev_match(2), Some(6));
    }

    #[test]
    fn test_incremental_append_search() {
        let log = "first line\n";
        let (mut file, mut index) = create_test_file(log);

        let manager = SearchManager::new();
        manager
            .search(
                &index,
                SearchOptions {
                    query: "append".to_string(),
                    is_regex: false,
                    case_sensitive: true,
                    invert: false,
                },
            )
            .unwrap();

        assert_eq!(manager.total_matches(), 0);

        // Append lines to file and update index
        file.write_all(b"append 1\nnormal line\nappend 2\n").unwrap();
        file.flush().unwrap();
        let old_total = index.total_lines();
        let added = index.append_from_disk().unwrap();
        let new_total = index.total_lines();
        assert_eq!(added, 3);

        let total = manager.append_lines(&index, old_total, new_total);
        assert_eq!(total, Some(2));
        assert_eq!(manager.total_matches(), 2);
    }

    #[test]
    fn test_invert_search() {
        let log = "error line\ninfo line 1\ninfo line 2\n";
        let (_file, index) = create_test_file(log);

        let manager = SearchManager::new();
        let res = manager
            .search(
                &index,
                SearchOptions {
                    query: "error".to_string(),
                    is_regex: false,
                    case_sensitive: false,
                    invert: true,
                },
            )
            .unwrap();

        assert_eq!(res.total_matches, 2);
        assert_eq!(res.match_lines, vec![2, 3]);
    }

    #[test]
    fn test_empty_query_clears_search() {
        let log = "some log content\n";
        let (_file, index) = create_test_file(log);

        let manager = SearchManager::new();
        let res = manager
            .search(
                &index,
                SearchOptions {
                    query: "   ".to_string(),
                    is_regex: false,
                    case_sensitive: false,
                    invert: false,
                },
            )
            .unwrap();

        assert_eq!(res.total_matches, 0);
        assert!(!manager.has_active_search());
    }

    #[test]
    fn test_cancellation_aborts_search() {
        // Create 20,000 lines
        let mut buffer = String::new();
        for i in 0..20000 {
            buffer.push_str(&format!("2026-10-27 INFO line {}\n", i));
        }
        let (_file, _index) = create_test_file(&buffer);

        let manager = SearchManager::new();
        // Trigger cancellation in advance or immediately
        manager.cancel_active();
        assert!(!manager.has_active_search());
    }

    #[test]
    fn test_export_filtered_to_file() {
        let log = "line 1 error\nline 2 info\nline 3 error fatal\n";
        let (_file, index) = create_test_file(log);

        let manager = SearchManager::new();
        manager
            .search(
                &index,
                SearchOptions {
                    query: "error".to_string(),
                    is_regex: false,
                    case_sensitive: false,
                    invert: false,
                },
            )
            .unwrap();

        let dest = NamedTempFile::new().unwrap();
        let exported_count = manager.export_filtered_to_file(&index, dest.path()).unwrap();
        assert_eq!(exported_count, 2);

        let content = std::fs::read_to_string(dest.path()).unwrap();
        assert_eq!(content, "line 1 error\nline 3 error fatal\n");

        let snippet = manager.get_all_filtered_content(&index, 10).unwrap();
        assert_eq!(snippet, "line 1 error\nline 3 error fatal\n");
    }
}
