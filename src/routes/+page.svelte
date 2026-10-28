<script lang="ts">
  import { onMount } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import VirtualList from "$lib/components/VirtualList.svelte";
  import FilterBar from "$lib/components/FilterBar.svelte";
  import JsonDrawer from "$lib/components/JsonDrawer.svelte";
  import CommandPalette from "$lib/components/CommandPalette.svelte";
  import TabBar from "$lib/components/TabBar.svelte";
  import LogLevelsModal from "$lib/components/LogLevelsModal.svelte";
  import TimelineHistogram from "$lib/components/TimelineHistogram.svelte";
  import ViewMenu from "$lib/components/ViewMenu.svelte";
  import { logLevelsManager } from "$lib/stores/logLevels.svelte";
  import type {
    FileOpenResult,
    FilterUpdatedPayload,
    FoldingStatus,
    LogAppendedPayload,
    LogLine,
    LogTab,
    SearchDisplayMode,
    SearchResult,
    SeverityHeatmap,
    TimeHistogram,
    TimelineBrushRange
  } from "$lib/types";
  import type { DeltaInfo } from "$lib/utils/timeDelta";

  // Multi-tab state
  let tabs = $state<LogTab[]>([]);
  let activeTabId = $state<string | null>(null);

  // Active tab derived helper
  let activeTab = $derived(tabs.find((t) => t.id === activeTabId) ?? null);

  // Derived properties from active tab
  let currentFile = $derived<FileOpenResult | null>(
    activeTab
      ? {
          tab_id: activeTab.id,
          path: activeTab.path,
          file_name: activeTab.fileName,
          file_size: activeTab.fileSize,
          total_lines: activeTab.totalLines,
          total_display_rows: activeTab.displayTotalRows,
          index_time_ms: activeTab.indexTimeMs
        }
      : null
  );

  let totalLines = $derived(activeTab ? activeTab.totalLines : 0);
  let displayTotalRows = $derived(activeTab ? activeTab.displayTotalRows : 0);
  let isLive = $derived(activeTab ? activeTab.isLive : true);
  let unreadLines = $derived(activeTab ? activeTab.unreadLines : 0);
  let searchQuery = $derived(activeTab ? activeTab.searchQuery : "");
  let isRegex = $derived(activeTab ? activeTab.isRegex : false);
  let caseSensitive = $derived(activeTab ? activeTab.caseSensitive : false);
  let invert = $derived(activeTab ? activeTab.invert : false);
  let searchMode = $derived<SearchDisplayMode>(activeTab ? activeTab.searchMode : "filter");
  let totalMatches = $derived(activeTab ? activeTab.totalMatches : 0);
  let searchTimeMs = $derived(activeTab ? activeTab.searchTimeMs : 0);
  let matchLines = $derived<number[]>(activeTab ? activeTab.matchLines : []);
  let currentMatchLine = $derived<number | null>(activeTab ? activeTab.currentMatchLine : null);
  let currentMatchIndex = $derived(activeTab ? activeTab.currentMatchIndex : 0);
  let searchError = $derived<string | null>(activeTab ? activeTab.searchError : null);
  let selectedLine = $derived<LogLine | null>(activeTab ? activeTab.selectedLine : null);
  let selectedLineNumbers = $derived<Set<number>>(activeTab ? activeTab.selectedLineNumbers : new Set<number>());
  let wrapLines = $derived(activeTab ? activeTab.wrapLines : false);
  let bookmarks = $derived<number[]>(activeTab ? activeTab.bookmarks : []);
  let isTimelineOpen = $derived<boolean>(Boolean(activeTab && activeTab.isTimelineOpen));
  let timelineHistogram = $state<TimeHistogram | null>(null);
  let isTimelineLoading = $state(false);
  let currentHeatmap = $state<SeverityHeatmap | null>(null);
  let currentTimeDelta = $state<DeltaInfo | null>(null);
  let foldingStatus = $derived<FoldingStatus>(
    activeTab
      ? activeTab.foldingStatus
      : {
          total_groups: 0,
          total_collapsed_lines: 0,
          auto_collapse_enabled: true
        }
  );

  // Global UI & App State
  let isLoading = $state(false);
  let errorMessage = $state<string | null>(null);
  let isDragging = $state(false);
  let virtualListRef = $state<VirtualList | null>(null);
  let filterBarRef = $state<FilterBar | null>(null);
  let isAtBottom = $state(true);
  let liveAnimations = $state(true);

  // Search Debouncing
  let isSearching = $state(false);
  let searchDebounceTimer: ReturnType<typeof setTimeout> | null = null;

  // Drawer UI state
  let isDrawerOpen = $state(false);
  let isDrawerPinned = $state(false);

  // Command Palette & Toasts
  let isCommandPaletteOpen = $state(false);
  let isLogLevelsModalOpen = $state(false);
  let toastMessage = $state<string | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | null = null;
  let copiedSnippet = $state(false);

  function showToast(msg: string) {
    toastMessage = msg;
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
      toastMessage = null;
    }, 2500);
  }

  // Quick Filter Presets (UI/UX Revamp)
  type QuickFilterPreset = "all" | "errors" | "warnings" | "json";

  let activeQuickPreset = $derived.by<QuickFilterPreset | null>(() => {
    if (!searchQuery) return "all";
    const errRx = logLevelsManager.getFilterRegex("ERROR");
    const warnRx = logLevelsManager.getFilterRegex("WARN");
    if (isRegex && errRx && searchQuery === errRx) return "errors";
    if (isRegex && warnRx && searchQuery === warnRx) return "warnings";
    if (!isRegex && searchQuery === "{") return "json";
    return null;
  });

  function applyQuickPreset(preset: QuickFilterPreset) {
    if (!activeTab) return;
    if (preset === "all") {
      clearSearch();
      return;
    }
    activeTab.searchMode = "filter";
    activeTab.caseSensitive = false;
    activeTab.invert = false;
    if (preset === "errors") {
      activeTab.isRegex = true;
      activeTab.searchQuery = logLevelsManager.getFilterRegex("ERROR");
    } else if (preset === "warnings") {
      activeTab.isRegex = true;
      activeTab.searchQuery = logLevelsManager.getFilterRegex("WARN");
    } else if (preset === "json") {
      activeTab.isRegex = false;
      activeTab.searchQuery = "{";
    }
    scheduleSearch(true);
  }

  async function copyCliSnippet() {
    try {
      await navigator.clipboard.writeText("cat app.log | loglance");
      copiedSnippet = true;
      showToast("Comando copiato negli appunti");
      setTimeout(() => {
        copiedSnippet = false;
      }, 1800);
    } catch {
      showToast("cat app.log | loglance");
    }
  }

  // Derived effective line count for virtual list
  let isFilterActive = $derived(Boolean(activeTab && activeTab.searchQuery.trim().length > 0 && activeTab.searchMode === "filter"));
  let effectiveTotalLines = $derived(activeTab ? (isFilterActive ? activeTab.totalMatches : activeTab.displayTotalRows) : 0);

  function formatBytes(bytes: number): string {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  }

  function pauseLive() {
    if (!activeTab || !activeTab.isLive) return;
    activeTab.isLive = false;
    activeTab.pausedAtLine = activeTab.totalLines;
    activeTab.unreadLines = 0;
  }

  function resumeLive() {
    if (!activeTab) return;
    activeTab.isLive = true;
    activeTab.unreadLines = 0;
    activeTab.pausedAtLine = activeTab.totalLines;
    requestAnimationFrame(() => {
      virtualListRef?.scrollToBottom(false);
    });
  }

  function toggleLive() {
    if (!activeTab) return;
    if (activeTab.isLive) {
      pauseLive();
    } else {
      resumeLive();
    }
  }

  function toggleLiveAnimations() {
    liveAnimations = !liveAnimations;
    try {
      localStorage.setItem("logviewer:liveAnimations", String(liveAnimations));
    } catch {}
    showToast(`Animazioni live: ${liveAnimations ? "attivate" : "disattivate"}`);
  }

  // --- Tab Management ---

  async function switchTab(tabId: string) {
    if (activeTabId === tabId) return;
    const target = tabs.find((t) => t.id === tabId);
    if (!target) return;

    activeTabId = tabId;
    target.unreadLines = 0;
    if (target.isLive) {
      target.pausedAtLine = target.totalLines;
    }

    try {
      await invoke("switch_tab", { tabId });
    } catch (e) {
      console.warn("Failed to switch tab backend session:", e);
    }

    // Refresh row count and folding status to guarantee consistency if appends occurred in background
    invoke<number>("get_display_row_count", { tabId }).then((rowCount) => {
      if (activeTabId === tabId && target) target.displayTotalRows = rowCount;
    });
    invoke<FoldingStatus>("get_folding_status", { tabId }).then((status) => {
      if (activeTabId === tabId && target) target.foldingStatus = status;
    });

    timelineHistogram = target.timelineHistogram ?? null;
    if (target.isTimelineOpen && !target.timelineHistogram) {
      fetchTimeline(target);
    }

    currentHeatmap = target.heatmap ?? null;
    if (!target.heatmap) {
      fetchHeatmap(target);
    }

    virtualListRef?.clearCache();

    requestAnimationFrame(() => {
      if (target.isLive) {
        virtualListRef?.scrollToBottom(false);
      } else if (target.lastScrollIndex !== undefined) {
        virtualListRef?.scrollToIndex(target.lastScrollIndex, false);
      }
    });
  }

  async function closeTab(tabIdToClose: string) {
    const index = tabs.findIndex((t) => t.id === tabIdToClose);
    if (index === -1) return;

    try {
      await invoke("close_tab", { tabId: tabIdToClose });
    } catch (e) {
      console.warn("Failed to close tab backend session:", e);
    }

    tabs = tabs.filter((t) => t.id !== tabIdToClose);

    if (activeTabId === tabIdToClose) {
      if (tabs.length > 0) {
        const nextIndex = Math.min(index, tabs.length - 1);
        switchTab(tabs[nextIndex].id);
      } else {
        activeTabId = null;
        virtualListRef?.clearCache();
        isDrawerOpen = false;
      }
    }
  }

  async function closeOtherTabs(keepTabId: string) {
    const toClose = tabs.filter((t) => t.id !== keepTabId);
    for (const tab of toClose) {
      await closeTab(tab.id);
    }
  }

  async function openFilePath(path: string) {
    // Check if file is already open in an existing tab
    const existing = tabs.find((t) => t.path === path);
    if (existing) {
      switchTab(existing.id);
      showToast(`Scheda già aperta: ${existing.fileName}`);
      return;
    }

    isLoading = true;
    errorMessage = null;

    try {
      const res = await invoke<FileOpenResult>("open_file", { path });
      const fStatus = await invoke<FoldingStatus>("get_folding_status", { tabId: res.tab_id });

      const newTab: LogTab = {
        id: res.tab_id,
        path: res.path,
        fileName: res.file_name,
        fileSize: res.file_size,
        totalLines: res.total_lines,
        displayTotalRows: res.total_display_rows ?? res.total_lines,
        indexTimeMs: res.index_time_ms,
        isLive: true,
        unreadLines: 0,
        pausedAtLine: res.total_lines,
        searchQuery: "",
        isRegex: false,
        caseSensitive: false,
        invert: false,
        searchMode: "filter",
        totalMatches: 0,
        searchTimeMs: 0,
        matchLines: [],
        currentMatchLine: null,
        currentMatchIndex: 0,
        searchError: null,
        foldingStatus: fStatus,
        selectedLine: null,
        selectedLineNumbers: new Set<number>(),
        lastAnchorLineNum: null,
        wrapLines: localStorage.getItem("logviewer:wrapLines") === "true",
        bookmarks: [],
        isTimelineOpen: localStorage.getItem("logviewer:isTimelineOpen") === "true",
        timelineHistogram: null,
        heatmap: null
      };

      tabs = [...tabs, newTab];
      activeTabId = newTab.id;
      timelineHistogram = null;
      currentHeatmap = null;

      fetchHeatmap(newTab);

      if (newTab.isTimelineOpen) {
        fetchTimeline(newTab);
      }

      virtualListRef?.clearCache();
      requestAnimationFrame(() => {
        virtualListRef?.scrollToBottom(false);
      });
    } catch (err: any) {
      errorMessage = typeof err === "string" ? err : err?.message || "Failed to open file";
      console.error(err);
    } finally {
      isLoading = false;
    }
  }

  async function handleOpenFilePicker() {
    try {
      const selected = await openDialog({
        multiple: true,
        directory: false,
        filters: [
          {
            name: "Log & Text Files",
            extensions: ["log", "txt", "out", "json", "jsonl", "*"]
          }
        ]
      });

      if (selected) {
        const filePaths = Array.isArray(selected) ? selected : [selected];
        for (const p of filePaths) {
          if (typeof p === "string") {
            await openFilePath(p);
          }
        }
      }
    } catch (err: any) {
      errorMessage = typeof err === "string" ? err : err?.message || "Dialog error";
    }
  }

  async function fetchSlice(start: number, count: number): Promise<LogLine[]> {
    if (!activeTab) return [];
    const tabId = activeTab.id;
    try {
      if (isFilterActive) {
        return await invoke<LogLine[]>("get_filtered_lines_slice", {
          tabId,
          startRow: start,
          count
        });
      } else {
        return await invoke<LogLine[]>("get_lines_slice", {
          tabId,
          startLine: start,
          count
        });
      }
    } catch (err) {
      console.error("Failed to fetch slice:", err);
      return [];
    }
  }

  // --- Step 4: Stacktrace Folding Actions ---

  async function handleToggleStacktraceGroup(groupId: number) {
    if (!activeTab) return;
    const tabId = activeTab.id;
    try {
      const res = await invoke<[boolean, number]>("toggle_stacktrace_group", { tabId, groupId });
      if (activeTab && activeTab.id === tabId) {
        activeTab.displayTotalRows = res[1];
        virtualListRef?.clearCache();
        const status = await invoke<FoldingStatus>("get_folding_status", { tabId });
        if (activeTab && activeTab.id === tabId) {
          activeTab.foldingStatus = status;
        }
      }
    } catch (err) {
      console.error("Failed to toggle stacktrace:", err);
    }
  }

  async function toggleAutoCollapseStacktraces() {
    if (!activeTab) return;
    const tabId = activeTab.id;
    try {
      const nextState = !activeTab.foldingStatus.auto_collapse_enabled;
      const newTotal = await invoke<number>("set_auto_collapse_stacktraces", { tabId, enabled: nextState });
      if (activeTab && activeTab.id === tabId) {
        activeTab.displayTotalRows = newTotal;
        activeTab.foldingStatus.auto_collapse_enabled = nextState;
        const status = await invoke<FoldingStatus>("get_folding_status", { tabId });
        if (activeTab && activeTab.id === tabId) {
          activeTab.foldingStatus = status;
        }
        virtualListRef?.clearCache();
      }
    } catch (err) {
      console.error("Failed to toggle auto collapse:", err);
    }
  }

  // --- Step 4: Line Selection, Highlighting & Drawer Actions ---

  function handleLineClick(line: LogLine, isCtrlOrMeta: boolean = false, isShift: boolean = false) {
    if (!activeTab) return;
    activeTab.selectedLine = line;
    if (isShift && activeTab.lastAnchorLineNum !== null) {
      const start = Math.min(activeTab.lastAnchorLineNum, line.line_number);
      const end = Math.max(activeTab.lastAnchorLineNum, line.line_number);
      const next = isCtrlOrMeta ? new Set(activeTab.selectedLineNumbers) : new Set<number>();
      for (let n = start; n <= end; n++) {
        next.add(n);
      }
      activeTab.selectedLineNumbers = next;
    } else if (isCtrlOrMeta) {
      const next = new Set(activeTab.selectedLineNumbers);
      if (next.has(line.line_number)) {
        next.delete(line.line_number);
      } else {
        next.add(line.line_number);
      }
      activeTab.selectedLineNumbers = next;
      activeTab.lastAnchorLineNum = line.line_number;
    } else {
      activeTab.selectedLineNumbers = new Set([line.line_number]);
      activeTab.lastAnchorLineNum = line.line_number;
    }
  }

  function handleLineDoubleClick(line: LogLine) {
    if (!activeTab) return;
    activeTab.selectedLine = line;
    activeTab.selectedLineNumbers = new Set([line.line_number]);
    activeTab.lastAnchorLineNum = line.line_number;
    isDrawerOpen = true;
  }

  function handleSelectionRangeChange(lineNums: number[], isCtrlOrMeta: boolean) {
    if (!activeTab) return;
    if (isCtrlOrMeta) {
      const next = new Set(activeTab.selectedLineNumbers);
      for (const num of lineNums) {
        next.add(num);
      }
      activeTab.selectedLineNumbers = next;
    } else {
      activeTab.selectedLineNumbers = new Set(lineNums);
    }
    if (lineNums.length > 0) {
      const lastLine = virtualListRef?.getLineByNumber(lineNums[lineNums.length - 1]);
      if (lastLine) activeTab.selectedLine = lastLine;
      activeTab.lastAnchorLineNum = lineNums[lineNums.length - 1];
    }
  }

  function toggleDrawerPin() {
    isDrawerPinned = !isDrawerPinned;
    try {
      localStorage.setItem("logviewer:drawerPinned", String(isDrawerPinned));
    } catch {}
    showToast(isDrawerPinned ? "Sidebar affiancata come colonna" : "Sidebar in overlay");
  }

  async function handleDrawerPrevLine() {
    if (!activeTab || !activeTab.selectedLine) return;
    const currentNum = activeTab.selectedLine.line_number;
    if (currentNum <= 1) return;
    const prevLine = virtualListRef?.getLineByNumber(currentNum - 1);
    if (prevLine) {
      activeTab.selectedLine = prevLine;
      activeTab.selectedLineNumbers = new Set([prevLine.line_number]);
    } else {
      try {
        const slice = await invoke<LogLine[]>("get_lines_slice", {
          tabId: activeTab.id,
          startLine: Math.max(0, currentNum - 2),
          count: 1
        });
        if (slice.length > 0 && activeTab) {
          activeTab.selectedLine = slice[0];
          activeTab.selectedLineNumbers = new Set([slice[0].line_number]);
        }
      } catch (e) {
        console.error(e);
      }
    }
  }

  async function handleDrawerNextLine() {
    if (!activeTab || !activeTab.selectedLine) return;
    const currentNum = activeTab.selectedLine.line_number;
    if (currentNum >= totalLines) return;
    const nextLine = virtualListRef?.getLineByNumber(currentNum + 1);
    if (nextLine) {
      activeTab.selectedLine = nextLine;
      activeTab.selectedLineNumbers = new Set([nextLine.line_number]);
    } else {
      try {
        const slice = await invoke<LogLine[]>("get_lines_slice", {
          tabId: activeTab.id,
          startLine: currentNum,
          count: 1
        });
        if (slice.length > 0 && activeTab) {
          activeTab.selectedLine = slice[0];
          activeTab.selectedLineNumbers = new Set([slice[0].line_number]);
        }
      } catch (e) {
        console.error(e);
      }
    }
  }

  // --- Step 5: Command Palette Actions ---

  async function handlePaletteAction(actionId: string, payload?: any) {
    switch (actionId) {
      case "jump_to_top":
        pauseLive();
        virtualListRef?.scrollToTop();
        break;
      case "jump_to_bottom":
        resumeLive();
        virtualListRef?.scrollToBottom();
        break;
      case "jump_to_line":
        if (typeof payload === "number" && activeTab) {
          pauseLive();
          try {
            const displayRow = await invoke<number>("map_line_to_display_row", {
              tabId: activeTab.id,
              lineNumber: payload
            });
            virtualListRef?.scrollToIndex(displayRow, true);
            showToast(`Jumped to line ${payload.toLocaleString()}`);
          } catch (e) {
            console.error(e);
          }
        }
        break;
      case "jump_to_timestamp":
        if (typeof payload === "string" && activeTab) {
          try {
            const line = await invoke<number | null>("find_timestamp_line", {
              tabId: activeTab.id,
              query: payload
            });
            if (line) {
              pauseLive();
              const displayRow = await invoke<number>("map_line_to_display_row", {
                tabId: activeTab.id,
                lineNumber: line
              });
              virtualListRef?.scrollToIndex(displayRow, true);
              showToast(`Jumped to line ${line.toLocaleString()} ("${payload}")`);
            } else {
              showToast(`No timestamp found matching "${payload}"`);
            }
          } catch (e) {
            console.error(e);
          }
        }
        break;
      case "next_match":
        handleNextMatch();
        break;
      case "prev_match":
        handlePrevMatch();
        break;
      case "toggle_live":
        toggleLive();
        break;
      case "toggle_live_animations":
        toggleLiveAnimations();
        break;
      case "toggle_folding":
        toggleAutoCollapseStacktraces();
        break;
      case "expand_all_stacktraces":
        if (activeTab) {
          try {
            const rows = await invoke<number>("expand_all_stacktraces", { tabId: activeTab.id });
            activeTab.displayTotalRows = rows;
            const status = await invoke<FoldingStatus>("get_folding_status", { tabId: activeTab.id });
            activeTab.foldingStatus = status;
            virtualListRef?.clearCache();
            showToast("All stacktraces expanded");
          } catch (e) {
            console.error(e);
          }
        }
        break;
      case "collapse_all_stacktraces":
        if (activeTab) {
          try {
            const rows = await invoke<number>("collapse_all_stacktraces", { tabId: activeTab.id });
            activeTab.displayTotalRows = rows;
            const status = await invoke<FoldingStatus>("get_folding_status", { tabId: activeTab.id });
            activeTab.foldingStatus = status;
            virtualListRef?.clearCache();
            showToast("All stacktraces collapsed");
          } catch (e) {
            console.error(e);
          }
        }
        break;
      case "toggle_drawer":
        isDrawerOpen = !isDrawerOpen;
        break;
      case "toggle_drawer_pin":
        toggleDrawerPin();
        break;
      case "focus_filter":
        filterBarRef?.focusInput();
        break;
      case "toggle_search_mode":
        handleModeChange(searchMode === "filter" ? "highlight" : "filter");
        break;
      case "toggle_regex":
        handleToggleRegex();
        break;
      case "toggle_case":
        handleToggleCase();
        break;
      case "toggle_invert":
        handleToggleInvert();
        break;
      case "clear_filter":
        clearSearch();
        break;
      case "open_file":
        handleOpenFilePicker();
        break;
      case "reload_file":
        if (activeTab) {
          const p = activeTab.path;
          const id = activeTab.id;
          await closeTab(id);
          await openFilePath(p);
          showToast(`Ricaricato ${activeTab?.fileName ?? p}`);
        }
        break;
      case "copy_file_path":
        if (activeTab) {
          try {
            await navigator.clipboard.writeText(activeTab.path);
            showToast("Percorso file copiato negli appunti");
          } catch {
            showToast(activeTab.path);
          }
        }
        break;
      case "close_current_tab":
        if (activeTabId) {
          closeTab(activeTabId);
        }
        break;
      case "close_other_tabs":
        if (activeTabId) {
          closeOtherTabs(activeTabId);
        }
        break;
      case "switch_to_tab":
        if (typeof payload === "string") {
          switchTab(payload);
        }
        break;
      case "configure_log_levels":
        isLogLevelsModalOpen = true;
        break;
      case "toggle_wrap":
        toggleWrapLines();
        break;
      case "toggle_bookmark":
        if (selectedLine) {
          toggleBookmark(selectedLine.line_number);
        } else {
          showToast("Seleziona prima una riga da contrassegnare (Ctrl+B)");
        }
        break;
      case "next_bookmark":
        jumpToNextBookmark();
        break;
      case "prev_bookmark":
        jumpToPrevBookmark();
        break;
      case "clear_all_bookmarks":
        clearAllBookmarks();
        break;
      case "toggle_timeline":
        toggleTimeline();
        break;
      case "export_filtered":
        handleExportFilteredLines();
        break;
      case "copy_filtered":
        handleCopyAllFilteredLines();
        break;
      case "copy_selected_lines":
        copySelectedLines(false);
        break;
      case "copy_selected_lines_with_numbers":
        copySelectedLines(true);
        break;
      case "select_all_lines":
        handleSelectAllLines();
        break;
    }
  }

  // --- Step 3: SIMD Search & Filter Engine Logic ---

  function scheduleSearch(immediate = false) {
    if (searchDebounceTimer) clearTimeout(searchDebounceTimer);
    if (immediate) {
      performSearch();
    } else {
      searchDebounceTimer = setTimeout(performSearch, 50);
    }
  }

  async function performSearch() {
    if (!activeTab) return;

    if (activeTab.searchQuery.trim().length === 0) {
      clearSearch();
      return;
    }

    isSearching = true;
    activeTab.searchError = null;
    const tabId = activeTab.id;

    try {
      const res = await invoke<SearchResult>("search_logs", {
        tabId,
        options: {
          query: activeTab.searchQuery,
          is_regex: activeTab.isRegex,
          case_sensitive: activeTab.caseSensitive,
          invert: activeTab.invert
        }
      });

      if (activeTab && activeTab.id === tabId) {
        activeTab.totalMatches = res.total_matches;
        activeTab.searchTimeMs = res.search_time_ms;
        activeTab.matchLines = res.match_lines;
        activeTab.searchError = res.error || null;
        virtualListRef?.clearCache();

        if (res.total_matches > 0) {
          if (activeTab.searchMode === "highlight") {
            activeTab.currentMatchIndex = 1;
            activeTab.currentMatchLine = activeTab.matchLines[0] || 1;
            virtualListRef?.scrollToIndex((activeTab.matchLines[0] || 1) - 1);
          } else {
            virtualListRef?.scrollToTop();
          }
        } else {
          activeTab.currentMatchIndex = 0;
          activeTab.currentMatchLine = null;
        }

        if (activeTab.isTimelineOpen) {
          fetchTimeline(activeTab, true);
        }
      }
    } catch (err: any) {
      const errStr = typeof err === "string" ? err : err?.message || "Search failed";
      if (!errStr.includes("Search cancelled") && activeTab && activeTab.id === tabId) {
        activeTab.searchError = errStr;
      }
    } finally {
      isSearching = false;
    }
  }

  async function clearSearch() {
    if (!activeTab) return;
    const tabId = activeTab.id;
    activeTab.searchQuery = "";
    activeTab.invert = false;
    activeTab.totalMatches = 0;
    activeTab.searchTimeMs = 0;
    activeTab.matchLines = [];
    activeTab.currentMatchLine = null;
    activeTab.currentMatchIndex = 0;
    activeTab.searchError = null;
    try {
      await invoke("clear_search", { tabId });
    } catch (e) {
      console.warn("Failed to clear search:", e);
    }
    if (activeTab.isTimelineOpen) {
      fetchTimeline(activeTab, false);
    }
    virtualListRef?.clearCache();
  }

  function handleSearchQueryChange(newQuery: string) {
    if (!activeTab) return;
    if (newQuery.startsWith("!") && newQuery.length > 1 && !activeTab.isRegex && !activeTab.invert) {
      activeTab.invert = true;
      activeTab.searchQuery = newQuery.slice(1);
      scheduleSearch();
      return;
    }
    activeTab.searchQuery = newQuery;
    scheduleSearch();
  }

  function handleToggleInvert() {
    if (!activeTab) return;
    activeTab.invert = !activeTab.invert;
    showToast(activeTab.invert ? "Filtro inverso (esclusione) attivato" : "Filtro normale (inclusione) attivato");
    scheduleSearch(true);
  }

  function handleFilterInclude(text: string) {
    if (!activeTab) return;
    activeTab.searchQuery = text;
    activeTab.isRegex = false;
    activeTab.invert = false;
    activeTab.searchMode = "filter";
    showToast(`Filtro attivo per: "${text.slice(0, 30)}"`);
    scheduleSearch(true);
  }

  function handleFilterExclude(text: string) {
    if (!activeTab) return;
    activeTab.searchQuery = text;
    activeTab.isRegex = false;
    activeTab.invert = true;
    activeTab.searchMode = "filter";
    showToast(`Esclusione attiva per: "${text.slice(0, 30)}"`);
    scheduleSearch(true);
  }

  function handleToggleRegex() {
    if (!activeTab) return;
    activeTab.isRegex = !activeTab.isRegex;
    scheduleSearch();
  }

  function handleToggleCase() {
    if (!activeTab) return;
    activeTab.caseSensitive = !activeTab.caseSensitive;
    scheduleSearch();
  }

  function handleModeChange(newMode: SearchDisplayMode) {
    if (!activeTab || activeTab.searchMode === newMode) return;
    activeTab.searchMode = newMode;
    virtualListRef?.clearCache();

    if (activeTab.searchMode === "highlight" && activeTab.matchLines.length > 0) {
      activeTab.currentMatchIndex = 1;
      activeTab.currentMatchLine = activeTab.matchLines[0];
      virtualListRef?.scrollToIndex(activeTab.matchLines[0] - 1);
    } else if (activeTab.searchMode === "filter") {
      virtualListRef?.scrollToTop();
    }
  }

  async function handleNextMatch() {
    if (!activeTab || activeTab.totalMatches === 0) return;
    const tabId = activeTab.id;

    if (activeTab.searchMode === "highlight") {
      const current = activeTab.currentMatchLine ?? 1;
      const next = await invoke<number | null>("get_next_match", { tabId, currentLine: current });
      if (next && activeTab && activeTab.id === tabId) {
        activeTab.currentMatchLine = next;
        const idx = activeTab.matchLines.indexOf(next);
        if (idx !== -1) {
          activeTab.currentMatchIndex = idx + 1;
        } else {
          activeTab.currentMatchIndex = (activeTab.currentMatchIndex % activeTab.totalMatches) + 1;
        }
        virtualListRef?.scrollToIndex(next - 1, true);
      }
    } else {
      if (activeTab.currentMatchIndex < activeTab.totalMatches) {
        activeTab.currentMatchIndex++;
        virtualListRef?.scrollToIndex(activeTab.currentMatchIndex - 1, true);
      } else {
        activeTab.currentMatchIndex = 1;
        virtualListRef?.scrollToIndex(0, true);
      }
    }
  }

  async function handlePrevMatch() {
    if (!activeTab || activeTab.totalMatches === 0) return;
    const tabId = activeTab.id;

    if (activeTab.searchMode === "highlight") {
      const current = activeTab.currentMatchLine ?? 1;
      const prev = await invoke<number | null>("get_prev_match", { tabId, currentLine: current });
      if (prev && activeTab && activeTab.id === tabId) {
        activeTab.currentMatchLine = prev;
        const idx = activeTab.matchLines.indexOf(prev);
        if (idx !== -1) {
          activeTab.currentMatchIndex = idx + 1;
        } else {
          activeTab.currentMatchIndex = activeTab.currentMatchIndex > 1 ? activeTab.currentMatchIndex - 1 : activeTab.totalMatches;
        }
        virtualListRef?.scrollToIndex(prev - 1, true);
      }
    } else {
      if (activeTab.currentMatchIndex > 1) {
        activeTab.currentMatchIndex--;
        virtualListRef?.scrollToIndex(activeTab.currentMatchIndex - 1, true);
      } else {
        activeTab.currentMatchIndex = activeTab.totalMatches;
        virtualListRef?.scrollToIndex(activeTab.totalMatches - 1, true);
      }
    }
  }

  // --- P1 Features: Bookmarks, Word Wrap, Export ---

  function toggleBookmark(lineNum: number) {
    if (!activeTab) return;
    const exists = activeTab.bookmarks.includes(lineNum);
    if (exists) {
      activeTab.bookmarks = activeTab.bookmarks.filter((n) => n !== lineNum);
      showToast(`Segnalibro rimosso dalla riga #${lineNum}`);
    } else {
      activeTab.bookmarks = [...activeTab.bookmarks, lineNum].sort((a, b) => a - b);
      showToast(`Segnalibro aggiunto alla riga #${lineNum}`);
    }
  }

  async function jumpToNextBookmark() {
    if (!activeTab || activeTab.bookmarks.length === 0) {
      showToast("Nessun segnalibro salvato");
      return;
    }
    const curLine = selectedLine?.line_number ?? 0;
    const next = activeTab.bookmarks.find((b) => b > curLine) ?? activeTab.bookmarks[0];
    try {
      const displayRow = await invoke<number>("map_line_to_display_row", {
        tabId: activeTab.id,
        lineNumber: next
      });
      pauseLive();
      virtualListRef?.scrollToIndex(displayRow, true);
      const line = virtualListRef?.getLineByNumber(next);
      if (line) {
        activeTab.selectedLine = line;
        activeTab.selectedLineNumbers = new Set([next]);
      }
      showToast(`Segnalibro: riga #${next} (${activeTab.bookmarks.indexOf(next) + 1}/${activeTab.bookmarks.length})`);
    } catch (e) {
      console.warn("Failed to jump to bookmark:", e);
    }
  }

  async function jumpToPrevBookmark() {
    if (!activeTab || activeTab.bookmarks.length === 0) {
      showToast("Nessun segnalibro salvato");
      return;
    }
    const curLine = selectedLine?.line_number ?? (totalLines + 1);
    const rev = [...activeTab.bookmarks].reverse();
    const prev = rev.find((b) => b < curLine) ?? rev[0];
    try {
      const displayRow = await invoke<number>("map_line_to_display_row", {
        tabId: activeTab.id,
        lineNumber: prev
      });
      pauseLive();
      virtualListRef?.scrollToIndex(displayRow, true);
      const line = virtualListRef?.getLineByNumber(prev);
      if (line) {
        activeTab.selectedLine = line;
        activeTab.selectedLineNumbers = new Set([prev]);
      }
      showToast(`Segnalibro: riga #${prev} (${activeTab.bookmarks.indexOf(prev) + 1}/${activeTab.bookmarks.length})`);
    } catch (e) {
      console.warn("Failed to jump to bookmark:", e);
    }
  }

  function clearAllBookmarks() {
    if (!activeTab) return;
    activeTab.bookmarks = [];
    showToast("Tutti i segnalibri sono stati rimossi");
  }

  function toggleWrapLines() {
    if (!activeTab) return;
    activeTab.wrapLines = !activeTab.wrapLines;
    try {
      localStorage.setItem("logviewer:wrapLines", String(activeTab.wrapLines));
    } catch {}
    showToast(`A capo automatico (Word Wrap): ${activeTab.wrapLines ? "attivato" : "disattivato"}`);
  }

  async function handleExportFilteredLines() {
    if (!activeTab) return;
    if (totalMatches === 0) {
      showToast("Nessuna riga filtrata da esportare");
      return;
    }

    try {
      const baseName = activeTab.fileName.replace(/\.[^/.]+$/, "");
      const defaultName = `${baseName}-filtered.log`;
      const savePath = await saveDialog({
        defaultPath: defaultName,
        filters: [
          { name: "File di Log", extensions: ["log", "txt", "out"] },
          { name: "Tutti i File", extensions: ["*"] }
        ]
      });

      if (!savePath) return;

      const count = await invoke<number>("export_filtered_lines", {
        tabId: activeTab.id,
        destPath: savePath
      });
      const fileOnly = savePath.split(/[\/\\]/).pop() ?? "file";
      showToast(`Esportate ${count.toLocaleString()} righe in ${fileOnly}`);
    } catch (err: any) {
      console.error("Export error:", err);
      showToast("Errore esportazione: " + (err?.message || err));
    }
  }

  async function copySelectedLines(withLineNumbers: boolean = false): Promise<boolean> {
    if (!activeTab) return false;

    let lineNumbers: number[] = [];
    if (activeTab.selectedLineNumbers && activeTab.selectedLineNumbers.size > 0) {
      lineNumbers = Array.from(activeTab.selectedLineNumbers).sort((a, b) => a - b);
    } else if (activeTab.selectedLine) {
      lineNumbers = [activeTab.selectedLine.line_number];
    }

    if (lineNumbers.length === 0) {
      showToast("Nessuna riga selezionata");
      return false;
    }

    try {
      let content = "";
      // Fast path: riga singola già disponibile in activeTab.selectedLine
      if (
        lineNumbers.length === 1 &&
        activeTab.selectedLine &&
        activeTab.selectedLine.line_number === lineNumbers[0]
      ) {
        content = withLineNumbers
          ? `${lineNumbers[0]}: ${activeTab.selectedLine.content}`
          : activeTab.selectedLine.content;
      } else {
        // Fast path: recupero ultra-rapido da mmap Rust senza dipendere dalla cache viewport
        content = await invoke<string>("get_lines_content", {
          tabId: activeTab.id,
          lineNumbers,
          withLineNumbers
        });
      }

      await navigator.clipboard.writeText(content);

      if (lineNumbers.length === 1) {
        showToast(
          withLineNumbers
            ? `Riga #${lineNumbers[0]} copiata (con numero)`
            : `Riga #${lineNumbers[0]} copiata negli appunti`
        );
      } else {
        showToast(
          withLineNumbers
            ? `${lineNumbers.length.toLocaleString()} righe copiate (con numeri)`
            : `${lineNumbers.length.toLocaleString()} righe copiate negli appunti`
        );
      }
      return true;
    } catch (err: any) {
      console.error("Errore copia righe:", err);
      showToast("Errore durante la copia negli appunti");
      return false;
    }
  }

  function handleSelectAllLines() {
    if (!activeTab || effectiveTotalLines <= 0) return;
    const MAX_SELECT = 20_000;
    const countToSelect = Math.min(effectiveTotalLines, MAX_SELECT);
    const nextSet = new Set<number>();
    for (let i = 1; i <= countToSelect; i++) {
      nextSet.add(i);
    }
    activeTab.selectedLineNumbers = nextSet;
    if (effectiveTotalLines > MAX_SELECT) {
      showToast(`Selezionate le prime ${MAX_SELECT.toLocaleString()} righe (file molto grande, usa Ctrl+K per esportare tutto)`);
    } else {
      showToast(`Tutte le ${countToSelect.toLocaleString()} righe selezionate (Ctrl+C per copiare)`);
    }
  }

  async function handleCopyAllFilteredLines() {
    if (!activeTab || totalMatches === 0) return;
    try {
      const content = await invoke<string>("get_filtered_content", {
        tabId: activeTab.id,
        maxLines: 5000
      });
      await navigator.clipboard.writeText(content);
      const lineCount = content.split("\n").filter(Boolean).length;
      showToast(`${lineCount.toLocaleString()} righe filtrate copiate negli appunti`);
    } catch (err: any) {
      showToast("Errore durante la copia delle righe filtrate");
    }
  }

  // --- P2 Timeline & Temporal Histogram Functions ---

  async function fetchTimeline(targetTab?: LogTab | null, forceFiltered?: boolean) {
    const tab = targetTab ?? activeTab;
    if (!tab) return;
    isTimelineLoading = true;
    try {
      const isFiltered = forceFiltered !== undefined ? forceFiltered : Boolean(tab.searchQuery.trim().length > 0 && tab.searchMode === "filter");
      const hist = await invoke<TimeHistogram>("get_time_histogram", {
        tabId: tab.id,
        bucketCount: 80,
        filtered: isFiltered
      });
      tab.timelineHistogram = hist;
      if (activeTab && activeTab.id === tab.id) {
        timelineHistogram = hist;
      }
    } catch (err) {
      console.warn("Failed to load timeline histogram:", err);
    } finally {
      isTimelineLoading = false;
    }
  }

  async function fetchHeatmap(targetTab?: LogTab | null) {
    const tab = targetTab ?? activeTab;
    if (!tab) return;
    try {
      const errorKeywords = logLevelsManager.levels.ERROR?.keywords || [];
      const warnKeywords = logLevelsManager.levels.WARN?.keywords || [];

      const res = await invoke<SeverityHeatmap>("get_severity_heatmap", {
        tabId: tab.id,
        numBins: 250,
        customErrorKeywords: errorKeywords,
        customWarnKeywords: warnKeywords
      });
      tab.heatmap = res;
      if (activeTab && activeTab.id === tab.id) {
        currentHeatmap = res;
      }
    } catch (err) {
      console.warn("Failed to fetch severity heatmap:", err);
    }
  }

  function toggleTimeline() {
    if (!activeTab) return;
    activeTab.isTimelineOpen = !activeTab.isTimelineOpen;
    try {
      localStorage.setItem("logviewer:isTimelineOpen", String(activeTab.isTimelineOpen));
    } catch {}

    if (activeTab.isTimelineOpen) {
      if (!timelineHistogram) {
        fetchTimeline(activeTab);
      }
    }
    showToast(`Timeline temporale: ${activeTab.isTimelineOpen ? "aperta" : "chiusa"}`);
  }

  async function handleTimelineSelectLine(lineNum: number) {
    if (!activeTab || lineNum <= 0) return;
    if (isLive) pauseLive();

    try {
      const displayRow = await invoke<number>("map_line_to_display_row", {
        tabId: activeTab.id,
        lineNumber: lineNum
      });
      virtualListRef?.scrollToIndex(displayRow - 1);
    } catch {
      virtualListRef?.scrollToIndex(lineNum - 1);
    }
  }

  function handleTimelineFilterTimeRange(range: TimelineBrushRange) {
    if (!activeTab) return;
    if (activeTab.timelineHistogram?.buckets[range.startIndex]) {
      const startBucket = activeTab.timelineHistogram.buckets[range.startIndex];
      handleSearchQueryChange(startBucket.label);
      showToast(`Filtro applicato su intervallo: ${startBucket.label}`);
    }
  }

  function isInputFocused(target: EventTarget | null): boolean {
    if (!target || !(target instanceof HTMLElement)) return false;
    const tag = target.tagName.toLowerCase();
    return tag === "input" || tag === "textarea" || target.isContentEditable;
  }

  function handleKeydown(e: KeyboardEvent) {
    // Toggle Command Palette: Cmd/Ctrl + K
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
      e.preventDefault();
      isCommandPaletteOpen = !isCommandPaletteOpen;
      return;
    }

    // Let Command Palette handle its own keys when open
    if (isCommandPaletteOpen) {
      return;
    }

    // Close Tab: Cmd/Ctrl + W
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "w") {
      e.preventDefault();
      if (activeTabId) {
        closeTab(activeTabId);
      }
      return;
    }

    // New Tab / Open file: Cmd/Ctrl + T or Cmd/Ctrl + O
    if ((e.metaKey || e.ctrlKey) && (e.key.toLowerCase() === "t" || e.key.toLowerCase() === "o")) {
      e.preventDefault();
      handleOpenFilePicker();
      return;
    }

    // Cycle tabs with Ctrl+Tab or Ctrl+Shift+Tab
    if (e.ctrlKey && e.key === "Tab") {
      e.preventDefault();
      if (tabs.length > 1 && activeTabId) {
        const curIdx = tabs.findIndex((t) => t.id === activeTabId);
        const nextIdx = e.shiftKey
          ? (curIdx - 1 + tabs.length) % tabs.length
          : (curIdx + 1) % tabs.length;
        switchTab(tabs[nextIdx].id);
      }
      return;
    }

    // Direct tab jump: Cmd/Ctrl + 1..9
    if ((e.metaKey || e.ctrlKey) && e.key >= "1" && e.key <= "9") {
      const idx = parseInt(e.key, 10) - 1;
      if (idx < tabs.length) {
        e.preventDefault();
        switchTab(tabs[idx].id);
        return;
      }
    }

    // Toggle stacktrace folding: Alt + S
    if (e.altKey && e.key.toLowerCase() === "s") {
      e.preventDefault();
      toggleAutoCollapseStacktraces();
      return;
    }

    // Toggle log levels customization modal: Alt + L
    if (e.altKey && e.key.toLowerCase() === "l") {
      e.preventDefault();
      isLogLevelsModalOpen = !isLogLevelsModalOpen;
      return;
    }

    // Toggle Invert / Negative Filter: Alt + I
    if (e.altKey && e.key.toLowerCase() === "i") {
      e.preventDefault();
      handleToggleInvert();
      return;
    }

    // Toggle Word Wrap: Alt + Z
    if (e.altKey && e.key.toLowerCase() === "z") {
      e.preventDefault();
      toggleWrapLines();
      return;
    }

    // Toggle Timeline / Time Histogram: Alt + T
    if (e.altKey && e.key.toLowerCase() === "t") {
      e.preventDefault();
      toggleTimeline();
      return;
    }

    // Bookmark navigation: F2 (next) and Shift+F2 (prev)
    if (e.key === "F2" && !isInputFocused(e.target)) {
      e.preventDefault();
      if (e.shiftKey) {
        jumpToPrevBookmark();
      } else {
        jumpToNextBookmark();
      }
      return;
    }

    // Toggle Bookmark on selected line: Cmd/Ctrl + B
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "b" && !isInputFocused(e.target)) {
      e.preventDefault();
      if (selectedLine) {
        toggleBookmark(selectedLine.line_number);
      } else {
        showToast("Seleziona una riga per aggiungere un segnalibro (Ctrl+B)");
      }
      return;
    }

    // Copy selected lines with line numbers: Cmd/Ctrl + Shift + C
    if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key.toLowerCase() === "c" && !isInputFocused(e.target)) {
      if ((activeTab?.selectedLineNumbers && activeTab.selectedLineNumbers.size > 0) || activeTab?.selectedLine) {
        e.preventDefault();
        copySelectedLines(true);
        return;
      }
    }

    // Copy selected lines: Cmd/Ctrl + C
    if ((e.metaKey || e.ctrlKey) && !e.shiftKey && e.key.toLowerCase() === "c" && !isInputFocused(e.target)) {
      const hasMultiRowSelection = Boolean(activeTab?.selectedLineNumbers && activeTab.selectedLineNumbers.size > 1);

      // Se l'utente ha selezionato più righe (tramite drag, Shift+Click o Ctrl+Click),
      // la selezione multi-riga ha SEMPRE la precedenza assoluta!
      if (hasMultiRowSelection) {
        e.preventDefault();
        window.getSelection()?.removeAllRanges();
        copySelectedLines(false);
        return;
      }

      // Se l'utente ha evidenziato intenzionalmente uno snippet di testo nel DOM
      const activeTextSel = window.getSelection()?.toString();
      if (activeTextSel && activeTextSel.trim().length > 0) {
        return; // Lascia copiare nativamente al browser lo snippet di testo evidenziato
      }

      // Altrimenti copia la riga selezionata
      if (
        (activeTab?.selectedLineNumbers && activeTab.selectedLineNumbers.size > 0) ||
        activeTab?.selectedLine
      ) {
        e.preventDefault();
        copySelectedLines(false);
        return;
      }
    }

    // Select all lines: Cmd/Ctrl + A (when not editing an input/textarea)
    if (
      (e.metaKey || e.ctrlKey) &&
      e.key.toLowerCase() === "a" &&
      !isInputFocused(e.target) &&
      !isDrawerOpen &&
      !isCommandPaletteOpen &&
      !isLogLevelsModalOpen
    ) {
      e.preventDefault();
      handleSelectAllLines();
      return;
    }

    // Quick filter focus: Vim-style "/" or Cmd/Ctrl + F
    if ((e.key === "/" && !isInputFocused(e.target)) || ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "f")) {
      e.preventDefault();
      filterBarRef?.focusInput();
      return;
    }

    // Space key jumps to bottom and resumes Live mode (when not in search input or drawer)
    if (e.key === " " && !isInputFocused(e.target) && !isDrawerOpen) {
      e.preventDefault();
      resumeLive();
      return;
    }

    // Home key jumps to top
    if (e.key === "Home" && !isInputFocused(e.target) && !isDrawerOpen) {
      e.preventDefault();
      pauseLive();
      virtualListRef?.scrollToTop();
      return;
    }

    // End key jumps to bottom
    if (e.key === "End" && !isInputFocused(e.target) && !isDrawerOpen) {
      e.preventDefault();
      resumeLive();
      virtualListRef?.scrollToBottom();
      return;
    }
  }

  onMount(() => {
    window.addEventListener("keydown", handleKeydown);

    function handleGlobalCopy(e: ClipboardEvent) {
      if (isInputFocused(document.activeElement)) return;
      const hasMultiRowSelection = Boolean(activeTab?.selectedLineNumbers && activeTab.selectedLineNumbers.size > 1);

      if (hasMultiRowSelection) {
        e.preventDefault();
        window.getSelection()?.removeAllRanges();
        copySelectedLines(false);
        return;
      }

      const sel = window.getSelection()?.toString();
      if (sel && sel.trim().length > 0) return;
      if (
        (activeTab?.selectedLineNumbers && activeTab.selectedLineNumbers.size > 0) ||
        activeTab?.selectedLine
      ) {
        e.preventDefault();
        copySelectedLines(false);
      }
    }

    window.addEventListener("copy", handleGlobalCopy);

    try {
      const savedAnim = localStorage.getItem("logviewer:liveAnimations");
      if (savedAnim !== null) {
        liveAnimations = savedAnim === "true";
      }
      const savedPinned = localStorage.getItem("logviewer:drawerPinned");
      if (savedPinned !== null) {
        isDrawerPinned = savedPinned === "true";
      }
    } catch {}

    let unlistenAppend: UnlistenFn | null = null;
    let unlistenFilter: UnlistenFn | null = null;
    let unlistenDragDrop: UnlistenFn | null = null;
    let unlistenStdin: UnlistenFn | null = null;

    // Check if a file was passed via CLI argument or stdin pipe
    invoke<string | null>("get_initial_file")
      .then((path) => {
        if (path) {
          openFilePath(path);
        }
      })
      .catch((err) => {
        console.warn("Failed to get initial file:", err);
      });

    // Native Tauri Webview drag-and-drop listener
    try {
      getCurrentWebview()
        .onDragDropEvent((event) => {
          if (event.payload.type === "over" || event.payload.type === "enter") {
            isDragging = true;
          } else if (event.payload.type === "drop") {
            isDragging = false;
            const paths = event.payload.paths;
            if (paths && paths.length > 0) {
              for (const p of paths) {
                openFilePath(p);
              }
            }
          } else if (event.payload.type === "leave") {
            isDragging = false;
          }
        })
        .then((fn) => {
          unlistenDragDrop = fn;
        })
        .catch((e) => {
          console.warn("Native drag-drop registration:", e);
        });
    } catch (e) {
      console.warn("Native drag-drop error:", e);
    }

    // Delayed stdin stream ready listener
    listen<{ path: string }>("log:stdin_ready", (event) => {
      if (event.payload?.path) {
        openFilePath(event.payload.path);
      }
    }).then((fn) => {
      unlistenStdin = fn;
    });

    // File Append Real-time Listener (Per-tab routing)
    listen<LogAppendedPayload>("log:appended", (event) => {
      const { tab_id, total_lines, file_size } = event.payload;
      const targetTab = tabs.find((t) => t.id === tab_id);
      if (!targetTab) return;

      const oldTotal = targetTab.totalLines;
      targetTab.fileSize = file_size;
      targetTab.totalLines = total_lines;

      if (targetTab.id === activeTabId) {
        invoke<number>("get_display_row_count", { tabId: targetTab.id }).then((rowCount) => {
          if (targetTab) {
            targetTab.displayTotalRows = rowCount;
            if (targetTab.isLive) {
              requestAnimationFrame(() => {
                virtualListRef?.scrollToBottom(false);
              });
            }
          }
        });

        invoke<FoldingStatus>("get_folding_status", { tabId: targetTab.id }).then((status) => {
          if (targetTab) {
            targetTab.foldingStatus = status;
          }
        });

        if (!isFilterActive) {
          virtualListRef?.invalidateFromLine(Math.max(1, oldTotal));
        }

        if (targetTab.isTimelineOpen) {
          fetchTimeline(targetTab);
        }

        fetchHeatmap(targetTab);

        if (targetTab.isLive) {
          requestAnimationFrame(() => {
            virtualListRef?.scrollToBottom(false);
          });
        } else {
          targetTab.unreadLines = Math.max(0, targetTab.totalLines - targetTab.pausedAtLine);
        }
      } else {
        // Tab in background: increment unread lines and display total rows count
        const diff = Math.max(0, total_lines - oldTotal);
        targetTab.unreadLines += diff;
        targetTab.displayTotalRows += diff;
      }
    }).then((fn) => {
      unlistenAppend = fn;
    });

    // Search Filter Real-time Listener (Per-tab routing)
    listen<FilterUpdatedPayload>("log:filter_updated", (event) => {
      const { tab_id, total_matches } = event.payload;
      const targetTab = tabs.find((t) => t.id === tab_id);
      if (!targetTab) return;

      const prevMatches = targetTab.totalMatches;
      targetTab.totalMatches = total_matches;

      if (targetTab.id === activeTabId && isFilterActive) {
        virtualListRef?.invalidateFromLine(Math.max(1, prevMatches));
        if (targetTab.isLive) {
          requestAnimationFrame(() => {
            virtualListRef?.scrollToBottom(false);
          });
        }
      }
    }).then((fn) => {
      unlistenFilter = fn;
    });

    return () => {
      window.removeEventListener("keydown", handleKeydown);
      window.removeEventListener("copy", handleGlobalCopy);
      if (unlistenAppend) unlistenAppend();
      if (unlistenFilter) unlistenFilter();
      if (unlistenDragDrop) unlistenDragDrop();
      if (unlistenStdin) unlistenStdin();
      if (searchDebounceTimer) clearTimeout(searchDebounceTimer);
      if (toastTimer) clearTimeout(toastTimer);
    };
  });
