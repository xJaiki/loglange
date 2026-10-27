use memchr::memchr_iter;
use memmap2::Mmap;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogLine {
    pub line_number: usize,
    pub content: String,
    pub byte_offset: u64,
    #[serde(default)]
    pub is_stacktrace: bool,
    #[serde(default)]
    pub is_collapsed_group: bool,
    #[serde(default)]
    pub group_id: Option<usize>,
    #[serde(default)]
    pub group_line_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOpenResult {
    #[serde(default)]
    pub tab_id: String,
    pub path: String,
    pub file_name: String,
    pub file_size: u64,
    pub total_lines: usize,
    #[serde(default)]
    pub total_display_rows: usize,
    pub index_time_ms: f64,
}

pub struct LogIndex {
    path: PathBuf,
    file_size: u64,
    mmap: Option<Arc<Mmap>>,
    offsets: Vec<u64>,
}

impl LogIndex {
    /// Open a file and index line offsets using SIMD + parallel scanning.
    pub fn open<P: AsRef<Path>>(path: P) -> std::io::Result<(Self, f64)> {
        let start_time = Instant::now();
        let path_buf = path.as_ref().to_path_buf();
        let file = File::open(&path_buf)?;
        let metadata = file.metadata()?;
        let file_size = metadata.len();

        if file_size == 0 {
            let duration = start_time.elapsed().as_secs_f64() * 1000.0;
            return Ok((
                Self {
                    path: path_buf,
                    file_size: 0,
                    mmap: None,
                    offsets: Vec::new(),
                },
                duration,
            ));
        }

        // Memory-map the file
        let mmap = unsafe { memmap2::MmapOptions::new().map(&file)? };
        let mmap_arc = Arc::new(mmap);
        let slice = &mmap_arc[..];

        // Parallel line indexing with memchr SIMD
        // Threshold: 4MB for parallelization
        const CHUNK_SIZE: usize = 4 * 1024 * 1024; // 4MB chunks

        let offsets = if file_size as usize > CHUNK_SIZE {
            let chunk_results: Vec<Vec<u64>> = slice
                .par_chunks(CHUNK_SIZE)
                .enumerate()
                .map(|(chunk_idx, chunk)| {
                    let base_offset = (chunk_idx * CHUNK_SIZE) as u64;
                    let mut local_offsets = Vec::new();
                    for pos in memchr_iter(b'\n', chunk) {
                        let next_line_offset = base_offset + pos as u64 + 1;
                        if next_line_offset < file_size {
                            local_offsets.push(next_line_offset);
                        }
                    }
                    local_offsets
                })
                .collect();

            let total_lines = 1 + chunk_results.iter().map(|v| v.len()).sum::<usize>();
            let mut all_offsets = Vec::with_capacity(total_lines);
            all_offsets.push(0);
            for chunk_offsets in chunk_results {
                all_offsets.extend(chunk_offsets);
            }
            all_offsets
        } else {
            let mut local_offsets = Vec::new();
            local_offsets.push(0);
            for pos in memchr_iter(b'\n', slice) {
                let next_line_offset = pos as u64 + 1;
                if next_line_offset < file_size {
                    local_offsets.push(next_line_offset);
                }
            }
            local_offsets
        };

        let duration = start_time.elapsed().as_secs_f64() * 1000.0;

        Ok((
            Self {
                path: path_buf,
                file_size,
                mmap: Some(mmap_arc),
                offsets,
            },
            duration,
        ))
    }

    pub fn total_lines(&self) -> usize {
        self.offsets.len()
    }

    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Incrementally updates line offsets when the file has grown on disk.
    /// Reads ONLY from the last known byte offset to the new EOF.
    /// Returns the number of newly added lines.
    pub fn append_from_disk(&mut self) -> std::io::Result<usize> {
        let metadata = std::fs::metadata(&self.path)?;
        let new_size = metadata.len();

        if new_size == self.file_size {
            return Ok(0);
        }

        // If the file shrank (truncated or rotated), re-index from scratch
        if new_size < self.file_size {
            let (new_index, _) = LogIndex::open(&self.path)?;
            let total = new_index.total_lines();
            *self = new_index;
            return Ok(total);
        }

        let old_size = self.file_size;
        let file = File::open(&self.path)?;
        let new_mmap = unsafe { memmap2::MmapOptions::new().map(&file)? };

        let prev_lines_count = self.offsets.len();
        const CHUNK_SIZE: usize = 4 * 1024 * 1024; // 4MB chunks

        if old_size == 0 {
            let mut local_offsets = Vec::new();
            local_offsets.push(0);
            for pos in memchr_iter(b'\n', &new_mmap[..]) {
                let next_line_offset = pos as u64 + 1;
                if next_line_offset < new_size {
                    local_offsets.push(next_line_offset);
                }
            }
            self.offsets = local_offsets;
        } else {
            // If the byte immediately preceding the appended section was a newline,
            // then old_size is the starting offset of a new line.
            if new_mmap[old_size as usize - 1] == b'\n' {
                self.offsets.push(old_size);
            }

            let append_slice = &new_mmap[old_size as usize..];
            if append_slice.len() > CHUNK_SIZE {
                let chunk_results: Vec<Vec<u64>> = append_slice
                    .par_chunks(CHUNK_SIZE)
                    .enumerate()
                    .map(|(chunk_idx, chunk)| {
                        let base_offset = old_size + (chunk_idx * CHUNK_SIZE) as u64;
                        let mut local_offsets = Vec::new();
                        for pos in memchr_iter(b'\n', chunk) {
                            let next_line_offset = base_offset + pos as u64 + 1;
                            if next_line_offset < new_size {
                                local_offsets.push(next_line_offset);
                            }
                        }
                        local_offsets
                    })
                    .collect();

                for chunk_offsets in chunk_results {
                    self.offsets.extend(chunk_offsets);
                }
            } else {
                for pos in memchr_iter(b'\n', append_slice) {
                    let next_line_offset = old_size + pos as u64 + 1;
                    if next_line_offset < new_size {
                        self.offsets.push(next_line_offset);
                    }
                }
            }
        }

        self.file_size = new_size;
        self.mmap = Some(Arc::new(new_mmap));

        let added_lines = self.offsets.len().saturating_sub(prev_lines_count);
        Ok(added_lines)
    }

    pub fn get_line_offset(&self, line_idx: usize) -> Option<u64> {
        self.offsets.get(line_idx).copied()
    }

    pub fn get_line_raw(&self, line_idx: usize) -> Option<&[u8]> {
        if line_idx >= self.offsets.len() {
            return None;
        }
        let mmap = self.mmap.as_ref()?;
        let start = self.offsets[line_idx] as usize;
        let end = if line_idx + 1 < self.offsets.len() {
            self.offsets[line_idx + 1] as usize
        } else {
            self.file_size as usize
        };

        if start <= end && end <= mmap.len() {
            Some(&mmap[start..end])
        } else {
            None
        }
    }

    /// Fast SIMD/sub-string search over line prefixes to find the first line matching a timestamp query.
    /// Returns 1-based line number.
    pub fn find_timestamp_line(&self, query: &str) -> Option<usize> {
        let trimmed = query.trim();
        if trimmed.is_empty() || self.offsets.is_empty() {
            return None;
        }

        let query_bytes = trimmed.as_bytes();
        let mmap = self.mmap.as_ref()?;
        let total = self.offsets.len();

        let finder = memchr::memmem::Finder::new(query_bytes);

        // Scan first 64 bytes of each line in parallel
        (0..total).into_par_iter().find_first(|&idx| {
            let start = self.offsets[idx] as usize;
            let end = if idx + 1 < total {
                self.offsets[idx + 1] as usize
            } else {
                self.file_size as usize
            };
            let prefix_len = (end.saturating_sub(start)).min(64);
            if start + prefix_len <= mmap.len() {
                let prefix = &mmap[start..start + prefix_len];
                finder.find(prefix).is_some()
            } else {
                false
            }
        }).map(|idx| idx + 1)
    }

    pub fn get_lines_slice(&self, start_line: usize, count: usize) -> Vec<LogLine> {
        if self.offsets.is_empty() || start_line >= self.offsets.len() || count == 0 {
            return Vec::new();
        }

        let end_line = (start_line + count).min(self.offsets.len());
        let mut lines = Vec::with_capacity(end_line - start_line);

        let Some(mmap) = self.mmap.as_ref() else {
            return Vec::new();
        };

        for idx in start_line..end_line {
            let start = self.offsets[idx] as usize;
            let end = if idx + 1 < self.offsets.len() {
                self.offsets[idx + 1] as usize
            } else {
                self.file_size as usize
            };

            if start <= end && end <= mmap.len() {
                let mut raw = &mmap[start..end];
                // Strip line endings (\r\n or \n)
                if raw.ends_with(b"\n") {
                    raw = &raw[..raw.len() - 1];
                }
                if raw.ends_with(b"\r") {
                    raw = &raw[..raw.len() - 1];
                }

                lines.push(LogLine {
                    line_number: idx + 1,
                    content: String::from_utf8_lossy(raw).into_owned(),
                    byte_offset: start as u64,
                    is_stacktrace: false,
                    is_collapsed_group: false,
                    group_id: None,
                    group_line_count: None,
                });
            }
        }

        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_indexing_small_file() {
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(temp, "Line 1: Hello").unwrap();
        writeln!(temp, "Line 2: World").unwrap();
        write!(temp, "Line 3: Trailing without newline").unwrap();
        temp.flush().unwrap();

        let (index, duration) = LogIndex::open(temp.path()).unwrap();
        println!("Indexed in {:.2}ms", duration);
        assert_eq!(index.total_lines(), 3);

        let lines = index.get_lines_slice(0, 10);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].line_number, 1);
        assert_eq!(lines[0].content, "Line 1: Hello");
        assert_eq!(lines[1].line_number, 2);
        assert_eq!(lines[1].content, "Line 2: World");
        assert_eq!(lines[2].line_number, 3);
        assert_eq!(lines[2].content, "Line 3: Trailing without newline");
    }

    #[test]
    fn test_empty_file() {
        let temp = NamedTempFile::new().unwrap();
        let (index, _) = LogIndex::open(temp.path()).unwrap();
        assert_eq!(index.total_lines(), 0);
        assert_eq!(index.file_size(), 0);
        assert!(index.get_lines_slice(0, 10).is_empty());
    }

    #[test]
    fn test_incremental_append() {
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(temp, "Line 1: Alpha").unwrap();
        writeln!(temp, "Line 2: Beta").unwrap();
        temp.flush().unwrap();

        let (mut index, _) = LogIndex::open(temp.path()).unwrap();
        assert_eq!(index.total_lines(), 2);

        // Append 2 more lines
        writeln!(temp, "Line 3: Gamma").unwrap();
        writeln!(temp, "Line 4: Delta").unwrap();
        temp.flush().unwrap();

        let added = index.append_from_disk().unwrap();
        assert_eq!(added, 2);
        assert_eq!(index.total_lines(), 4);

        let lines = index.get_lines_slice(0, 10);
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0].content, "Line 1: Alpha");
        assert_eq!(lines[1].content, "Line 2: Beta");
        assert_eq!(lines[2].content, "Line 3: Gamma");
        assert_eq!(lines[3].content, "Line 4: Delta");
    }

    #[test]
    fn test_incremental_append_partial_line() {
        let mut temp = NamedTempFile::new().unwrap();
        write!(temp, "Partial line").unwrap();
        temp.flush().unwrap();

        let (mut index, _) = LogIndex::open(temp.path()).unwrap();
        assert_eq!(index.total_lines(), 1);
        assert_eq!(index.get_lines_slice(0, 1)[0].content, "Partial line");

        // Complete the line and add another
        writeln!(temp, " completed!").unwrap();
        writeln!(temp, "Next full line").unwrap();
        temp.flush().unwrap();

        index.append_from_disk().unwrap();
        assert_eq!(index.total_lines(), 2);

        let lines = index.get_lines_slice(0, 5);
        assert_eq!(lines[0].content, "Partial line completed!");
        assert_eq!(lines[1].content, "Next full line");
    }

    #[test]
    fn test_file_truncation_reindexing() {
        let mut temp = NamedTempFile::new().unwrap();
        for i in 1..=50 {
            writeln!(temp, "Initial line {}", i).unwrap();
        }
        temp.flush().unwrap();

        let (mut index, _) = LogIndex::open(temp.path()).unwrap();
        assert_eq!(index.total_lines(), 50);

        // Truncate file and write 3 lines
        let file = File::create(temp.path()).unwrap();
        let mut writer = std::io::BufWriter::new(file);
        writeln!(writer, "Reset line 1").unwrap();
        writeln!(writer, "Reset line 2").unwrap();
        writeln!(writer, "Reset line 3").unwrap();
        writer.flush().unwrap();
        drop(writer);

        index.append_from_disk().unwrap();
        assert_eq!(index.total_lines(), 3);
        let lines = index.get_lines_slice(0, 10);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].content, "Reset line 1");
        assert_eq!(lines[2].content, "Reset line 3");
    }

    #[test]
    fn test_find_timestamp_line() {
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(temp, "2026-10-27 10:00:00.123 [INFO] App started").unwrap();
        writeln!(temp, "2026-10-27 10:15:30.456 [WARN] High memory").unwrap();
        writeln!(temp, "2026-10-27 11:45:00.789 [ERROR] Database crash").unwrap();
        temp.flush().unwrap();

        let (index, _) = LogIndex::open(temp.path()).unwrap();
        assert_eq!(index.find_timestamp_line("10:15"), Some(2));
        assert_eq!(index.find_timestamp_line("11:45:00"), Some(3));
        assert_eq!(index.find_timestamp_line("10:00:00"), Some(1));
        assert_eq!(index.find_timestamp_line("23:59:59"), None);
    }

    #[test]
    #[ignore] // Run with `cargo test -- --ignored` to verify 1GB file indexing performance
    fn test_indexing_1gb_file() {
        use std::io::BufWriter;
        let mut temp = NamedTempFile::new().unwrap();
        let mut writer = BufWriter::with_capacity(4 * 1024 * 1024, &mut temp);
        let sample_line = b"2026-10-27 12:00:00.123 [INFO] [worker-pool-4] com.example.service.Worker - Processing batch event request id=9876543210 status=SUCCESS latency_ms=12\n";
        let target_size = 1024 * 1024 * 1024; // 1 GB
        let mut written = 0;
        while written < target_size {
            writer.write_all(sample_line).unwrap();
            written += sample_line.len();
        }
        writer.flush().unwrap();
        drop(writer);

        let file_path = temp.path();
        let (index, duration_ms) = LogIndex::open(file_path).unwrap();
        println!(
            "Indexed 1GB file ({} bytes, {} lines) in {:.2}ms",
            index.file_size(),
            index.total_lines(),
            duration_ms
        );

        assert!(
            duration_ms < 100.0,
            "Opening and indexing 1GB must be < 100ms, took {:.2}ms",
            duration_ms
        );

        let first_slice = index.get_lines_slice(0, 50);
        assert_eq!(first_slice.len(), 50);
        assert_eq!(first_slice[0].line_number, 1);

        let last_slice = index.get_lines_slice(index.total_lines() - 10, 10);
        assert_eq!(last_slice.len(), 10);
    }
}
