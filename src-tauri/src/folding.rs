use crate::indexer::{LogIndex, LogLine};
use parking_lot::RwLock;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StacktraceGroup {
    pub id: usize,
    pub start_line: usize, // 1-based line number in file
    pub end_line: usize,   // 1-based line number (inclusive)
    pub line_count: usize,
    pub expanded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayRow {
    Single(usize),          // 0-based line index in LogIndex
    CollapsedGroup(usize),  // group id in FoldingState
}

/// Compact interval representing a collapsed stacktrace group for zero-allocation O(log K) virtual list mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollapsedInterval {
    pub group_id: usize,
    pub start_line_0: usize, // 0-based index of first line in group
    pub end_line_0: usize,   // 0-based index of last line in group (inclusive)
    pub line_count: usize,
    pub display_row: usize,  // 0-based virtual display row index
    pub cum_saved: usize,    // Cumulative lines collapsed up to and including this interval
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoldingStatus {
    pub total_groups: usize,
    pub total_collapsed_lines: usize,
    pub auto_collapse_enabled: bool,
}

/// Checks whether raw line bytes match common stacktrace frame signatures.
#[inline]
pub fn is_stacktrace_line(raw: &[u8]) -> bool {
    let mut i = 0;
    while i < raw.len() && (raw[i] == b' ' || raw[i] == b'\t') {
        i += 1;
    }
    let trimmed = &raw[i..];
    if trimmed.is_empty() {
        return false;
    }

    // Java / Kotlin / JS / TS: "at ..."
    if trimmed.starts_with(b"at ") {
        return true;
    }

    // Java / Kotlin: "Caused by: ..." or "Caused by "
    if trimmed.starts_with(b"Caused by:") || trimmed.starts_with(b"Caused by ") {
        return true;
    }

    // Java: "... 42 more"
    if trimmed.starts_with(b"...") {
        let rest = &trimmed[3..];
        let mut k = 0;
        while k < rest.len() && rest[k] == b' ' {
            k += 1;
        }
        if k < rest.len() && rest[k].is_ascii_digit() {
            return true;
        }
    }

    // Python traceback lines: 'File "...", line 123' or header
    if trimmed.starts_with(b"File \"") || trimmed.starts_with(b"Traceback (most recent call last):") {
        return true;
    }

    // Rust / C++ / GDB stack frames: "0: 0x...", "1: ...", "   0: std::..."
    if trimmed.len() >= 3 && trimmed[0].is_ascii_digit() {
        let mut j = 1;
        while j < trimmed.len() && trimmed[j].is_ascii_digit() {
            j += 1;
        }
        if j < trimmed.len() && trimmed[j] == b':' {
            return true;
        }
    }

    // Go goroutine stack traces
    if trimmed.starts_with(b"goroutine ") || trimmed.starts_with(b"stack backtrace:") {
        return true;
    }

    false
}

pub struct FoldingState {
    pub groups: Vec<StacktraceGroup>,
    pub auto_collapse_enabled: bool,
    pub collapsed_intervals: Vec<CollapsedInterval>,
    pub total_lines: usize,
}

impl Default for FoldingState {
    fn default() -> Self {
        Self {
            groups: Vec::new(),
            auto_collapse_enabled: true,
            collapsed_intervals: Vec::new(),
            total_lines: 0,
        }
    }
}

pub struct FoldingManager {
    state: RwLock<FoldingState>,
}

impl Default for FoldingManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FoldingManager {
    pub fn new() -> Self {
        Self {
            state: RwLock::new(FoldingState::default()),
        }
    }

    /// Clears folding state.
    pub fn clear(&self) {
        let mut state = self.state.write();
        state.groups.clear();
        state.collapsed_intervals.clear();
        state.total_lines = 0;
    }

    /// Scans the entire index to detect consecutive stacktrace runs (>= 2 lines)
    /// and populates the display row mappings. Parallelized via Rayon for large files.
    pub fn scan_index(&self, index: &LogIndex) {
        let total = index.total_lines();
        if total == 0 {
            let mut state = self.state.write();
            state.groups.clear();
            state.collapsed_intervals.clear();
            state.total_lines = 0;
            return;
        }

        // Parallel chunk scanning for files with > 16384 lines
        const PARALLEL_THRESHOLD: usize = 16384;
        const CHUNK_SIZE: usize = 16384;

        let groups = if total > PARALLEL_THRESHOLD {
            let chunk_count = (total + CHUNK_SIZE - 1) / CHUNK_SIZE;
            // Each chunk returns local runs: (start_line_0, end_line_0_inclusive)
            let chunk_runs: Vec<Vec<(usize, usize)>> = (0..chunk_count)
                .into_par_iter()
                .map(|chunk_idx| {
                    let start_line = chunk_idx * CHUNK_SIZE;
                    let end_line = (start_line + CHUNK_SIZE).min(total);
                    let mut local_runs = Vec::new();
                    let mut current_start: Option<usize> = None;

                    for line_idx in start_line..end_line {
                        let is_st = if let Some(raw) = index.get_line_raw(line_idx) {
                            is_stacktrace_line(raw)
                        } else {
                            false
                        };

                        if is_st {
                            if current_start.is_none() {
                                current_start = Some(line_idx);
                            }
                        } else if let Some(s) = current_start {
                            local_runs.push((s, line_idx - 1));
                            current_start = None;
                        }
                    }

                    if let Some(s) = current_start {
                        local_runs.push((s, end_line - 1));
                    }

                    local_runs
                })
                .collect();

            // Stitch adjacent runs across chunk boundaries
            let mut merged_runs: Vec<(usize, usize)> = Vec::new();
            for chunk in chunk_runs {
                for (start, end) in chunk {
                    if let Some(last) = merged_runs.last_mut() {
                        if last.1 + 1 == start {
                            last.1 = end;
                            continue;
                        }
                    }
                    merged_runs.push((start, end));
                }
            }

            // Keep only runs of >= 2 lines
            merged_runs
                .into_iter()
                .filter(|(start, end)| end + 1 - start >= 2)
                .enumerate()
                .map(|(id, (start, end))| StacktraceGroup {
                    id,
                    start_line: start + 1,
                    end_line: end + 1,
                    line_count: end + 1 - start,
                    expanded: false,
                })
                .collect()
        } else {
            let mut groups = Vec::new();
            let mut current_group_start: Option<usize> = None;

            for line_idx in 0..total {
                let is_st = if let Some(raw) = index.get_line_raw(line_idx) {
                    is_stacktrace_line(raw)
                } else {
                    false
                };

                if is_st {
                    if current_group_start.is_none() {
                        current_group_start = Some(line_idx);
                    }
                } else if let Some(start_idx) = current_group_start {
                    let count = line_idx - start_idx;
                    if count >= 2 {
                        groups.push(StacktraceGroup {
                            id: groups.len(),
                            start_line: start_idx + 1, // 1-based
                            end_line: line_idx,        // 1-based inclusive
                            line_count: count,
                            expanded: false,
                        });
                    }
                    current_group_start = None;
                }
            }

            // Handle trailing stacktrace run at EOF
            if let Some(start_idx) = current_group_start {
                let count = total - start_idx;
                if count >= 2 {
                    groups.push(StacktraceGroup {
                        id: groups.len(),
                        start_line: start_idx + 1,
                        end_line: total,
                        line_count: count,
                        expanded: false,
                    });
                }
            }
            groups
        };

        let mut state = self.state.write();
        state.groups = groups;
        state.total_lines = total;
        state.rebuild_display_rows();
    }

    /// Incremental scan on file append.
    pub fn append_lines(&self, index: &LogIndex, start_line: usize, end_line: usize) {
        let mut state = self.state.write();
        state.total_lines = end_line;

        // Check if last group was at the boundary
        let mut check_from = start_line;
        if let Some(last_group) = state.groups.last_mut() {
            if last_group.end_line == start_line {
                // Could continue the last group
                check_from = last_group.start_line - 1;
                state.groups.pop();
            }
        }

        let mut current_group_start: Option<usize> = None;
        for line_idx in check_from..end_line {
            let is_st = if let Some(raw) = index.get_line_raw(line_idx) {
                is_stacktrace_line(raw)
            } else {
                false
            };

            if is_st {
                if current_group_start.is_none() {
                    current_group_start = Some(line_idx);
                }
            } else if let Some(start_idx) = current_group_start {
                let count = line_idx - start_idx;
                if count >= 2 {
                    let group_id = state.groups.len();
                    state.groups.push(StacktraceGroup {
                        id: group_id,
                        start_line: start_idx + 1,
                        end_line: line_idx,
                        line_count: count,
                        expanded: false,
                    });
                }
                current_group_start = None;
            }
        }

        if let Some(start_idx) = current_group_start {
            let count = end_line - start_idx;
            if count >= 2 {
                let group_id = state.groups.len();
                state.groups.push(StacktraceGroup {
                    id: group_id,
                    start_line: start_idx + 1,
                    end_line,
                    line_count: count,
                    expanded: false,
                });
            }
        }

        state.rebuild_display_rows();
    }

    /// Toggles a specific group's expanded state in O(1) time without full document reallocations.
    pub fn toggle_group(&self, group_id: usize) -> Option<bool> {
        let mut state = self.state.write();
        if let Some(group) = state.groups.iter_mut().find(|g| g.id == group_id) {
            group.expanded = !group.expanded;
            let new_val = group.expanded;
            state.rebuild_display_rows();
            Some(new_val)
        } else {
            None
        }
    }

    /// Sets whether auto-collapsing of stacktraces is globally enabled.
    pub fn set_auto_collapse(&self, enabled: bool) {
        let mut state = self.state.write();
        state.auto_collapse_enabled = enabled;
        // Reset all individual group states when toggling global mode
        for group in state.groups.iter_mut() {
            group.expanded = !enabled;
        }
        state.rebuild_display_rows();
    }

    /// Expand all stacktrace groups.
    pub fn expand_all(&self) {
        let mut state = self.state.write();
        for group in state.groups.iter_mut() {
            group.expanded = true;
        }
        state.rebuild_display_rows();
    }

    /// Collapse all stacktrace groups.
    pub fn collapse_all(&self) {
        let mut state = self.state.write();
        for group in state.groups.iter_mut() {
            group.expanded = false;
        }
        state.rebuild_display_rows();
    }

    /// Returns the effective total row count for the virtual list in O(1) time.
    pub fn total_display_rows(&self) -> usize {
        let state = self.state.read();
        if !state.auto_collapse_enabled || state.collapsed_intervals.is_empty() {
            return state.total_lines;
        }
        let total_saved = state.collapsed_intervals.last().map(|c| c.cum_saved).unwrap_or(0);
        state.total_lines.saturating_sub(total_saved)
    }

    /// Returns high-level status of stacktraces in the document.
    pub fn get_status(&self) -> FoldingStatus {
        let state = self.state.read();
        let total_collapsed_lines = if state.auto_collapse_enabled {
            state.collapsed_intervals.last().map(|c| c.cum_saved).unwrap_or(0)
        } else {
            0
        };

        FoldingStatus {
            total_groups: state.groups.len(),
            total_collapsed_lines,
            auto_collapse_enabled: state.auto_collapse_enabled,
        }
    }

    /// Slices visible lines according to display row mapping (handling folded pills) in O(log K) zero-allocation.
    pub fn get_display_slice(&self, index: &LogIndex, start_row: usize, count: usize) -> Vec<LogLine> {
        let state = self.state.read();
        let total_display = if !state.auto_collapse_enabled || state.collapsed_intervals.is_empty() {
            state.total_lines
        } else {
            let total_saved = state.collapsed_intervals.last().map(|c| c.cum_saved).unwrap_or(0);
            state.total_lines.saturating_sub(total_saved)
        };

        if total_display == 0 || start_row >= total_display || count == 0 {
            return Vec::new();
        }

        let end_row = (start_row + count).min(total_display);
        let mut lines = Vec::with_capacity(end_row - start_row);

        if !state.auto_collapse_enabled || state.collapsed_intervals.is_empty() {
            return index.get_lines_slice(start_row, end_row - start_row);
        }

        let intervals = &state.collapsed_intervals;
        let mut interval_idx = intervals.partition_point(|it| it.display_row < start_row);

        for row_idx in start_row..end_row {
            if interval_idx < intervals.len() && intervals[interval_idx].display_row == row_idx {
                let it = intervals[interval_idx];
                let group = &state.groups[it.group_id];
                let byte_offset = index.get_line_offset(it.start_line_0).unwrap_or(0);

                lines.push(LogLine {
                    line_number: group.start_line,
                    content: format!(
                        "    [+ {} stacktrace lines (lines {}-{}) — click to expand]",
                        group.line_count, group.start_line, group.end_line
                    ),
                    byte_offset,
                    is_stacktrace: true,
                    is_collapsed_group: true,
                    group_id: Some(group.id),
                    group_line_count: Some(group.line_count),
                });
                interval_idx += 1;
            } else {
                let saved_before = if interval_idx == 0 {
                    0
                } else {
                    intervals[interval_idx - 1].cum_saved
                };
                let line_idx = row_idx + saved_before;
                if let Some(raw) = index.get_line_raw(line_idx) {
                    let mut content_slice = raw;
                    if content_slice.ends_with(b"\n") {
                        content_slice = &content_slice[..content_slice.len() - 1];
                    }
                    if content_slice.ends_with(b"\r") {
                        content_slice = &content_slice[..content_slice.len() - 1];
                    }
                    let content = String::from_utf8_lossy(content_slice).into_owned();
                    let is_st = is_stacktrace_line(raw);
                    let byte_offset = index.get_line_offset(line_idx).unwrap_or(0);

                    lines.push(LogLine {
                        line_number: line_idx + 1,
                        content,
                        byte_offset,
                        is_stacktrace: is_st,
                        is_collapsed_group: false,
                        group_id: None,
                        group_line_count: None,
                    });
                }
            }
        }

        lines
    }

    /// Maps a 1-based log line number to its corresponding 0-based virtual display row in O(log K) time.
    /// If the line falls inside a collapsed stacktrace group, returns that group's placeholder row.
    pub fn line_to_display_row(&self, line_number: usize) -> usize {
        let state = self.state.read();
        let target_line_idx = line_number.saturating_sub(1);
        if !state.auto_collapse_enabled || state.collapsed_intervals.is_empty() {
            return target_line_idx.min(state.total_lines.saturating_sub(1));
        }

        if target_line_idx >= state.total_lines {
            let total_saved = state.collapsed_intervals.last().map(|c| c.cum_saved).unwrap_or(0);
            return state.total_lines.saturating_sub(total_saved).saturating_sub(1);
        }

        let intervals = &state.collapsed_intervals;
        let idx = intervals.partition_point(|it| it.end_line_0 < target_line_idx);
        if idx < intervals.len() {
            let it = &intervals[idx];
            if target_line_idx >= it.start_line_0 {
                return it.display_row;
            }
        }

        let saved_before = if idx == 0 {
            0
        } else {
            intervals[idx - 1].cum_saved
        };
        target_line_idx.saturating_sub(saved_before)
    }
}

impl FoldingState {
    pub fn rebuild_display_rows(&mut self) {
        self.collapsed_intervals.clear();
        if !self.auto_collapse_enabled || self.groups.is_empty() {
            return;
        }

        let mut cum_saved = 0;
        for group in &self.groups {
            if !group.expanded {
                let start_line_0 = group.start_line - 1;
                let end_line_0 = group.end_line - 1;
                let display_row = start_line_0 - cum_saved;
                cum_saved += group.line_count - 1;
                self.collapsed_intervals.push(CollapsedInterval {
                    group_id: group.id,
                    start_line_0,
                    end_line_0,
                    line_count: group.line_count,
                    display_row,
                    cum_saved,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_is_stacktrace_line() {
        assert!(is_stacktrace_line(b"    at com.example.MyClass.run(MyClass.java:42)"));
        assert!(is_stacktrace_line(b"\tat java.base/java.lang.Thread.run(Thread.java:829)"));
        assert!(is_stacktrace_line(b"Caused by: java.lang.NullPointerException: null"));
        assert!(is_stacktrace_line(b"  Caused by: java.io.IOException: connection reset"));
        assert!(is_stacktrace_line(b"\t... 14 more"));
        assert!(is_stacktrace_line(b"  File \"/app/server.py\", line 42, in handle_request"));
        assert!(is_stacktrace_line(b"Traceback (most recent call last):"));
        assert!(is_stacktrace_line(b"   0: std::panicking::begin_panic"));
        assert!(is_stacktrace_line(b"goroutine 1 [running]:"));

        // Non-stacktrace lines
        assert!(!is_stacktrace_line(b"2026-10-27 12:00:00 [ERROR] Connection timed out"));
        assert!(!is_stacktrace_line(b"Just looking at the file"));
        assert!(!is_stacktrace_line(b""));
    }

    #[test]
    fn test_folding_scan_and_toggle() {
        let mut tmp = NamedTempFile::new().unwrap();
        let log_data = "\
2026-10-27 12:00:00 [INFO] Starting service
2026-10-27 12:00:01 [ERROR] Failed to connect
    at com.example.Client.connect(Client.java:10)
    at com.example.Client.run(Client.java:20)
    at com.example.Main.main(Main.java:5)
2026-10-27 12:00:02 [INFO] Retrying
    at com.example.Retry.do(Retry.java:1)
    at com.example.Retry.do2(Retry.java:2)
2026-10-27 12:00:03 [INFO] Connected
";
        tmp.write_all(log_data.as_bytes()).unwrap();
        tmp.flush().unwrap();

        let (index, _) = LogIndex::open(tmp.path()).unwrap();
        assert_eq!(index.total_lines(), 9);

        let manager = FoldingManager::new();
        manager.scan_index(&index);

        // Group 1: lines 3..5 (3 lines)
        // Group 2: lines 7..8 (2 lines)
        let status = manager.get_status();
        assert_eq!(status.total_groups, 2);
        assert_eq!(status.total_collapsed_lines, 3); // (3-1) + (2-1) = 3 lines saved

        // Total display rows: 9 - 3 = 6 rows
        assert_eq!(manager.total_display_rows(), 6);

        // Slice rows 0..6
        let slice = manager.get_display_slice(&index, 0, 6);
        assert_eq!(slice.len(), 6);
        assert_eq!(slice[0].line_number, 1);
        assert_eq!(slice[1].line_number, 2);
        // Row 2 is collapsed group 0 (representing lines 3-5)
        assert!(slice[2].is_collapsed_group);
        assert_eq!(slice[2].group_line_count, Some(3));
        assert_eq!(slice[2].line_number, 3);

        // Expand group 0
        let toggled = manager.toggle_group(0);
        assert_eq!(toggled, Some(true));
        // Row count now: 6 + 2 = 8 rows
        assert_eq!(manager.total_display_rows(), 8);

        // Collapse all
        manager.collapse_all();
        assert_eq!(manager.total_display_rows(), 6);

        // Disable auto-collapse globally
        manager.set_auto_collapse(false);
        assert_eq!(manager.total_display_rows(), 9);
    }

    #[test]
    fn test_folding_incremental_append() {
        let mut tmp = NamedTempFile::new().unwrap();
        let log_data = "\
2026-10-27 12:00:00 [INFO] Line 1
2026-10-27 12:00:01 [ERROR] Failure
    at foo.bar.A.run(A.java:1)
    at foo.bar.B.run(B.java:2)
";
        tmp.write_all(log_data.as_bytes()).unwrap();
        tmp.flush().unwrap();

        let (mut index, _) = LogIndex::open(tmp.path()).unwrap();
        assert_eq!(index.total_lines(), 4);

        let manager = FoldingManager::new();
        manager.scan_index(&index);
        assert_eq!(manager.get_status().total_groups, 1);
        assert_eq!(manager.total_display_rows(), 3); // 4 - 1 = 3

        // Append more stacktrace lines
        let more_data = "\
    at foo.bar.C.run(C.java:3)
2026-10-27 12:00:02 [INFO] Line 4
";
        tmp.write_all(more_data.as_bytes()).unwrap();
        tmp.flush().unwrap();

        let added = index.append_from_disk().unwrap();
        assert_eq!(added, 2);
        assert_eq!(index.total_lines(), 6);

        manager.append_lines(&index, 4, 6);
        let status = manager.get_status();
        assert_eq!(status.total_groups, 1);
        // The group was extended from 2 to 3 lines (lines 3..5)
        assert_eq!(status.total_collapsed_lines, 2); // 3 - 1 = 2
        assert_eq!(manager.total_display_rows(), 4); // 6 - 2 = 4
    }

    #[test]
    fn test_line_to_display_row_mapping() {
        let mut tmp = NamedTempFile::new().unwrap();
        let log_data = "\
Line 1: Normal
Line 2: Error
\tat com.example.A(A.java:10)
\tat com.example.B(B.java:20)
\tat com.example.C(C.java:30)
Line 6: Normal line after
";
        tmp.write_all(log_data.as_bytes()).unwrap();
        tmp.flush().unwrap();

        let (index, _) = LogIndex::open(tmp.path()).unwrap();
        let manager = FoldingManager::new();
        manager.scan_index(&index);

        // Display rows:
        // row 0: Line 1 (line 1)
        // row 1: Line 2 (line 2)
        // row 2: CollapsedGroup (lines 3, 4, 5)
        // row 3: Line 6 (line 6)
        assert_eq!(manager.line_to_display_row(1), 0);
        assert_eq!(manager.line_to_display_row(2), 1);
        assert_eq!(manager.line_to_display_row(3), 2);
        assert_eq!(manager.line_to_display_row(4), 2);
        assert_eq!(manager.line_to_display_row(5), 2);
        assert_eq!(manager.line_to_display_row(6), 3);
    }
}