</script>

<div
  class="flex flex-col h-screen w-screen bg-[#0B0D13] text-[#F4F6FB] select-none font-sans overflow-hidden"
  role="region"
  aria-label="Loglance Log Viewer"
  ondragover={(e) => {
    e.preventDefault();
    isDragging = true;
  }}
  ondragleave={() => {
    isDragging = false;
  }}
  ondrop={async (e) => {
    e.preventDefault();
    isDragging = false;
    if (e.dataTransfer?.files && e.dataTransfer.files.length > 0) {
      for (let i = 0; i < e.dataTransfer.files.length; i++) {
        const file = e.dataTransfer.files[i];
        const filePath = (file as any).path || file.name;
        if (filePath) {
          await openFilePath(filePath);
        }
      }
    }
  }}
>
  <!-- Zona 1: Top Header Command Bar (h-12) - Visibile solo a file aperto -->
  {#if activeTab}
    <header class="flex items-center justify-between px-4 h-12 border-b border-white/[0.08] bg-gradient-to-b from-[#171B26] to-[#12151E] shadow-[0_1px_8px_rgba(0,0,0,0.4)] shrink-0 text-xs font-sans gap-3 z-20">
      <!-- 1. Sinistra: Brand -->
      <div class="flex items-center gap-3 min-w-0 shrink-0">
        <div class="flex items-center gap-2.5">
          <div class="w-7 h-7 rounded-lg bg-amber-500/15 ring-1 ring-inset ring-amber-400/35 shadow-[inset_0_1px_0_rgba(255,255,255,0.18),0_2px_8px_rgba(245,158,11,0.2)] flex items-center justify-center text-amber-300 shrink-0">
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M4 6h16M4 12h10M4 18h14" />
            </svg>
          </div>
          <span class="font-sans font-bold text-[14px] tracking-tight text-white">Loglance</span>
        </div>

        <button
          onclick={handleOpenFilePicker}
          class="tactile-button hidden sm:flex items-center gap-2 px-2.5 py-1 rounded-lg cursor-pointer max-w-[240px] truncate"
          title="Indicizzato in {activeTab.indexTimeMs.toFixed(1)}ms — Percorso: {activeTab.path}"
        >
          <svg class="w-3.5 h-3.5 text-amber-400 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
          </svg>
          <span class="font-semibold text-[#F4F6FB] truncate">{activeTab.fileName}</span>
          <span class="text-[#B4BCD0] text-[11px] font-mono shrink-0">{formatBytes(activeTab.fileSize)}</span>
        </button>
      </div>

    <!-- 2. Centro: Search & Filter Bar Integrata (per tab attivo) -->
    {#if activeTab}
      <div class="flex-1 flex justify-center min-w-[220px] max-w-2xl">
        <FilterBar
          bind:this={filterBarRef}
          query={searchQuery}
          {isRegex}
          {caseSensitive}
          {invert}
          mode={searchMode}
          {totalMatches}
          {totalLines}
          {searchTimeMs}
          {currentMatchIndex}
          error={searchError}
          isLoading={isSearching}
          quickPreset={activeQuickPreset}
          canExport={isFilterActive && totalMatches > 0}
          onSearchChange={handleSearchQueryChange}
          onToggleRegex={handleToggleRegex}
          onToggleCase={handleToggleCase}
          onToggleInvert={handleToggleInvert}
          onModeChange={handleModeChange}
          onNextMatch={handleNextMatch}
          onPrevMatch={handlePrevMatch}
          onClear={clearSearch}
          onApplyQuickPreset={applyQuickPreset}
          onExport={handleExportFilteredLines}
          onCopy={handleCopyAllFilteredLines}
        />
      </div>
    {/if}

    <!-- 3. Destra: Segnalibri, Menu Vista & Azioni Globali -->
    <div class="flex items-center gap-2 shrink-0">
      {#if activeTab}
        <!-- Bookmarks Pill (solo se ci sono segnalibri) -->
        {#if bookmarks.length > 0}
          <div class="flex items-center rounded-lg bg-cyan-950/40 border border-cyan-500/30 text-cyan-200 text-xs overflow-hidden shadow-xs">
            <button
              onclick={jumpToPrevBookmark}
              class="px-1.5 py-1 hover:bg-cyan-500/20 text-cyan-300 transition-colors cursor-pointer border-r border-cyan-500/20"
              title="Segnalibro precedente (Shift + F2)"
            >
              <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M5 15l7-7 7 7" />
              </svg>
            </button>
            <button
              onclick={jumpToNextBookmark}
              class="px-2 py-1 flex items-center gap-1 hover:bg-cyan-500/20 text-cyan-200 transition-colors cursor-pointer font-medium text-[11px]"
              title="Segnalibro successivo (F2)"
            >
              <svg class="w-3 h-3 fill-cyan-400 text-cyan-400" viewBox="0 0 24 24">
                <path d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
              </svg>
              <span>{bookmarks.length}</span>
            </button>
            <button
              onclick={jumpToNextBookmark}
              class="px-1.5 py-1 hover:bg-cyan-500/20 text-cyan-300 transition-colors cursor-pointer border-l border-cyan-500/20"
              title="Segnalibro successivo (F2)"
            >
              <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M19 9l-7 7-7-7" />
              </svg>
            </button>
          </div>
        {/if}

        <!-- Menu Unificato "Vista" (A capo, Timeline, Stacktrace, Livelli) -->
        <ViewMenu
          {wrapLines}
          onToggleWrap={toggleWrapLines}
          {isTimelineOpen}
          onToggleTimeline={toggleTimeline}
          {foldingStatus}
          onToggleFolding={toggleAutoCollapseStacktraces}
          onOpenLogLevels={() => (isLogLevelsModalOpen = true)}
        />
      {/if}

      <button
        onclick={() => (isCommandPaletteOpen = true)}
        class="tactile-button flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-[#F4F6FB] text-xs cursor-pointer"
        title="Command Palette (Ctrl/Cmd + K)"
      >
        <svg class="w-3.5 h-3.5 text-indigo-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
        </svg>
        <span class="hidden sm:inline font-medium">Comandi</span>
        <kbd class="tactile-kbd text-[10px] text-[#B4BCD0] px-1.5 py-0.5 rounded">⌘K</kbd>
      </button>

      <button
        onclick={handleOpenFilePicker}
        class="flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-amber-500/20 hover:bg-amber-500/30 text-amber-200 ring-1 ring-inset ring-amber-400/40 shadow-[inset_0_1px_0_rgba(255,255,255,0.15),0_1px_3px_rgba(0,0,0,0.4)] transition-all duration-150 active:scale-95 text-xs cursor-pointer font-medium"
        title="Apri File Log (Ctrl/Cmd + O o Ctrl/Cmd + T)"
      >
        <svg class="w-3.5 h-3.5 text-amber-300" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 19a2 2 0 01-2-2V7a2 2 0 012-2h4l2 2h4a2 2 0 012 2v1M5 19h14a2 2 0 002-2v-5a2 2 0 00-2-2H9a2 2 0 00-2 2v5a2 2 0 01-2 2z" />
        </svg>
        <span class="hidden sm:inline">Apri</span>
        <kbd class="tactile-kbd text-[10px] text-amber-200 px-1.5 py-0.5 rounded">⌘O</kbd>
      </button>
    </div>
  </header>
  {/if}

  <!-- Zona 1.5: Tab Bar per la navigazione multi-log -->
  {#if tabs.length > 0}
    <TabBar
      {tabs}
      {activeTabId}
      onSelectTab={switchTab}
      onCloseTab={closeTab}
      onNewTab={handleOpenFilePicker}
    />
  {/if}

  <!-- Error Banner if any -->
  {#if errorMessage}
    <div
      transition:fly={{ y: -12, duration: 180, easing: cubicOut }}
      class="px-4 py-2 bg-rose-500/15 border-b border-rose-500/35 text-rose-200 text-xs font-sans flex items-center justify-between z-10"
    >
      <span>{errorMessage}</span>
      <button onclick={() => (errorMessage = null)} class="text-rose-200 hover:underline cursor-pointer font-semibold">Chiudi</button>
    </div>
  {/if}

  <!-- Zona 1.7: Timeline / Istogramma Temporale con Brushing -->
  {#if activeTab && isTimelineOpen}
    <TimelineHistogram
      histogram={timelineHistogram}
      isLoading={isTimelineLoading}
      {isFilterActive}
      onSelectLine={handleTimelineSelectLine}
      onFilterTimeRange={handleTimelineFilterTimeRange}
      onClose={toggleTimeline}
    />
  {/if}

  <!-- Main Canvas -->
  <main class="relative flex-1 w-full h-full overflow-hidden bg-[#0B0D13] flex flex-row">
    <!-- Colonna sinistra: Canvas principale con stream log -->
    <div class="relative flex-1 h-full min-w-0 overflow-hidden">
      {#if activeTab && effectiveTotalLines > 0}
        <VirtualList
          bind:this={virtualListRef}
          totalLines={effectiveTotalLines}
          itemHeight={24}
          buffer={40}
          {fetchSlice}
          {isLive}
          {liveAnimations}
          searchQuery={searchQuery}
          {isRegex}
          {caseSensitive}
          wrapLines={wrapLines}
          bookmarks={bookmarks}
          heatmap={currentHeatmap}
          onToggleBookmark={toggleBookmark}
          activeMatchLine={currentMatchLine}
          selectedLineNumber={selectedLine ? selectedLine.line_number : null}
          {selectedLineNumbers}
          {matchLines}
          onLineClick={handleLineClick}
          onLineDoubleClick={handleLineDoubleClick}
          onSelectionRangeChange={handleSelectionRangeChange}
          onToggleStacktraceGroup={handleToggleStacktraceGroup}
          onCopySelected={copySelectedLines}
          onUserScrollUp={pauseLive}
          onUserScrollBottom={resumeLive}
          onSelectionPause={pauseLive}
          onOpenLogLevels={() => (isLogLevelsModalOpen = true)}
          onFilterInclude={handleFilterInclude}
          onFilterExclude={handleFilterExclude}
          onTimeDeltaChange={(delta) => (currentTimeDelta = delta)}
          onToast={(msg) => showToast(msg)}
          onQuickMapLevel={(word, level) => {
            showToast(`Mappato "${word}" come ${level}`);
            if (activeQuickPreset === "errors" || activeQuickPreset === "warnings") {
              scheduleSearch(true);
            }
          }}
          onScrollChange={(info) => {
            isAtBottom = info.isAtBottom;
            if (activeTab) {
              activeTab.lastScrollIndex = Math.floor(info.scrollTop / 24);
            }
            if (info.wasUserAction) {
              if (!info.isAtBottom && isLive) {
                pauseLive();
              } else if (info.isAtBottom && !isLive) {
                const sel = window.getSelection();
                if (!sel || sel.toString().trim().length === 0) {
                  resumeLive();
                }
              }
            }
          }}
        />

        <!-- Floating Jump to Live Pill (shown when paused) -->
        {#if !isLive}
          <button
            transition:fly={{ y: 16, duration: 200, easing: cubicOut }}
            onclick={resumeLive}
            class="absolute bottom-5 right-8 z-30 flex items-center gap-2.5 px-4 py-2 rounded-full bg-[#161A26]/95 backdrop-blur-xl ring-1 ring-inset ring-amber-400/50 text-white shadow-[0_12px_32px_rgba(0,0,0,0.65),0_0_20px_rgba(245,158,11,0.18)] hover:ring-amber-300 hover:bg-[#1C2130] transition-all duration-150 active:scale-95 cursor-pointer group font-sans"
            title="Riprendi live tail (Space)"
          >
            <span class="w-2 h-2 rounded-full bg-amber-400 animate-pulse shadow-[0_0_8px_rgba(251,191,36,0.9)]"></span>
            <span class="text-xs font-semibold text-amber-200">Riprendi Live</span>
            {#if unreadLines > 0}
              <span class="px-2 py-0.5 rounded-full bg-amber-500/25 text-[10.5px] font-mono font-bold text-amber-200 ring-1 ring-inset ring-amber-400/30">
                +{unreadLines.toLocaleString()}
              </span>
            {/if}
            <svg class="w-3.5 h-3.5 text-amber-300 group-hover:translate-y-0.5 transition-transform" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M19 14l-7 7m0 0l-7-7m7 7V3" />
            </svg>
            <kbd class="tactile-kbd text-[9.5px] px-1.5 py-0.5 rounded text-[#B4BCD0]">Space</kbd>
          </button>
        {/if}
      {:else if activeTab && isFilterActive && totalMatches === 0}
        <!-- Empty Filter State -->
        <div
          in:fade={{ duration: 180 }}
          class="flex flex-col items-center justify-center h-full text-center p-6 text-[#B4BCD0] space-y-3.5 font-sans"
        >
          <div class="w-12 h-12 rounded-2xl tactile-surface text-amber-300 flex items-center justify-center">
            <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
            </svg>
          </div>
          <p class="text-sm font-semibold text-white">
            {invert ? 'Tutte le righe contengono il pattern escluso' : 'Nessuna riga corrisponde al filtro'}
          </p>
          <p class="text-xs text-[#B4BCD0] max-w-xs leading-relaxed">
            {#if invert}
              Nessuna riga priva di <code class="text-rose-200 font-mono bg-white/[0.06] px-1.5 py-0.5 rounded ring-1 ring-inset ring-white/[0.1]">{searchQuery}</code> ({searchTimeMs.toFixed(1)}ms).
            {:else}
              Nessun risultato per <code class="text-indigo-200 font-mono bg-white/[0.06] px-1.5 py-0.5 rounded ring-1 ring-inset ring-white/[0.1]">{searchQuery}</code> ({searchTimeMs.toFixed(1)}ms).
            {/if}
          </p>
          <button
            onclick={clearSearch}
            class="px-4 py-2 rounded-xl bg-indigo-500/20 hover:bg-indigo-500/30 text-xs text-indigo-200 ring-1 ring-inset ring-indigo-400/40 font-semibold transition-all duration-150 active:scale-95 cursor-pointer"
          >
            Mostra tutte le righe
          </button>
        </div>
      {:else if isLoading}
        <div class="flex flex-col items-center justify-center h-full text-[#B4BCD0] space-y-3 font-sans">
          <div class="w-7 h-7 border-2 border-indigo-400 border-t-transparent rounded-full animate-spin"></div>
          <p class="text-xs font-medium">Indicizzazione ultra-rapida in corso...</p>
        </div>
      {:else}
        <!-- Zona 2: Home Screen "Loglance Hub" Emozionante, Minimale & Fluida -->
        <div class="relative flex flex-col items-center justify-center h-full p-6 font-sans overflow-hidden select-none">
          <!-- Subtle Tech Dot-Grid with Radial Vignette -->
          <div
            class="absolute inset-0 pointer-events-none opacity-30"
            style="background-image: radial-gradient(rgba(255, 255, 255, 0.15) 1px, transparent 1px); background-size: 24px 24px; mask-image: radial-gradient(ellipse 75% 65% at 50% 50%, black 20%, transparent 85%);"
          ></div>

          <!-- Modern Top Ambient Glare Spotlight (Soft Indigo Ceiling Wash) -->
          <div class="absolute -top-36 left-1/2 -translate-x-1/2 w-[700px] h-[360px] bg-gradient-to-b from-indigo-500/20 via-sky-400/08 to-transparent rounded-full blur-[100px] pointer-events-none"></div>

          <!-- Ambient Core Backlight (respira dolcemente al centro) -->
          <div class="absolute w-[560px] h-[320px] rounded-full bg-gradient-to-tr from-amber-500/12 via-indigo-600/12 to-sky-400/08 blur-[120px] pointer-events-none -translate-x-1/2 -translate-y-1/2 left-1/2 top-1/2 animate-aurora"></div>

          <!-- Hero Card: Rock-Solid Static Position (Zero fluttuazione, bottoni fermi) -->
          <div class="relative z-10 flex flex-col items-center max-w-xl text-center space-y-7">
            <!-- Iconic Glowing Badge -->
            <div class="relative group">
              <div class="absolute -inset-2 rounded-3xl bg-gradient-to-r from-amber-500/25 via-indigo-500/25 to-sky-500/25 blur-lg opacity-40 group-hover:opacity-75 transition duration-500"></div>
              <div class="relative w-20 h-20 rounded-2xl bg-gradient-to-b from-[#1E2333] to-[#121520] ring-1 ring-white/15 shadow-[0_16px_40px_rgba(0,0,0,0.6),inset_0_1px_0_rgba(255,255,255,0.2)] flex items-center justify-center text-amber-300">
                <svg class="w-10 h-10 transition-transform duration-300 group-hover:scale-110" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M13 10V3L4 14h7v7l9-11h-7z" />
                </svg>
              </div>
            </div>

            <!-- Title -->
            <h1 class="text-3xl sm:text-4xl font-extrabold tracking-tight text-white">
              Loglance
            </h1>

            <!-- Drag & Drop Interactive Dropzone Card -->
            <div
              onclick={handleOpenFilePicker}
              class="group/drop w-full p-6 sm:p-8 rounded-2xl bg-[#12151E]/80 hover:bg-[#161B27]/90 border border-dashed {isDragging ? "border-amber-400 bg-amber-500/10 ring-2 ring-amber-400/30" : "border-white/[0.12] hover:border-amber-400/50"} backdrop-blur-xl transition-colors duration-200 cursor-pointer shadow-[0_16px_40px_rgba(0,0,0,0.5),inset_0_1px_0_rgba(255,255,255,0.1)] flex flex-col items-center gap-3.5"
              role="button"
              tabindex="0"
              onkeydown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  handleOpenFilePicker();
                }
              }}
            >
              <div class="w-11 h-11 rounded-xl bg-amber-500/15 ring-1 ring-amber-400/35 flex items-center justify-center text-amber-300 group-hover/drop:scale-110 transition-transform">
                <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
                </svg>
              </div>

              <div class="space-y-1">
                <p class="text-sm font-semibold text-white">
                  {isDragging ? "Rilascia i file qui per aprirli" : "Trascina qui uno o più file di log, oppure clicca"}
                </p>
                <p class="text-xs text-[#7E879B]">
                  Supporta selezioni multiple, schede concorrenti, .log, .txt, .json, .jsonl, .out
                </p>
              </div>

              <!-- Static Action Buttons: Fermi e solidi con micro-interazione al click -->
              <div class="flex items-center gap-2 pt-1">
                <span class="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-amber-500 text-black font-semibold text-xs shadow-md group-hover/drop:bg-amber-400 active:scale-[0.98] transition-all">
                  <span>Sfoglia File</span>
                  <kbd class="text-[10px] bg-black/20 text-black px-1.5 py-0.5 rounded font-mono font-bold">⌘O</kbd>
                </span>
                <span class="text-xs text-[#7E879B]">o</span>
                <button
                  type="button"
                  onclick={(e) => {
                    e.stopPropagation();
                    isCommandPaletteOpen = true;
                  }}
                  class="px-3 py-1.5 rounded-lg bg-white/[0.06] hover:bg-white/[0.1] active:scale-[0.98] text-xs font-semibold text-[#D8DFEE] ring-1 ring-inset ring-white/[0.1] transition-all cursor-pointer"
                >
                  Tavolozza Comandi <kbd class="text-[10px] text-[#A6B2C8] ml-1 font-mono">⌘K</kbd>
                </button>
              </div>
            </div>

            <!-- Terminal Pipe Tip Pill (Click to Copy) -->
            <div class="flex flex-col sm:flex-row items-center gap-2 text-xs text-[#7E879B]">
              <span class="text-[#9AA5BE]">Oppure fai streaming diretto dal terminale:</span>
              <button
                type="button"
                onclick={copyCliSnippet}
                class="group/cli flex items-center gap-2 px-3 py-1 rounded-lg bg-black/40 hover:bg-black/60 ring-1 ring-inset ring-white/[0.1] text-[#E0E7FF] font-mono text-[11px] transition-all cursor-pointer active:scale-[0.98]"
                title="Clicca per copiare il comando negli appunti"
              >
                <span class="text-amber-400">$</span>
                <span>cat app.log | loglance</span>
                <span class="text-[#7E879B] group-hover/cli:text-white transition-colors text-[10px]">
                  {copiedSnippet ? "✓ Copiato" : "Copia"}
                </span>
              </button>
            </div>
          </div>
        </div>
      {/if}

      <!-- Native Webview Drag Overlay -->
      {#if isDragging}
        <div
          transition:fade={{ duration: 120 }}
          class="absolute inset-0 bg-indigo-950/70 backdrop-blur-sm z-50 flex flex-col items-center justify-center p-6 border-2 border-dashed border-sky-400 pointer-events-none gap-4"
        >
          <div class="w-16 h-16 rounded-2xl bg-sky-500/20 ring-1 ring-sky-400/40 text-sky-300 flex items-center justify-center shadow-[0_0_28px_rgba(56,189,248,0.4)]">
            <svg class="w-8 h-8" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
            </svg>
          </div>
          <div class="px-5 py-2.5 rounded-2xl tactile-surface text-sky-200 text-sm font-semibold tracking-wide shadow-xl">
            Rilascia i file per aprirli istantaneamente in schede Loglance
          </div>
        </div>
      {/if}
    </div>

    <!-- Step 4: JSON & XML Inspector Pinned Side Column -->
    {#if isDrawerOpen && isDrawerPinned && selectedLine}
      <JsonDrawer
        line={selectedLine}
        isOpen={isDrawerOpen}
        isPinned={true}
        onClose={() => (isDrawerOpen = false)}
        onTogglePin={toggleDrawerPin}
        onPrevLine={handleDrawerPrevLine}
        onNextLine={handleDrawerNextLine}
      />
    {/if}
  </main>

  <!-- Step 4: JSON & XML Inspector Floating Overlay Drawer -->
  {#if isDrawerOpen && !isDrawerPinned && selectedLine}
    <JsonDrawer
      line={selectedLine}
      isOpen={isDrawerOpen}
      isPinned={false}
      onClose={() => (isDrawerOpen = false)}
      onTogglePin={toggleDrawerPin}
      onPrevLine={handleDrawerPrevLine}
      onNextLine={handleDrawerNextLine}
    />
  {/if}

  <!-- Zona 3: Bottom Status Bar Minimalista e Zen (h-7 / 28px) - Visibile solo a file aperto -->
  {#if activeTab}
    <footer class="h-7 px-3.5 bg-[#12151E] border-t border-white/[0.08] text-[11px] font-sans flex items-center justify-between text-[#B4BCD0] shrink-0 z-20">
      <!-- Sinistra: LIVE/PAUSED + Metriche righe -->
      <div class="flex items-center gap-2.5 truncate">
        {#if isLive}
          <button
            onclick={pauseLive}
            class="flex items-center gap-1.5 px-2 py-0.5 rounded-md bg-emerald-500/15 text-emerald-300 ring-1 ring-inset ring-emerald-400/35 hover:bg-emerald-500/25 transition cursor-pointer font-semibold text-[10.5px]"
            title="Metti in pausa lo streaming automatico (Space o scroll in su)"
          >
            <span class="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse"></span>
            <span>LIVE</span>
          </button>
        {:else}
          <button
            onclick={resumeLive}
            class="flex items-center gap-1.5 px-2 py-0.5 rounded-md bg-amber-500/15 text-amber-300 ring-1 ring-inset ring-amber-400/35 hover:bg-amber-500/25 transition cursor-pointer font-semibold text-[10.5px]"
            title="Riprendi lo streaming automatico (Space)"
          >
            <span class="w-1.5 h-1.5 rounded-full bg-amber-400"></span>
            <span>PAUSED</span>
            {#if unreadLines > 0}
              <span class="text-amber-200 font-mono">+{unreadLines.toLocaleString()}</span>
            {/if}
          </button>
        {/if}

        <span class="text-white/20 select-none">·</span>

        <div class="flex items-center gap-1.5 text-[#B4BCD0]">
          <span class="font-mono text-[#F4F6FB]">{effectiveTotalLines.toLocaleString()}</span>
          <span>{effectiveTotalLines === 1 ? 'riga' : 'righe'}</span>

          {#if isFilterActive}
            <span class="text-indigo-300 font-mono">
              ({totalMatches.toLocaleString()} / {totalLines.toLocaleString()})
            </span>
            {#if invert}
              <span class="px-1.5 py-0.2 rounded bg-rose-500/20 text-rose-300 ring-1 ring-inset ring-rose-400/35 text-[9px] font-mono font-semibold">
                Esclusione
              </span>
            {/if}
          {:else if displayTotalRows < totalLines}
            <span class="text-purple-300 text-[10.5px]">
              ({(totalLines - displayTotalRows).toLocaleString()} compresse)
            </span>
          {/if}
        </div>
      </div>

      <!-- Centro: Badge Contestuali (Delta temporale o Selezione) -->
      <div class="flex items-center gap-2">
        {#if currentTimeDelta}
          <button
            onclick={async () => {
              if (currentTimeDelta) {
                try {
                  await navigator.clipboard.writeText(currentTimeDelta.formatted);
                  showToast(`Δt copiato: ${currentTimeDelta.formattedShort}`);
                } catch {}
              }
            }}
            class="flex items-center gap-1.5 px-2 py-0.5 rounded-md bg-amber-500/20 text-amber-200 ring-1 ring-inset ring-amber-400/40 hover:bg-amber-500/30 transition cursor-pointer font-mono text-[10.5px] font-semibold active:scale-95 shadow-xs"
            title="Clicca per copiare il delta temporale negli appunti ({currentTimeDelta.formatted})"
          >
            <span>⏱️ Δt:</span>
            <span class="text-white font-bold">{currentTimeDelta.formattedShort}</span>
            <span class="text-[9.5px] text-amber-300/80">({currentTimeDelta.lineCount} righe)</span>
          </button>
        {/if}

        {#if selectedLineNumbers.size > 0}
          <span class="px-2 py-0.5 rounded-full bg-indigo-500/20 text-indigo-300 ring-1 ring-inset ring-indigo-400/35 text-[10px] font-mono font-semibold">
            {selectedLineNumbers.size.toLocaleString()} {selectedLineNumbers.size === 1 ? 'selezionata' : 'selezionate'}
          </span>
        {/if}
      </div>

      <!-- Destra: Schede aperte + Vai alla fine -->
      <div class="flex items-center gap-3 shrink-0">
        {#if tabs.length > 1}
          <span class="text-[10.5px] text-[#7E879B] font-mono">
            {tabs.length} schede
          </span>
        {/if}
        {#if !isAtBottom || !isLive}
          <button
            onclick={resumeLive}
            class="text-indigo-300 hover:text-indigo-200 font-semibold transition cursor-pointer text-[11px] flex items-center gap-1 hover:underline"
            title="Torna alla fine del file (Space)"
          >
            <span>Vai alla fine (Space)</span>
            <span>⇣</span>
          </button>
        {/if}
      </div>
    </footer>
  {/if}

  <!-- Step 5: Toast Notification -->
  {#if toastMessage}
    <div
      transition:fly={{ y: 12, duration: 180, easing: cubicOut }}
      class="fixed bottom-10 left-1/2 -translate-x-1/2 z-50 px-4 py-2 rounded-xl tactile-surface text-white text-xs font-sans font-semibold flex items-center gap-2.5 shadow-2xl"
    >
      <span class="w-2 h-2 rounded-full bg-indigo-400 shadow-[0_0_8px_rgba(129,140,248,0.9)]"></span>
      <span>{toastMessage}</span>
    </div>
  {/if}

  <!-- Step 5: Floating Command Palette (Cmd/Ctrl + K) -->
  <CommandPalette
    isOpen={isCommandPaletteOpen}
    {currentFile}
    {isLive}
    {liveAnimations}
    {searchMode}
    {isRegex}
    {caseSensitive}
    {invert}
    {isDrawerOpen}
    foldingEnabled={foldingStatus.auto_collapse_enabled}
    {totalLines}
    {tabs}
    {activeTabId}
    onClose={() => (isCommandPaletteOpen = false)}
    onAction={handlePaletteAction}
  />

  <!-- Modal Personalizzazione Livelli di Log -->
  <LogLevelsModal
    isOpen={isLogLevelsModalOpen}
    onClose={() => {
      isLogLevelsModalOpen = false;
      fetchHeatmap(activeTab);
      if (activeQuickPreset === "errors" || activeQuickPreset === "warnings") {
        scheduleSearch(true);
      }
    }}
  />
</div>

<style>
  @keyframes auroraPulse {
    0%, 100% {
      transform: translate(-50%, -50%) scale(1);
      opacity: 0.35;
    }
    50% {
      transform: translate(-50%, -50%) scale(1.15);
      opacity: 0.65;
    }
  }

  .animate-aurora {
    animation: auroraPulse 8s ease-in-out infinite;
    will-change: transform, opacity;
  }

  @media (prefers-reduced-motion: reduce) {
    .animate-aurora {
      animation: none !important;
    }
  }
</style>
