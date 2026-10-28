export interface LogLine {
  line_number: number;
  content: string;
  byte_offset: number;
  is_stacktrace?: boolean;
  is_collapsed_group?: boolean;
  group_id?: number | null;
  group_line_count?: number | null;
}

export interface FileOpenResult {
  tab_id: string;
  path: string;
  file_name: string;
  file_size: number;
  total_lines: number;
  total_display_rows?: number;
  index_time_ms: number;
}

export interface LogAppendedPayload {
  tab_id: string;
  total_lines: number;
  file_size: number;
}

export interface SearchOptions {
  query: string;
  is_regex: boolean;
  case_sensitive: boolean;
  invert: boolean;
}

export interface SearchResult {
  query_id: number;
  total_matches: number;
  total_lines: number;
  search_time_ms: number;
  match_lines: number[];
  error?: string | null;
}

export interface FilterUpdatedPayload {
  tab_id: string;
  total_matches: number;
}

export type SearchDisplayMode = "filter" | "highlight";

export type LogLevel = "ERROR" | "WARN" | "INFO" | "DEBUG" | "TRACE";

export interface FoldingStatus {
  total_groups: number;
  total_collapsed_lines: number;
  auto_collapse_enabled: boolean;
}

export interface ExtractedPayload {
  type: "json" | "xml" | "text";
  raw: string;
  parsed?: any;
  formatted?: string;
  error?: string | null;
}

export interface TimeBucket {
  start_time: number;
  end_time: number;
  label: string;
  count: number;
  error_count: number;
  warn_count: number;
  info_count: number;
  start_line: number;
  end_line: number;
}

export interface TimeHistogram {
  buckets: TimeBucket[];
  total_lines: number;
  lines_with_timestamp: number;
  min_time: number;
  max_time: number;
  min_time_str: string;
  max_time_str: string;
  has_timestamps: boolean;
}

export interface TimelineBrushRange {
  startIndex: number;
  endIndex: number;
  startTime: number;
  endTime: number;
  startLine: number;
  endLine: number;
  totalCount: number;
  errorCount: number;
  warnCount: number;
}

export interface HeatmapBin {
  error_count: number;
  warn_count: number;
  start_line: number;
  end_line: number;
}

export interface SeverityHeatmap {
  total_lines: number;
  total_errors: number;
  total_warnings: number;
  max_errors_in_bin: number;
  max_warnings_in_bin: number;
  bins: HeatmapBin[];
}

export interface LogTab {
  id: string;
  path: string;
  fileName: string;
  fileSize: number;
  totalLines: number;
  displayTotalRows: number;
  indexTimeMs: number;
  // Live Tail State
  isLive: boolean;
  unreadLines: number;
  pausedAtLine: number;
  // Search State
  searchQuery: string;
  isRegex: boolean;
  caseSensitive: boolean;
  invert: boolean;
  searchMode: SearchDisplayMode;
  totalMatches: number;
  searchTimeMs: number;
  matchLines: number[];
  currentMatchLine: number | null;
  currentMatchIndex: number;
  searchError: string | null;
  // Folding State
  foldingStatus: FoldingStatus;
  // Selection State
  selectedLine: LogLine | null;
  selectedLineNumbers: Set<number>;
  lastAnchorLineNum: number | null;
  lastScrollIndex?: number;
  // P1 View & Investigation State
  wrapLines: boolean;
  bookmarks: number[];
  // P2 Timeline & Temporal State
  isTimelineOpen: boolean;
  timelineHistogram: TimeHistogram | null;
  heatmap?: SeverityHeatmap | null;
}
