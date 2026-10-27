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
}
