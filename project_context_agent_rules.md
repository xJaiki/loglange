# Project Context: Ultra-Fast Minimalist Real-Time Log Viewer

## 1. Vision & Core Philosophy
We are building a **hyper-fast, minimalist, cross-platform desktop log viewer** (Windows, macOS, Linux).
- **Primary Goal:** Open multi-gigabyte log files instantly (<100ms) and tail high-frequency real-time streams (10k+ lines/sec) at a locked 120fps with near-zero RAM footprint.
- **Aesthetic & UX:** Inspired by **Zed**, **Raycast**, and **Linear**. Keyboard-first, zero clutter, high-contrast warm dark mode, crisp monospace typography, no legacy menus or bloated toolbars.

---

## 2. Tech Stack (Strict)
- **Core / Backend:** **Rust** (Edition 2021+)
- **Application Shell:** **Tauri v2**
- **Frontend Framework:** **Svelte 5 (Runes)** + **TypeScript** + **Vite**
- **Styling:** **Tailwind CSS v4** (Custom warm-dark palette, zero heavy component libraries)
- **Key Rust Crates:**
  - `memmap2`: Memory-mapped file reading (NEVER load full files into heap memory).
  - `memchr`: SIMD-accelerated newline (`\n`) scanning.
  - `notify`: Cross-platform non-blocking file system watcher for real-time tailing.
  - `grep-regex` & `grep-matcher` (or `regex` with bytes): SIMD fast regex filtering.
  - `serde` / `bincode` (or Tauri v2 raw IPC channels): Low-overhead frontend-backend data transfer.

---

## 3. Non-Negotiable Architecture Rules ("Zero-Lag Rules")
1. **NEVER load the full file into memory ( neither in Rust heap nor in JS Frontend).**
   - Always use `memmap2::Mmap` to map the file on disk.
2. **Line Offset Indexing:**
   - Maintain an in-memory index of line byte offsets: `Vec<u64>`.
   - Scan for `\n` using `memchr::memchr_iter` on background worker threads.
3. **Viewport-Only IPC (Virtualization):**
   - The Svelte frontend only renders the visible viewport (e.g., 50–80 DOM nodes max) using fixed-height virtual scrolling.
   - The frontend requests ONLY the visible slice via Tauri command: `get_lines_slice(start_line: usize, count: usize) -> Vec<LogLine>`.
   - Rust slices the `Mmap` bytes using the offset index, converts only those 50–80 lines to UTF-8 (`String::from_utf8_lossy`), and returns them.
4. **Incremental Real-Time Tailing:**
   - When `notify` detects a file append, read ONLY from the last known byte offset to the new EOF.
   - Append new newline offsets to the index vector and emit a lightweight Tauri event (`log:appended { total_lines }`) throttled to max once per 16ms (60Hz/120Hz frame pacing).
5. **Non-Blocking Filtering:**
   - Search and Regex filtering run entirely in Rust on background threads over the `Mmap` buffer.
   - Output is a secondary index `filtered_indices: Vec<u32>` mapping visible rows to original line numbers. Cancel ongoing searches immediately if the user types a new character.

---

## 4. UI / UX Design System
- **Color Palette:**
  - Background Primary: `#0D0E11`
  - Background Elevated (Drawers/Modals): `#14161B`
  - Border / Hairline: `#22252D`
  - Text Primary: `#E4E6EB` | Text Muted: `#7A8191`
  - Log Levels: `ERROR` (`#FF5D62`), `WARN` (`#E6A93B`), `INFO` (`#58A6FF`), `DEBUG` (`#8B949E`).
- **Typography:** Monospace everywhere for logs (`JetBrains Mono`, `Geist Mono`, or system monospace fallback). Strict fixed `line-height` (e.g., `22px`) to guarantee O(1) virtual scroll math.
- **3-Zone Layout:**
  1. **Floating Command Palette (`Cmd/Ctrl + K`):** Open files, switch tabs, toggle settings, jump to line/timestamp.
  2. **Main Virtual Canvas (95% screen):** Line numbers gutter, log text, collapsible stacktraces, and a custom right-side **Minimap Scrollbar** showing colored ticks for errors and search matches.
  3. **Bottom Status & Filter Pill:** Vim-style `/` quick filter input, regex toggle, Filter vs Highlight mode switch, and a live status indicator (`LIVE` pulsing green dot vs `PAUSED` amber badge with unread line count).

---

## 5. Essential Features ("The Right Tools")
1. **Smart Tail Lock:** Auto-scrolls at the bottom when `LIVE`. Scrolling up 1px with mouse wheel or selecting text immediately pauses auto-scroll. Pressing `Space` or clicking the floating badge jumps back to bottom and resumes `LIVE`.
2. **Stacktrace Auto-Collapsing:** Consecutive lines starting with whitespace + `at ` or `Caused by:` are grouped into an expandable inline pill (`[+ N stacktrace lines]`).
3. **Inline JSON/XML Inspector Drawer:** Clicking a log line containing a JSON object extracts and pretty-prints the JSON in a clean slide-over side panel with one-click key/value copy.
4. **Dual Search Mode:**
   - *Filter Mode:* Hides non-matching lines.
   - *Highlight Mode:* Keeps all lines visible for context but highlights matches and marks them on the scrollbar.

---

## 6. Agent Execution Roadmap (Step-by-Step)
- **Step 1 (Foundation & Zero-Copy Reader):** Scaffold Tauri v2 + Svelte 5 + Tailwind. Implement `indexer.rs` (`memmap2` + `memchr`) and `get_lines_slice` IPC. Build the fixed-height `VirtualList.svelte` component. Verify opening a 1GB+ file in <100ms.
- **Step 2 (Live Watcher & Smart Tail):** Implement `watcher.rs` with incremental offset indexing and 16ms throttled events. Implement Smart Tail Lock in Svelte.
- **Step 3 (SIMD Search & Filtering):** Implement background regex/substring matcher in Rust producing `filtered_indices`. Connect the bottom `/` filter bar with Filter-Out and Highlight modes.
- **Step 4 (Log Ergonomics):** Add log-level tokenizer, stacktrace folding logic, and the right-hand JSON pretty-printer drawer.
- **Step 5 (Keyboard UX, Stdin & Packaging):** Add `Cmd+K` Command Palette, file drag-and-drop, CLI argument/stdin pipe support, and cross-platform build configuration.

## 7. Coding Guidelines for the AI Agent
- Write idiomatic, safe Rust. Use `Arc<RwLock<...>>` or lock-free structures for sharing state between the file watcher thread, search threads, and Tauri IPC handlers.
- Keep Svelte 5 components ultra-lean using `$state` and `$derived` runes. Avoid unnecessary DOM wrappers inside virtual row items.
- Do not implement steps ahead of schedule. Complete and verify one step at a time.