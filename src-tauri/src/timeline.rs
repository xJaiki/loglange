use crate::indexer::LogIndex;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeBucket {
    pub start_time: i64,      // Timestamp in milliseconds
    pub end_time: i64,        // Timestamp in milliseconds
    pub label: String,        // Formatted label, e.g. "12:34:00" or "12:34:00.123"
    pub count: u32,           // Total lines in this bucket
    pub error_count: u32,     // Lines with ERROR / FATAL / CRITICAL
    pub warn_count: u32,      // Lines with WARN
    pub info_count: u32,      // Lines with INFO / DEBUG / TRACE / OTHER
    pub start_line: usize,    // 1-based line number of first line in this bucket
    pub end_line: usize,      // 1-based line number of last line in this bucket
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeHistogram {
    pub buckets: Vec<TimeBucket>,
    pub total_lines: usize,
    pub lines_with_timestamp: usize,
    pub min_time: i64,
    pub max_time: i64,
    pub min_time_str: String,
    pub max_time_str: String,
    pub has_timestamps: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeatmapBin {
    pub error_count: u32,
    pub warn_count: u32,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeverityHeatmap {
    pub total_lines: usize,
    pub total_errors: usize,
    pub total_warnings: usize,
    pub max_errors_in_bin: u32,
    pub max_warnings_in_bin: u32,
    pub bins: Vec<HeatmapBin>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warn,
    Info,
}

/// Howard Hinnant's algorithm to convert civil date (Y, M, D) to days since 1970-01-01.
fn civil_to_days(year: i32, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year } as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32;
    let doy = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe as i64 - 719468
}

/// Converts (year, month, day, hour, min, sec, ms) to milliseconds since Unix epoch.
pub fn to_epoch_millis(year: i32, month: u32, day: u32, hour: u32, min: u32, sec: u32, ms: u32) -> i64 {
    let days = civil_to_days(year, month, day);
    (days * 86400 + (hour as i64) * 3600 + (min as i64) * 60 + (sec as i64)) * 1000 + (ms as i64)
}

/// Parses 2 ascii decimal digits into u32.
#[inline(always)]
fn parse_2digits(b: &[u8]) -> Option<u32> {
    if b.len() >= 2 && b[0].is_ascii_digit() && b[1].is_ascii_digit() {
        Some((b[0] - b'0') as u32 * 10 + (b[1] - b'0') as u32)
    } else {
        None
    }
}

/// Parses 4 ascii decimal digits into u32.
#[inline(always)]
fn parse_4digits(b: &[u8]) -> Option<u32> {
    if b.len() >= 4
        && b[0].is_ascii_digit()
        && b[1].is_ascii_digit()
        && b[2].is_ascii_digit()
        && b[3].is_ascii_digit()
    {
        Some(
            (b[0] - b'0') as u32 * 1000
                + (b[1] - b'0') as u32 * 100
                + (b[2] - b'0') as u32 * 10
                + (b[3] - b'0') as u32,
        )
    } else {
        None
    }
}

/// Parses up to 3 fractional decimal digits into milliseconds (0..=999).
#[inline(always)]
fn parse_fractional_ms(b: &[u8]) -> u32 {
    let mut ms = 0u32;
    let mut mult = 100u32;
    for &byte in b.iter().take(3) {
        if byte.is_ascii_digit() {
            ms += (byte - b'0') as u32 * mult;
            mult /= 10;
        } else {
            break;
        }
    }
    ms
}

/// Fast parsing of month 3-letter abbreviation to month number 1..=12.
fn parse_month(b: &[u8]) -> Option<u32> {
    if b.len() < 3 {
        return None;
    }
    match &[b[0].to_ascii_uppercase(), b[1].to_ascii_lowercase(), b[2].to_ascii_lowercase()] {
        b"Jan" => Some(1),
        b"Feb" => Some(2),
        b"Mar" => Some(3),
        b"Apr" => Some(4),
        b"May" => Some(5),
        b"Jun" => Some(6),
        b"Jul" => Some(7),
        b"Aug" => Some(8),
        b"Sep" => Some(9),
        b"Oct" => Some(10),
        b"Nov" => Some(11),
        b"Dec" => Some(12),
        _ => None,
    }
}

/// Extracts timestamp in milliseconds from a log line prefix slice.
/// Returns Some(milliseconds) or None.
pub fn extract_timestamp(line: &[u8]) -> Option<i64> {
    if line.len() < 8 {
        return None;
    }

    let search_window = &line[..line.len().min(48)];

    // Check potential start offsets (e.g. at 0, or after '[' or after a space)
    let mut check_offsets = [0usize, 0, 0, 0];
    let mut offset_count = 1;

    for (i, &b) in search_window.iter().enumerate() {
        if (b == b'[' || b == b' ' || b == b'(') && i + 1 < search_window.len() {
            if offset_count < check_offsets.len() {
                check_offsets[offset_count] = i + 1;
                offset_count += 1;
            }
        }
    }

    for &start in &check_offsets[..offset_count] {
        let slice = &search_window[start..];

        // Format 1: ISO 8601 (YYYY-MM-DD[T ]HH:MM:SS) -> 19 chars min
        if slice.len() >= 19 && slice[4] == b'-' && slice[7] == b'-' && (slice[10] == b'T' || slice[10] == b' ') && slice[13] == b':' && slice[16] == b':' {
            if let (Some(y), Some(m), Some(d), Some(h), Some(min), Some(s)) = (
                parse_4digits(&slice[0..4]),
                parse_2digits(&slice[5..7]),
                parse_2digits(&slice[8..10]),
                parse_2digits(&slice[11..13]),
                parse_2digits(&slice[14..16]),
                parse_2digits(&slice[17..19]),
            ) {
                if m >= 1 && m <= 12 && d >= 1 && d <= 31 && h < 24 && min < 60 && s < 60 {
                    let ms = if slice.len() > 19 && (slice[19] == b'.' || slice[19] == b',') {
                        parse_fractional_ms(&slice[20..])
                    } else {
                        0
                    };
                    return Some(to_epoch_millis(y as i32, m, d, h, min, s, ms));
                }
            }
        }

        // Format 2: Syslog (Mmm DD HH:MM:SS) -> 15 chars min
        if slice.len() >= 15 && slice[3] == b' ' && slice[6] == b' ' && slice[9] == b':' && slice[12] == b':' {
            if let Some(month) = parse_month(&slice[0..3]) {
                let day_bytes = &slice[4..6];
                let day = if day_bytes[0] == b' ' && day_bytes[1].is_ascii_digit() {
                    (day_bytes[1] - b'0') as u32
                } else if day_bytes[0].is_ascii_digit() && day_bytes[1].is_ascii_digit() {
                    (day_bytes[0] - b'0') as u32 * 10 + (day_bytes[1] - b'0') as u32
                } else {
                    0
                };

                if day >= 1 && day <= 31 {
                    if let (Some(h), Some(min), Some(s)) = (
                        parse_2digits(&slice[7..9]),
                        parse_2digits(&slice[10..12]),
                        parse_2digits(&slice[13..15]),
                    ) {
                        if h < 24 && min < 60 && s < 60 {
                            let ms = if slice.len() > 15 && (slice[15] == b'.' || slice[15] == b',') {
                                parse_fractional_ms(&slice[16..])
                            } else {
                                0
                            };
                            return Some(to_epoch_millis(2026, month, day, h, min, s, ms));
                        }
                    }
                }
            }
        }

        // Format 3: Time only (HH:MM:SS) -> 8 chars min
        if slice.len() >= 8 && slice[2] == b':' && slice[5] == b':' {
            if let (Some(h), Some(min), Some(s)) = (
                parse_2digits(&slice[0..2]),
                parse_2digits(&slice[3..5]),
                parse_2digits(&slice[6..8]),
            ) {
                if h < 24 && min < 60 && s < 60 {
                    let ms = if slice.len() > 8 && (slice[8] == b'.' || slice[8] == b',') {
                        parse_fractional_ms(&slice[9..])
                    } else {
                        0
                    };
                    return Some(((h as i64) * 3600 + (min as i64) * 60 + (s as i64)) * 1000 + (ms as i64));
                }
            }
        }
    }

    None
}

/// Detects line severity from prefix slice.
#[inline(always)]
pub fn detect_severity(line: &[u8]) -> Severity {
    let check_len = line.len().min(128);
    let slice = &line[..check_len];

    // Quick byte search for Error keywords
    if contains_subslice(slice, b"ERROR")
        || contains_subslice(slice, b"FATAL")
        || contains_subslice(slice, b"CRITICAL")
        || contains_subslice(slice, b"SEVERE")
        || contains_subslice(slice, b"[E]")
        || contains_subslice(slice, b"ERR ")
    {
        Severity::Error
    } else if contains_subslice(slice, b"WARN") || contains_subslice(slice, b"[W]") {
        Severity::Warn
    } else {
        Severity::Info
    }
}

#[inline(always)]
fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    memchr::memmem::find(haystack, needle).is_some()
}

/// Formats epoch milliseconds to a concise time label.
pub fn format_time_label(millis: i64, show_ms: bool) -> String {
    let total_seconds = millis / 1000;
    let rem_ms = (millis % 1000).abs();

    if total_seconds < 86400 {
        // Time only (HH:MM:SS[.mmm])
        let h = total_seconds / 3600;
        let m = (total_seconds % 3600) / 60;
        let s = total_seconds % 60;
        if show_ms {
            format!("{:02}:{:02}:{:02}.{:03}", h, m, s, rem_ms)
        } else {
            format!("{:02}:{:02}:{:02}", h, m, s)
        }
    } else {
        // Epoch millis -> Time
        let rem_seconds = total_seconds % 86400;
        let h = rem_seconds / 3600;
        let min = (rem_seconds % 3600) / 60;
        let s = rem_seconds % 60;

        if show_ms {
            format!("{:02}:{:02}:{:02}.{:03}", h, min, s, rem_ms)
        } else {
            format!("{:02}:{:02}:{:02}", h, min, s)
        }
    }
}

/// Builds an interactive time histogram for all lines or a subset of line indices (e.g. filtered lines).
pub fn build_histogram(
    index: &LogIndex,
    line_indices: Option<&[u32]>, // 0-based line indices
    target_bucket_count: usize,
) -> TimeHistogram {
    let num_buckets = target_bucket_count.clamp(1, 150);
    let total_lines = index.total_lines();

    if total_lines == 0 {
        return TimeHistogram {
            buckets: Vec::new(),
            total_lines: 0,
            lines_with_timestamp: 0,
            min_time: 0,
            max_time: 0,
            min_time_str: String::new(),
            max_time_str: String::new(),
            has_timestamps: false,
        };
    }

    // Determine target lines to scan: either custom slice or full range
    match index.get_line_raw(0) {
        Some(_) => (),
        None => {
            return TimeHistogram {
                buckets: Vec::new(),
                total_lines,
                lines_with_timestamp: 0,
                min_time: 0,
                max_time: 0,
                min_time_str: String::new(),
                max_time_str: String::new(),
                has_timestamps: false,
            }
        }
    };

    // Scan lines in parallel to extract (line_index, timestamp, severity)
    let parsed_records: Vec<(usize, i64, Severity)> = if let Some(indices) = line_indices {
        indices
            .par_iter()
            .filter_map(|&raw_idx| {
                let idx = raw_idx as usize;
                if let Some(content) = index.get_line_raw(idx) {
                    let ts = extract_timestamp(content)?;
                    let sev = detect_severity(content);
                    Some((idx, ts, sev))
                } else {
                    None
                }
            })
            .collect()
    } else {
        (0..total_lines)
            .into_par_iter()
            .filter_map(|idx| {
                if let Some(content) = index.get_line_raw(idx) {
                    let ts = extract_timestamp(content)?;
                    let sev = detect_severity(content);
                    Some((idx, ts, sev))
                } else {
                    None
                }
            })
            .collect()
    };

    if parsed_records.is_empty() {
        return TimeHistogram {
            buckets: Vec::new(),
            total_lines: line_indices.map_or(total_lines, |l| l.len()),
            lines_with_timestamp: 0,
            min_time: 0,
            max_time: 0,
            min_time_str: String::new(),
            max_time_str: String::new(),
            has_timestamps: false,
        };
    }

    let min_time = parsed_records.iter().map(|r| r.1).min().unwrap_or(0);
    let max_time = parsed_records.iter().map(|r| r.1).max().unwrap_or(0);

    let time_span = max_time - min_time;
    let show_ms = time_span > 0 && time_span < 15_000;
    let actual_buckets_count = if time_span <= 0 { 1 } else { num_buckets };
    let bucket_duration = if time_span <= 0 {
        1
    } else {
        ((time_span as f64) / (actual_buckets_count as f64)).ceil() as i64
    };

    // Initialize buckets
    let mut buckets: Vec<TimeBucket> = (0..actual_buckets_count)
        .map(|i| {
            let b_start = min_time + (i as i64) * bucket_duration;
            let b_end = if i + 1 == actual_buckets_count {
                max_time + 1
            } else {
                b_start + bucket_duration
            };
            TimeBucket {
                start_time: b_start,
                end_time: b_end,
                label: format_time_label(b_start, show_ms),
                count: 0,
                error_count: 0,
                warn_count: 0,
                info_count: 0,
                start_line: usize::MAX,
                end_line: 0,
            }
        })
        .collect();

    // Populate buckets
    for (line_idx, ts, sev) in parsed_records.iter() {
        let line_num = line_idx + 1; // 1-based line number
        let raw_b_idx = if bucket_duration > 0 {
            ((ts - min_time) / bucket_duration) as usize
        } else {
            0
        };
        let b_idx = raw_b_idx.min(actual_buckets_count.saturating_sub(1));

        let bucket = &mut buckets[b_idx];
        bucket.count += 1;
        match sev {
            Severity::Error => bucket.error_count += 1,
            Severity::Warn => bucket.warn_count += 1,
            Severity::Info => bucket.info_count += 1,
        }

        if line_num < bucket.start_line {
            bucket.start_line = line_num;
        }
        if line_num > bucket.end_line {
            bucket.end_line = line_num;
        }
    }

    // Fix empty buckets start_line
    for b in &mut buckets {
        if b.count == 0 {
            b.start_line = 0;
            b.end_line = 0;
        }
    }

    TimeHistogram {
        buckets,
        total_lines: line_indices.map_or(total_lines, |l| l.len()),
        lines_with_timestamp: parsed_records.len(),
        min_time,
        max_time,
        min_time_str: format_time_label(min_time, show_ms),
        max_time_str: format_time_label(max_time, show_ms),
        has_timestamps: true,
    }
}

/// Builds a high-speed error and warning density heatmap for the Minimap track.
pub fn build_severity_heatmap(
    index: &LogIndex,
    target_bins: usize,
    custom_error_keywords: Option<&[String]>,
    custom_warn_keywords: Option<&[String]>,
) -> SeverityHeatmap {
    let total_lines = index.total_lines();
    if total_lines == 0 {
        return SeverityHeatmap {
            total_lines: 0,
            total_errors: 0,
            total_warnings: 0,
            max_errors_in_bin: 0,
            max_warnings_in_bin: 0,
            bins: Vec::new(),
        };
    }

    let num_bins = target_bins.clamp(1, 500).min(total_lines);

    // Default keywords
    let default_err: &[&[u8]] = &[b"ERROR", b"FATAL", b"CRITICAL", b"SEVERE", b"ERRORE", b"PANIC", b"[E]", b"ERR "];
    let default_warn: &[&[u8]] = &[b"WARN", b"WARNING", b"ATTENZIONE", b"AVVISO", b"[W]"];

    let custom_err_bytes: Option<Vec<Vec<u8>>> = custom_error_keywords.map(|list| {
        list.iter().map(|s| s.as_bytes().to_vec()).collect()
    });
    let custom_warn_bytes: Option<Vec<Vec<u8>>> = custom_warn_keywords.map(|list| {
        list.iter().map(|s| s.as_bytes().to_vec()).collect()
    });

    let merged = (0..total_lines)
        .into_par_iter()
        .fold(
            || vec![(0u32, 0u32); num_bins],
            |mut local_bins, idx| {
                if let Some(content) = index.get_line_raw(idx) {
                    let check_len = content.len().min(128);
                    let slice = &content[..check_len];

                    let is_error = if let Some(ref list) = custom_err_bytes {
                        list.iter().any(|k| contains_subslice(slice, k))
                    } else {
                        default_err.iter().any(|&k| contains_subslice(slice, k))
                    };

                    let is_warn = if is_error {
                        false
                    } else if let Some(ref list) = custom_warn_bytes {
                        list.iter().any(|k| contains_subslice(slice, k))
                    } else {
                        default_warn.iter().any(|&k| contains_subslice(slice, k))
                    };

                    if is_error || is_warn {
                        let bin_idx = ((idx * num_bins) / total_lines).min(num_bins - 1);
                        if is_error {
                            local_bins[bin_idx].0 += 1;
                        } else {
                            local_bins[bin_idx].1 += 1;
                        }
                    }
                }
                local_bins
            },
        )
        .reduce(
            || vec![(0u32, 0u32); num_bins],
            |mut a, b| {
                for (i, (err, warn)) in b.into_iter().enumerate() {
                    a[i].0 += err;
                    a[i].1 += warn;
                }
                a
            },
        );

    let mut total_errors = 0usize;
    let mut total_warnings = 0usize;
    let mut max_errors_in_bin = 0u32;
    let mut max_warnings_in_bin = 0u32;

    let bins: Vec<HeatmapBin> = merged
        .into_iter()
        .enumerate()
        .map(|(i, (error_count, warn_count))| {
            total_errors += error_count as usize;
            total_warnings += warn_count as usize;
            if error_count > max_errors_in_bin {
                max_errors_in_bin = error_count;
            }
            if warn_count > max_warnings_in_bin {
                max_warnings_in_bin = warn_count;
            }

            let start_line = (i * total_lines) / num_bins + 1;
            let end_line = ((i + 1) * total_lines) / num_bins;
            HeatmapBin {
                error_count,
                warn_count,
                start_line,
                end_line: end_line.max(start_line),
            }
        })
        .collect();

    SeverityHeatmap {
        total_lines,
        total_errors,
        total_warnings,
        max_errors_in_bin,
        max_warnings_in_bin,
        bins,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_severity_heatmap() {
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(temp, "[INFO] Line 1 ok").unwrap();
        writeln!(temp, "[WARN] Line 2 warning").unwrap();
        writeln!(temp, "[ERROR] Line 3 fatal error").unwrap();
        writeln!(temp, "[INFO] Line 4 ok").unwrap();
        writeln!(temp, "[ERRORE] Line 5 errore bloccante").unwrap();
        writeln!(temp, "[ATTENZIONE] Line 6 attenzione").unwrap();
        temp.flush().unwrap();

        let (index, _) = LogIndex::open(temp.path()).unwrap();
        let heatmap = build_severity_heatmap(&index, 3, None, None);

        assert_eq!(heatmap.total_lines, 6);
        assert_eq!(heatmap.total_errors, 2);
        assert_eq!(heatmap.total_warnings, 2);
        assert_eq!(heatmap.bins.len(), 3);
    }

    #[test]
    fn test_extract_timestamp_iso() {
        let line = b"2026-10-28 12:34:56.789 [INFO] Server started";
        let ts = extract_timestamp(line).expect("should parse ISO timestamp");
        let expected = to_epoch_millis(2026, 10, 28, 12, 34, 56, 789);
        assert_eq!(ts, expected);
    }

    #[test]
    fn test_extract_timestamp_bracketed_iso() {
        let line = b"[2026-10-28T14:20:00Z] [ERROR] Connection refused";
        let ts = extract_timestamp(line).expect("should parse bracketed ISO");
        let expected = to_epoch_millis(2026, 10, 28, 14, 20, 0, 0);
        assert_eq!(ts, expected);
    }

    #[test]
    fn test_extract_timestamp_time_only() {
        let line = b"15:30:45.123 [WARN] High memory usage";
        let ts = extract_timestamp(line).expect("should parse time-only");
        let expected = (15 * 3600 + 30 * 60 + 45) * 1000 + 123;
        assert_eq!(ts, expected);
    }

    #[test]
    fn test_extract_timestamp_syslog() {
        let line = b"Oct 28 09:15:30 myhost myapp: User login";
        let ts = extract_timestamp(line).expect("should parse syslog");
        let expected = to_epoch_millis(2026, 10, 28, 9, 15, 30, 0);
        assert_eq!(ts, expected);
    }

    #[test]
    fn test_same_second_histogram() {
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(temp, "20:15:01.513 [INFO] Handled request 1").unwrap();
        writeln!(temp, "20:15:01.525 [DEBUG] Cache hit").unwrap();
        writeln!(temp, "20:15:01.537 [WARN] High latency").unwrap();
        writeln!(temp, "20:15:01.813 [INFO] Handled request 2").unwrap();
        temp.flush().unwrap();

        let (index, _) = LogIndex::open(temp.path()).unwrap();
        let hist = build_histogram(&index, None, 80);
        assert!(hist.has_timestamps);
        assert_eq!(hist.lines_with_timestamp, 4);
    }

    #[test]
    fn test_detect_severity() {
        assert_eq!(detect_severity(b"2026-10-28 12:00:00 [ERROR] Failed to connect"), Severity::Error);
        assert_eq!(detect_severity(b"2026-10-28 12:00:00 [WARN] Disk 80% full"), Severity::Warn);
        assert_eq!(detect_severity(b"2026-10-28 12:00:00 [INFO] Request completed"), Severity::Info);
    }

    #[test]
    fn test_build_histogram_integration() {
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(temp, "2026-10-28 10:00:00 [INFO] Start").unwrap();
        writeln!(temp, "2026-10-28 10:15:00 [WARN] Warning line").unwrap();
        writeln!(temp, "2026-10-28 10:30:00 [ERROR] Fatal crash").unwrap();
        writeln!(temp, "2026-10-28 10:45:00 [INFO] Recovered").unwrap();
        writeln!(temp, "2026-10-28 11:00:00 [INFO] End").unwrap();
        temp.flush().unwrap();

        let (index, _) = LogIndex::open(temp.path()).unwrap();
        let hist = build_histogram(&index, None, 4);

        assert!(hist.has_timestamps);
        assert_eq!(hist.total_lines, 5);
        assert_eq!(hist.lines_with_timestamp, 5);
        assert_eq!(hist.buckets.len(), 4);

        let total_count: u32 = hist.buckets.iter().map(|b| b.count).sum();
        assert_eq!(total_count, 5);

        let error_count: u32 = hist.buckets.iter().map(|b| b.error_count).sum();
        assert_eq!(error_count, 1);

        let warn_count: u32 = hist.buckets.iter().map(|b| b.warn_count).sum();
        assert_eq!(warn_count, 1);
    }
}
