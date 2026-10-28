<script lang="ts">
  import { onMount } from "svelte";
  import { fly, fade } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import type { LogLine, SeverityHeatmap } from "$lib/types";
  import Minimap from "$lib/components/Minimap.svelte";
  import { tokenizeLogLine } from "$lib/utils/tokenizer";
  import { logLevelsManager, type StandardLogLevel } from "$lib/stores/logLevels.svelte";
  import { extractTimestampMs, calculateDelta, type DeltaInfo } from "$lib/utils/timeDelta";

  interface Props {
    totalLines: number;
    itemHeight?: number;
    buffer?: number;
    fetchSlice: (start: number, count: number) => Promise<LogLine[]>;
    onLineClick?: (line: LogLine, isCtrlOrMeta?: boolean, isShift?: boolean) => void;
    onLineDoubleClick?: (line: LogLine) => void;
    onSelectionRangeChange?: (lineNumbers: number[], isCtrlOrMeta: boolean) => void;
    onToggleStacktraceGroup?: (groupId: number) => void;
    onScrollChange?: (info: { scrollTop: number; isAtBottom: boolean; wasUserAction?: boolean }) => void;
    onUserScrollUp?: () => void;
    onUserScrollBottom?: () => void;
    onSelectionPause?: () => void;
    onOpenLogLevels?: () => void;
    onQuickMapLevel?: (word: string, level: StandardLogLevel) => void;
    onFilterInclude?: (text: string) => void;
    onFilterExclude?: (text: string) => void;
    onTimeDeltaChange?: (delta: DeltaInfo | null) => void;
    onToast?: (message: string) => void;
    onToggleBookmark?: (lineNum: number) => void;
    onCopySelected?: (withNumbers?: boolean) => void;
    bookmarks?: number[];
    heatmap?: SeverityHeatmap | null;
    wrapLines?: boolean;
    searchQuery?: string;
    isRegex?: boolean;
    caseSensitive?: boolean;
    activeMatchLine?: number | null;
    selectedLineNumber?: number | null;
    selectedLineNumbers?: Set<number>;
    matchLines?: number[];
    isLive?: boolean;
    liveAnimations?: boolean;
  }

  let {
    totalLines = 0,
    itemHeight = 24,
    buffer = 40,
    fetchSlice,
    onLineClick,
    onLineDoubleClick,
    onSelectionRangeChange,
    onToggleStacktraceGroup,
    onScrollChange,
    onUserScrollUp,
    onUserScrollBottom,
    onSelectionPause,
    onOpenLogLevels,
    onQuickMapLevel,
    onFilterInclude,
    onFilterExclude,
    onTimeDeltaChange,
    onToast,
    onToggleBookmark,
    onCopySelected,
    bookmarks = [],
    heatmap = null,
    wrapLines = false,
    searchQuery = "",
    isRegex = false,
    caseSensitive = false,
    activeMatchLine = null,
    selectedLineNumber = null,
    selectedLineNumbers = new Set<number>(),
    matchLines = [],
    isLive = false,
    liveAnimations = true
  }: Props = $props();

  let containerEl = $state<HTMLDivElement | null>(null);
  let scrollTop = $state(0);
  let viewportHeight = $state(800);
  let isProgrammaticScroll = false;

  // Row cache: Map<rowIndex (0-based), LogLine>
  let linesMap = $state<Map<number, LogLine>>(new Map());

  // Mouse drag selection state
  let isDraggingSelection = $state(false);
  let dragAnchorLine = $state<number | null>(null);
  let dragHasMoved = $state(false);

  function handleRowMouseDown(e: MouseEvent, lineNum: number) {
    if (e.button !== 0) return; // Solo tasto sinistro
    const target = e.target as HTMLElement;
    if (target.closest("button") || target.closest("input")) return;

    dragAnchorLine = lineNum;
    isDraggingSelection = true;
    dragHasMoved = false;
  }

  function handleRowMouseEnter(e: MouseEvent, lineNum: number) {
    if (!isDraggingSelection || dragAnchorLine === null) return;
    if (lineNum === dragAnchorLine && !dragHasMoved) return;
    dragHasMoved = true;
    window.getSelection()?.removeAllRanges();

    const start = Math.min(dragAnchorLine, lineNum);
    const end = Math.max(dragAnchorLine, lineNum);
    const range: number[] = [];
    for (let n = start; n <= end; n++) {
      range.push(n);
    }
    const isCtrl = e.ctrlKey || e.metaKey;
    onSelectionRangeChange?.(range, isCtrl);
  }

  function handleGlobalMouseUp() {
    if (isDraggingSelection) {
      if (dragHasMoved) {
        window.getSelection()?.removeAllRanges();
      }
      isDraggingSelection = false;
      dragAnchorLine = null;
      setTimeout(() => {
        dragHasMoved = false;
      }, 50);
    }
  }

  // Live animations: tracking newly appended rows for gentle glow and micro-fade
  let prevTotalLines = 0;
  let freshRows = $state<Set<number>>(new Set());
  let clearFreshTimer: ReturnType<typeof setTimeout> | null = null;

  $effect(() => {
    const currentTotal = totalLines;
    const previousTotal = prevTotalLines;
    prevTotalLines = currentTotal;

    // Solo se siamo in modalità live, con animazioni abilitate, e non all'apertura iniziale
    if (isLive && liveAnimations && previousTotal > 0 && currentTotal > previousTotal) {
      const delta = currentTotal - previousTotal;
      // Burst protection: se arrivano più di 30 righe in un batch,
      // disattiviamo l'animazione per non degradare le prestazioni (Zero-Lag Rule).
      if (delta <= 30) {
        const nextFresh = new Set(freshRows);
        for (let r = previousTotal; r < currentTotal; r++) {
          nextFresh.add(r);
        }
        freshRows = nextFresh;

        if (clearFreshTimer) clearTimeout(clearFreshTimer);
        clearFreshTimer = setTimeout(() => {
          freshRows = new Set();
        }, 950);
      } else {
        if (freshRows.size > 0) {
          freshRows = new Set();
        }
      }
    }
  });

  // Virtual window calculation
  let visibleStartIndex = $derived(Math.floor(scrollTop / itemHeight));
  let visibleCount = $derived(Math.ceil(viewportHeight / itemHeight));

  let startIndex = $derived(Math.max(0, visibleStartIndex - buffer));
  let endIndex = $derived(Math.min(totalLines, visibleStartIndex + visibleCount + buffer));
  let renderedCount = $derived(Math.max(0, endIndex - startIndex));

  let visibleRowIndices = $derived.by(() => {
    const arr = new Array(renderedCount);
    for (let i = 0; i < renderedCount; i++) {
      arr[i] = startIndex + i;
    }
    return arr;
  });

  let totalContentHeight = $derived(totalLines * itemHeight);
  let startOffsetY = $derived(startIndex * itemHeight);

  // Gutter width calculation based on total digits + severity strip + optional T0/bookmark badge
  let gutterWidth = $derived(
    Math.max(58, `${totalLines}`.length * 9.5 + 38)
  );

  // Time delta anchor state (T0)
  let timeAnchorLine = $state<LogLine | null>(null);

  // Derived Delta Info between two points in time
  let activeTimeDelta = $derived.by<DeltaInfo | null>(() => {
    // 1. If anchor is set: compare anchor line with selectedLine
    if (timeAnchorLine) {
      const tAnchor = extractTimestampMs(timeAnchorLine.content);
      if (tAnchor !== null) {
        const targetLineNum = selectedLineNumber ?? (selectedLineNumbers?.size ? Array.from(selectedLineNumbers)[0] : null);
        if (targetLineNum && targetLineNum !== timeAnchorLine.line_number) {
          const compLine = Array.from(linesMap.values()).find(l => l.line_number === targetLineNum);
          if (compLine) {
            const tComp = extractTimestampMs(compLine.content);
            if (tComp !== null) {
              return calculateDelta(tAnchor, tComp, timeAnchorLine.line_number, targetLineNum);
            }
          }
        }
      }
    }

    // 2. If no anchor, but multiple lines selected: compare min and max selected line
    if (selectedLineNumbers && selectedLineNumbers.size >= 2) {
      const sorted = Array.from(selectedLineNumbers).sort((a, b) => a - b);
      const minLineNum = sorted[0];
      const maxLineNum = sorted[sorted.length - 1];

      const minLine = Array.from(linesMap.values()).find(l => l.line_number === minLineNum);
      const maxLine = Array.from(linesMap.values()).find(l => l.line_number === maxLineNum);

      if (minLine && maxLine) {
        const tMin = extractTimestampMs(minLine.content);
        const tMax = extractTimestampMs(maxLine.content);
        if (tMin !== null && tMax !== null) {
          return calculateDelta(tMin, tMax, minLineNum, maxLineNum);
        }
      }
    }

    return null;
  });

  $effect(() => {
    onTimeDeltaChange?.(activeTimeDelta);
  });

  // Pending fetch tracking to avoid redundant concurrent IPC requests
  let pendingFetchRange: { start: number; end: number } | null = null;

  // Request visible slice from Rust backend with chunk-alignment
  $effect(() => {
    const start = startIndex;
    const count = renderedCount;
    if (count <= 0 || totalLines <= 0) return;

    // Check if we already have all rows in this range
    let missing = false;
    for (let r = start; r < start + count; r++) {
      if (!linesMap.has(r)) {
        missing = true;
        break;
      }
    }

    if (!missing) return;

    // If a request covering this range is already in flight, skip duplicate IPC
    if (
      pendingFetchRange &&
      pendingFetchRange.start <= start &&
      pendingFetchRange.end >= start + count
    ) {
      return;
    }

    // Align request to blocks of 64 lines for high cache efficiency
    const CHUNK_ALIGN = 64;
    const fetchStart = Math.max(0, Math.floor((start - 16) / CHUNK_ALIGN) * CHUNK_ALIGN);
    const fetchEnd = Math.min(
      totalLines,
      Math.ceil((start + count + 16) / CHUNK_ALIGN) * CHUNK_ALIGN
    );
    const fetchCount = fetchEnd - fetchStart;

    pendingFetchRange = { start: fetchStart, end: fetchEnd };

    fetchSlice(fetchStart, fetchCount).then((newLines) => {
      if (!newLines || newLines.length === 0) {
        pendingFetchRange = null;
        return;
      }
      const nextMap = new Map(linesMap);
      for (let i = 0; i < newLines.length; i++) {
        nextMap.set(fetchStart + i, newLines[i]);
      }

      // Bounded cache: keep up to 3500 entries centered around active viewport
      // Fast O(N) window pruning without costly keys.sort()
      if (nextMap.size > 3500) {
        const keepMin = Math.max(0, start - 1200);
        const keepMax = start + count + 1200;
        for (const key of nextMap.keys()) {
          if (key < keepMin || key > keepMax) {
            nextMap.delete(key);
          }
        }
      }

      linesMap = nextMap;
      pendingFetchRange = null;
    }).catch((err) => {
      console.error("Failed to fetch slice:", err);
      pendingFetchRange = null;
    });
  });

  function performAutoScrollBottom(smooth = false) {
    if (!containerEl) return;
    isProgrammaticScroll = true;
    if (smooth) {
      containerEl.scrollTo({
        top: containerEl.scrollHeight,
        behavior: "smooth",
      });
    } else {
      containerEl.scrollTop = containerEl.scrollHeight;
    }
    // Release lock once scroll events have finished
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        isProgrammaticScroll = false;
      });
    });
  }

  // Reactive Smart Tail lock: automatically pin to bottom when new lines arrive in LIVE mode
  $effect(() => {
    if (isLive && totalLines > 0 && containerEl) {
      // Defer to next frame so the canvas spacer has applied the updated height
      requestAnimationFrame(() => {
        if (containerEl && isLive) {
          performAutoScrollBottom(false);
        }
      });
    }
  });

  // Handle scroll events
  function handleScroll(e: Event) {
    if (contextMenu) closeContextMenu();
    const target = e.currentTarget as HTMLDivElement;
    scrollTop = target.scrollTop;
    const isAtBottom = target.scrollTop + target.clientHeight >= target.scrollHeight - 32;
    const wasUserAction = !isProgrammaticScroll;

    onScrollChange?.({ scrollTop, isAtBottom, wasUserAction });
  }

  // Detect mouse wheel scrolling
  function handleWheel(e: WheelEvent) {
    if (e.deltaY < 0) {
      // User scrolled up with wheel/trackpad
      onUserScrollUp?.();
    } else if (e.deltaY > 0 && containerEl) {
      // If user scrolls down with wheel and reaches the bottom
      const isAtBottom = containerEl.scrollTop + containerEl.clientHeight >= containerEl.scrollHeight - 32;
      if (isAtBottom) {
        onUserScrollBottom?.();
      }
    }
  }

  // Handle keyboard navigation for smart tail lock
  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === "ArrowUp" || e.key === "PageUp" || e.key === "Home") {
      onUserScrollUp?.();
    } else if (e.key === "End") {
      onUserScrollBottom?.();
    }
  }

  function handleResize() {
    if (containerEl) {
      viewportHeight = containerEl.clientHeight;
    }
  }

  // Public methods exposed via component
  export function scrollToIndex(index: number, smooth = false) {
    if (!containerEl) return;
    const targetTop = index * itemHeight;
    containerEl.scrollTo({
      top: targetTop,
      behavior: smooth ? "smooth" : "auto",
    });
  }

  export function scrollToBottom(smooth = false) {
    performAutoScrollBottom(smooth);
  }

  export function scrollToTop(smooth = false) {
    if (!containerEl) return;
    containerEl.scrollTo({
      top: 0,
      behavior: smooth ? "smooth" : "auto",
    });
  }

  export function getIsAtBottom(): boolean {
    if (!containerEl) return false;
    return containerEl.scrollTop + containerEl.clientHeight >= containerEl.scrollHeight - 32;
  }

  export function clearCache() {
    linesMap = new Map();
    pendingFetchRange = null;
    freshRows = new Set();
    prevTotalLines = 0;
    if (clearFreshTimer) {
      clearTimeout(clearFreshTimer);
      clearFreshTimer = null;
    }
  }

  // Invalidate cache for rows at or above a given row index (e.g. on append or fold toggle)
  export function invalidateFromLine(fromLine: number) {
    pendingFetchRange = null;
    if (linesMap.size === 0) return;
    const nextMap = new Map(linesMap);
    let changed = false;
    const fromRow = Math.max(0, fromLine - 1);
    for (const rowIndex of nextMap.keys()) {
      if (rowIndex >= fromRow) {
        nextMap.delete(rowIndex);
        changed = true;
      }
    }
    if (changed) {
      linesMap = nextMap;
    }
  }

  export function getLineByNumber(lineNum: number): LogLine | undefined {
    for (const line of linesMap.values()) {
      if (line.line_number === lineNum) return line;
    }
    return undefined;
  }

  // Context Menu State
  interface ContextMenuState {
    x: number;
    y: number;
    lineData: LogLine | null;
    rawContent: string;
    targetWord: string | null;
    selectedText: string | null;
  }
  let contextMenu = $state<ContextMenuState | null>(null);

  function handleRowContextMenu(e: MouseEvent, lineData: LogLine | null | undefined, rawContent: string) {
    e.preventDefault();
    e.stopPropagation();

    // Se la riga cliccata col destro non fa parte delle righe selezionate correnti,
    // impostiamola come riga selezionata (comportamento standard IDE)
    if (lineData && (!selectedLineNumbers || !selectedLineNumbers.has(lineData.line_number))) {
      onLineClick?.(lineData, false, false);
    }

    // Check if user selected some text
    const sel = window.getSelection()?.toString().trim();
    let wordToMap: string | null = null;
    let selectedText: string | null = null;

    if (sel && sel.length > 0 && sel.length <= 120) {
      selectedText = sel;
      // Clean selected text from brackets/quotes for mapping
      wordToMap = sel.replace(/^[\[\("']+|[\]\)"',:;]+$/g, "").trim();
    } else {
      // Try to find a capitalized word or word in brackets from the line
      const bracketMatch = /\[([A-Za-z0-9_\-]{2,20})\]/.exec(rawContent);
      if (bracketMatch) {
        wordToMap = bracketMatch[1];
      } else {
        const wordMatch = /\b([A-Z]{3,15})\b/.exec(rawContent);
        if (wordMatch) {
          wordToMap = wordMatch[1];
        }
      }
    }

    const menuWidth = 260;
    const menuHeight = 340;
    const posX = Math.min(e.clientX, window.innerWidth - menuWidth - 10);
    const posY = Math.min(e.clientY, window.innerHeight - menuHeight - 10);

    contextMenu = {
      x: Math.max(10, posX),
      y: Math.max(10, posY),
      lineData: lineData ?? null,
      rawContent,
      targetWord: wordToMap && wordToMap.length > 0 ? wordToMap : null,
      selectedText: selectedText && selectedText.length > 0 ? selectedText : null,
    };
  }

  function closeContextMenu() {
    contextMenu = null;
  }

  function handleMapWord(level: StandardLogLevel) {
    if (!contextMenu?.targetWord) return;
    const word = contextMenu.targetWord;
    logLevelsManager.addKeyword(level, word);
    onQuickMapLevel?.(word, level);
    closeContextMenu();
  }

  async function handleCopyLineContent() {
    if (!contextMenu?.rawContent) return;
    try {
      await navigator.clipboard.writeText(contextMenu.rawContent);
      onToast?.("Riga copiata negli appunti");
    } catch (e) {
      console.error("Errore copia:", e);
    }
    closeContextMenu();
  }

  function handleToggleTimeAnchorFromMenu() {
    if (!contextMenu?.lineData) return;
    const line = contextMenu.lineData;
    if (timeAnchorLine?.line_number === line.line_number) {
      timeAnchorLine = null;
      onToast?.("Riferimento Δt (T₀) rimosso");
    } else {
      timeAnchorLine = line;
      onToast?.(`Riferimento Δt (T₀) impostato su riga #${line.line_number}`);
    }
    closeContextMenu();
  }

  onMount(() => {
    handleResize();
    const ro = new ResizeObserver(() => handleResize());
    if (containerEl) {
      ro.observe(containerEl);
    }

    function checkSelection() {
      const sel = window.getSelection();
      if (sel && sel.toString().trim().length > 0 && containerEl?.contains(sel.anchorNode)) {
        onSelectionPause?.();
      }
    }

    function handleGlobalClick(e: MouseEvent) {
      if (contextMenu) {
        closeContextMenu();
      }
    }

    function handleGlobalKeydown(e: KeyboardEvent) {
      if (e.key === "Escape") {
        if (contextMenu) {
          closeContextMenu();
        } else if (timeAnchorLine) {
          timeAnchorLine = null;
          onToast?.("Riferimento Δt (T₀) rimosso");
        }
      } else if ((e.key === "t" || e.key === "d") && !e.ctrlKey && !e.metaKey && !e.altKey) {
        const target = e.target as HTMLElement;
        if (target.tagName !== "INPUT" && target.tagName !== "TEXTAREA" && selectedLineNumber) {
          const line = Array.from(linesMap.values()).find(l => l.line_number === selectedLineNumber);
          if (line) {
            if (timeAnchorLine?.line_number === line.line_number) {
              timeAnchorLine = null;
              onToast?.("Riferimento Δt (T₀) rimosso");
            } else {
              timeAnchorLine = line;
              onToast?.(`Riferimento Δt (T₀) impostato su riga #${line.line_number}`);
            }
          }
        }
      }
    }

    document.addEventListener("selectionchange", checkSelection);
    window.addEventListener("mouseup", handleGlobalMouseUp);
    window.addEventListener("click", handleGlobalClick);
    window.addEventListener("keydown", handleGlobalKeydown);

    return () => {
      ro.disconnect();
      document.removeEventListener("selectionchange", checkSelection);
      window.removeEventListener("mouseup", handleGlobalMouseUp);
      window.removeEventListener("click", handleGlobalClick);
      window.removeEventListener("keydown", handleGlobalKeydown);
      if (clearFreshTimer) {
        clearTimeout(clearFreshTimer);
        clearFreshTimer = null;
      }
    };
  });
</script>

<div class="relative w-full h-full flex overflow-hidden">
  <!-- Virtual Scrollable Canvas -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    bind:this={containerEl}
    onscroll={handleScroll}
    onwheel={handleWheel}
    onkeydown={handleKeyDown}
    class="relative flex-1 h-full overflow-y-auto overflow-x-auto {isDraggingSelection ? 'select-none' : 'select-text'} font-mono text-[13px] leading-[24px] outline-none"
    role="region"
    aria-label="Log lines stream"
    tabindex="0"
    style="background-color: var(--color-canvas); color: var(--color-text-primary);"
  >
    <!-- Relative Canvas Spacer sizing the full virtual document height -->
    <div
      class="w-full relative min-w-full"
      style="height: {totalContentHeight}px; min-height: 100%;"
    >
      <!-- Visible Slice Content positioned within the virtual scroll canvas -->
      <div
        class="absolute left-0 top-0 w-full min-w-full will-change-transform"
        style="transform: translateY({startOffsetY}px);"
      >
        {#each visibleRowIndices as rowIndex (rowIndex)}
          {@const lineData = linesMap.get(rowIndex)}
          {@const rawContent = lineData ? lineData.content : ""}
          {@const lineNum = lineData ? lineData.line_number : rowIndex + 1}
          {@const isSelected = (selectedLineNumbers ? selectedLineNumbers.has(lineNum) : selectedLineNumber === lineNum) || activeMatchLine === lineNum}
          {@const isCollapsedGroup = lineData?.is_collapsed_group === true}
          {@const isFresh = liveAnimations && freshRows.has(rowIndex)}

          {#if isCollapsedGroup}
            <!-- Collapsed Stacktrace Group Pill Row -->
            <div
              role="row"
              tabindex="-1"
              oncontextmenu={(e) => handleRowContextMenu(e, lineData, rawContent)}
              onmousedown={(e) => handleRowMouseDown(e, lineNum)}
              onmouseenter={(e) => handleRowMouseEnter(e, lineNum)}
              onclick={(e) => {
                if (dragHasMoved) return;
                const activeLine = lineData ?? {
                  line_number: lineNum,
                  content: "",
                  byte_offset: 0,
                  is_stacktrace: true
                };
                onLineClick?.(activeLine, e.ctrlKey || e.metaKey, e.shiftKey);
              }}
              onkeydown={(e) => {
                if ((e.key === "Enter" || e.key === " ") && lineData) {
                  onLineClick?.(lineData, e.ctrlKey || e.metaKey, e.shiftKey);
                }
              }}
              class="flex items-center {wrapLines ? 'w-full min-w-full' : 'w-full min-w-max'} hover:bg-purple-500/[0.09] transition-colors duration-100 bg-purple-500/[0.045] select-none {isFresh ? 'animate-fresh-line' : ''}"
              style={wrapLines ? `min-height: ${itemHeight}px;` : `height: ${itemHeight}px; line-height: ${itemHeight}px;`}
            >
              <!-- 1. Gutter Sticky con Severity Strip P0.3 -->
              <div
                class="shrink-0 select-none font-mono text-[11.5px] border-r border-white/[0.08] h-full flex items-center sticky left-0 z-10 bg-[#0B0D13] text-purple-300/75 {isFresh ? 'text-sky-300 font-semibold !bg-[#0D1526]' : ''}"
                style="width: {gutterWidth}px;"
              >
                <!-- Severity Accent Strip: 3.5px solid/glow purple for stacktrace -->
                <div
                  class="w-[3.5px] h-full shrink-0 bg-purple-400 shadow-[0_0_8px_rgba(192,132,252,0.6)]"
                ></div>

                <!-- Row Number, Bookmark indicator & T0 Anchor Badge -->
                <div class="flex-1 flex items-center justify-end pr-2.5 gap-1 min-w-0">
                  {#if bookmarks.includes(lineNum)}
                    <button
                      type="button"
                      onclick={(e) => { e.stopPropagation(); onToggleBookmark?.(lineNum); }}
                      class="text-cyan-400 hover:text-cyan-200 transition-transform active:scale-90 p-0.5 cursor-pointer leading-none shrink-0"
                      title="Segnalibro attivo (F2 per saltare, clicca per rimuovere)"
                    >
                      <svg class="w-3 h-3 fill-cyan-400 drop-shadow-[0_0_6px_rgba(34,211,238,0.8)]" viewBox="0 0 24 24">
                        <path d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
                      </svg>
                    </button>
                  {/if}

                  {#if timeAnchorLine && timeAnchorLine.line_number === lineNum}
                    <span class="px-1 py-0.2 rounded bg-amber-500/30 text-[9px] font-sans font-bold text-amber-200 ring-1 ring-inset ring-amber-400/50 shadow-[0_0_6px_rgba(251,191,36,0.6)] animate-pulse shrink-0">
                      T₀
                    </span>
                  {/if}
                  <button
                    type="button"
                    onclick={(e) => { e.stopPropagation(); onToggleBookmark?.(lineNum); }}
                    class="cursor-pointer hover:text-cyan-300 transition-colors text-right bg-transparent p-0 border-0 font-mono text-[11px]"
                    title="Clicca per impostare/rimuovere segnalibro"
                  >
                    {lineNum}
                  </button>
                </div>
              </div>

              <!-- Expandable Pill -->
              <div class="px-3 flex items-center h-full">
                <button
                  onclick={() => {
                    if (lineData?.group_id !== undefined && lineData.group_id !== null) {
                      onToggleStacktraceGroup?.(lineData.group_id);
                    }
                  }}
                  class="flex items-center gap-1.5 px-2.5 py-0.5 rounded-full bg-purple-500/20 hover:bg-purple-500/30 ring-1 ring-inset ring-purple-400/40 shadow-[inset_0_1px_0_rgba(255,255,255,0.12)] text-purple-200 text-[11px] font-sans font-medium cursor-pointer transition-all duration-150 active:scale-[0.98] select-none group leading-none"
                  title="Clicca per espandere lo stacktrace"
                >
                  <svg class="w-3 h-3 text-purple-300 group-hover:translate-x-0.5 transition-transform duration-150" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M9 5l7 7-7 7" />
                  </svg>
                  <span>Stacktrace · {lineData?.group_line_count} frames nascosti</span>
                  <span class="ml-1 px-1.5 py-0.5 rounded-full bg-purple-400/25 text-[9.5px] font-semibold uppercase tracking-wider text-purple-100">Espandi</span>
                </button>
              </div>
            </div>
          {:else}
            {@const parsed = tokenizeLogLine(rawContent, searchQuery, isRegex, caseSensitive)}

            <!-- Riga normale (non collapsed group) -->
            <div
              role="row"
              tabindex="-1"
              oncontextmenu={(e) => handleRowContextMenu(e, lineData, rawContent)}
              onmousedown={(e) => handleRowMouseDown(e, lineNum)}
              onmouseenter={(e) => handleRowMouseEnter(e, lineNum)}
              onclick={(e) => {
                if (dragHasMoved) return;
                const activeLine = lineData ?? {
                  line_number: lineNum,
                  content: rawContent,
                  byte_offset: 0,
                  is_stacktrace: false
                };
                onLineClick?.(activeLine, e.ctrlKey || e.metaKey, e.shiftKey);
              }}
              ondblclick={(e) => {
                e.stopPropagation();
                if (lineData) onLineDoubleClick?.(lineData);
              }}
              onkeydown={(e) => {
                if ((e.key === "Enter" || e.key === " ") && lineData) onLineClick?.(lineData, e.ctrlKey || e.metaKey, e.shiftKey);
              }}
              class="group flex items-center {wrapLines ? 'w-full min-w-full' : 'w-full min-w-max'} cursor-pointer transition-colors duration-75 {isSelected ? '!bg-indigo-500/25 shadow-[inset_0_1px_0_rgba(129,140,248,0.2)]' : (isFresh ? 'animate-fresh-line ' : '') + parsed.rowTintClass}"
              style={wrapLines ? `min-height: ${itemHeight}px;` : `height: ${itemHeight}px; line-height: ${itemHeight}px;`}
            >
              <!-- 1. Colonna Numero Riga (Gutter) Sticky con Severity Strip P0.3 & Bookmark P1.1 -->
              <div
                class="shrink-0 select-none font-mono text-[11.5px] border-r border-white/[0.08] h-full flex items-center sticky left-0 z-10 bg-[#0B0D13] {isSelected ? '!bg-[#161B2E] text-indigo-200 font-bold' : isFresh ? 'text-sky-300 font-semibold !bg-[#0D1526]' : 'text-[#7E879B] group-hover:text-[#B4BCD0]'}"
                style="width: {gutterWidth}px;"
              >
                <!-- Severity Accent Strip: 3.5px solid high-contrast bar -->
                <div
                  class="w-[3.5px] h-full shrink-0 transition-colors duration-100 {isSelected ? 'bg-indigo-400 shadow-[0_0_8px_rgba(129,140,248,0.8)]' : parsed.stripClass}"
                ></div>

                <!-- Row Number, Bookmark indicator & T0 Anchor Badge -->
                <div class="flex-1 flex items-center justify-end pr-2.5 gap-1 min-w-0">
                  {#if bookmarks.includes(lineNum)}
                    <button
                      type="button"
                      onclick={(e) => { e.stopPropagation(); onToggleBookmark?.(lineNum); }}
                      class="text-cyan-400 hover:text-cyan-200 transition-transform active:scale-90 p-0.5 cursor-pointer leading-none shrink-0"
                      title="Segnalibro attivo (F2 per saltare, clicca per rimuovere)"
                    >
                      <svg class="w-3 h-3 fill-cyan-400 drop-shadow-[0_0_6px_rgba(34,211,238,0.8)]" viewBox="0 0 24 24">
                        <path d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
                      </svg>
                    </button>
                  {/if}

                  {#if timeAnchorLine && timeAnchorLine.line_number === lineNum}
                    <span class="px-1 py-0.2 rounded bg-amber-500/30 text-[9px] font-sans font-bold text-amber-200 ring-1 ring-inset ring-amber-400/50 shadow-[0_0_6px_rgba(251,191,36,0.6)] animate-pulse shrink-0">
                      T₀
                    </span>
                  {/if}
                  <button
                    type="button"
                    onclick={(e) => { e.stopPropagation(); onToggleBookmark?.(lineNum); }}
                    class="cursor-pointer hover:text-cyan-300 transition-colors text-right bg-transparent p-0 border-0 font-mono text-[11px]"
                    title="Clicca per impostare/rimuovere segnalibro"
                  >
                    {lineNum}
                  </button>
                </div>
              </div>

              <!-- 2. Colonna Timestamp Compatta (Solo per righe non-stacktrace) -->
              {#if !lineData?.is_stacktrace}
                <div
                  class="shrink-0 w-[102px] px-2.5 font-mono text-[11.5px] text-[#B4BCD0] group-hover:text-[#E2E8F0] {isDraggingSelection ? 'select-none' : 'select-text'} truncate border-r border-white/[0.06] h-full flex items-center"
                  title={parsed.fullTimestamp ?? ""}
                >
                  {#if parsed.extractedTimestamp}
                    {parsed.extractedTimestamp}
                  {:else}
                    <span class="opacity-30">—</span>
                  {/if}
                </div>

                <!-- 3. Colonna Livello Semaforica (Fissa 62px) -->
                <div class="shrink-0 w-[62px] px-1.5 h-full flex items-center justify-center border-r border-white/[0.06] select-none">
                  {#if parsed.level}
                    <span class="w-full text-center py-[1px] rounded-[4px] font-sans font-bold text-[10px] tracking-wider uppercase leading-tight {parsed.levelBadgeClass}">
                      {parsed.level}
                    </span>
                  {/if}
                </div>
              {/if}

              <!-- 4. Colonna Messaggio + Chip Payload -->
              <div class="px-3.5 {wrapLines ? 'whitespace-pre-wrap break-all py-1' : 'whitespace-pre'} text-[13px] tracking-[-0.005em] font-mono {isDraggingSelection ? 'select-none' : 'select-text'} flex items-center h-full {lineData?.is_stacktrace ? 'text-purple-200/85 pl-8 bg-purple-500/[0.025]' : 'text-[#F4F6FB]'} {isFresh ? 'animate-fresh-content' : ''}">
                {#if lineData}
                  <span class="inline-block">{@html parsed.html}</span>

                  {#if parsed.hasJson}
                    <button
                      onclick={(e) => { e.stopPropagation(); onLineDoubleClick?.(lineData); }}
                      class="ml-2.5 px-2 py-0.5 rounded-[5px] bg-emerald-500/15 text-emerald-200 hover:bg-emerald-500/25 ring-1 ring-inset ring-emerald-400/35 shadow-[inset_0_1px_0_rgba(255,255,255,0.1)] text-[10px] font-sans font-semibold cursor-pointer transition-all duration-150 active:scale-95 select-none flex items-center gap-1 leading-none"
                      title="Ispeziona JSON nel pannello laterale"
                    >
                      <span class="font-mono text-emerald-300 font-bold">&#123;&#125;</span>
                      <span>JSON</span>
                    </button>
                  {:else if parsed.hasXml}
                    <button
                      onclick={(e) => { e.stopPropagation(); onLineDoubleClick?.(lineData); }}
                      class="ml-2.5 px-2 py-0.5 rounded-[5px] bg-amber-500/15 text-amber-200 hover:bg-amber-500/25 ring-1 ring-inset ring-amber-400/35 shadow-[inset_0_1px_0_rgba(255,255,255,0.1)] text-[10px] font-sans font-semibold cursor-pointer transition-all duration-150 active:scale-95 select-none flex items-center gap-1 leading-none"
                      title="Ispeziona XML nel pannello laterale"
                    >
                      <span class="font-mono text-amber-300 font-bold">&lt;&gt;</span>
                      <span>XML</span>
                    </button>
                  {/if}
                {:else}
                  <span class="inline-block h-3 w-48 rounded bg-white/[0.06] animate-pulse"></span>
                {/if}
              </div>
            </div>
          {/if}
        {/each}
      </div>
    </div>
  </div>

  <!-- Minimap Scrollbar Track on the right -->
  {#if totalLines > 0}
    <Minimap
      {totalLines}
      {visibleStartIndex}
      {visibleCount}
      {matchLines}
      {bookmarks}
      {heatmap}
      onSeek={(line) => scrollToIndex(line - 1)}
    />
  {/if}

  <!-- Floating Time Delta HUD Pill (P0.2) -->
  {#if activeTimeDelta}
    <div
      transition:fly={{ y: 14, duration: 180, easing: cubicOut }}
      class="absolute bottom-5 left-1/2 -translate-x-1/2 z-30 flex items-center gap-3 px-3.5 py-1.5 rounded-full bg-[#141824]/95 backdrop-blur-xl border border-amber-400/40 shadow-[0_12px_28px_rgba(0,0,0,0.65),0_0_16px_rgba(245,158,11,0.2)] font-sans text-xs text-white select-none"
    >
      <div class="flex items-center gap-2">
        <span class="w-2 h-2 rounded-full bg-amber-400 shadow-[0_0_8px_rgba(251,191,36,0.9)] animate-pulse"></span>
        <span class="font-semibold text-amber-200">Δt:</span>
        <span class="font-mono font-bold text-white tracking-tight">{activeTimeDelta.formatted}</span>
      </div>

      <div class="w-px h-3.5 bg-white/[0.15]"></div>

      <span class="text-[#B4BCD0] font-mono text-[11px]">
        #{activeTimeDelta.fromLine} → #{activeTimeDelta.toLine} ({activeTimeDelta.lineCount} {activeTimeDelta.lineCount === 1 ? 'riga' : 'righe'})
      </span>

      <button
        onclick={async () => {
          try {
            await navigator.clipboard.writeText(activeTimeDelta!.formatted);
            onToast?.(`Δt copiato: ${activeTimeDelta!.formattedShort}`);
          } catch {}
        }}
        class="px-2 py-0.5 rounded-full bg-amber-500/20 hover:bg-amber-500/30 text-amber-200 text-[10.5px] font-semibold ring-1 ring-inset ring-amber-400/40 transition active:scale-95 cursor-pointer"
        title="Copia valore delta negli appunti"
      >
        Copia
      </button>

      {#if timeAnchorLine}
        <button
          onclick={() => { timeAnchorLine = null; onToast?.("Riferimento T₀ rimosso"); }}
          class="text-[#7E879B] hover:text-white transition p-0.5 rounded-full cursor-pointer hover:bg-white/[0.08]"
          title="Rimuovi riferimento T₀ (Esc)"
        >
          <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      {/if}
    </div>
  {/if}

  <!-- Context Menu Fluttuante per In-Place Mapping, Filtro Rapido, Delta T0 & Azioni -->
  {#if contextMenu}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      role="menu"
      tabindex="-1"
      class="fixed z-50 min-w-[240px] max-w-[320px] bg-[#0E121A]/95 backdrop-blur-md border border-white/[0.14] rounded-xl shadow-[0_16px_40px_rgba(0,0,0,0.7),inset_0_1px_0_rgba(255,255,255,0.12)] p-1.5 text-xs text-[#F4F6FB] animate-scale-up select-none"
      style="left: {contextMenu.x}px; top: {contextMenu.y}px;"
      onclick={(e) => e.stopPropagation()}
    >
      <!-- Sezione Filtro Rapido / Esclusione (P0.1) -->
      {#if contextMenu.selectedText || contextMenu.targetWord}
        {@const filterTerm = contextMenu.selectedText || contextMenu.targetWord || ""}
        <div class="px-2 py-1 text-[10px] font-semibold text-[#7E879B] uppercase tracking-wider">
          Filtro Rapido
        </div>

        <button
          onclick={() => {
            if (filterTerm) onFilterInclude?.(filterTerm);
            closeContextMenu();
          }}
          class="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-indigo-500/15 text-[#E2E8F0] hover:text-white transition-colors cursor-pointer text-left group"
        >
          <svg class="w-3.5 h-3.5 text-indigo-400 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
          </svg>
          <span class="truncate">Filtra per <span class="font-mono text-indigo-300">"{filterTerm.slice(0, 24)}"</span></span>
        </button>

        <button
          onclick={() => {
            if (filterTerm) onFilterExclude?.(filterTerm);
            closeContextMenu();
          }}
          class="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-rose-500/15 text-rose-300 hover:text-rose-200 transition-colors cursor-pointer text-left group"
        >
          <svg class="w-3.5 h-3.5 text-rose-400 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M18.364 18.364A9 9 0 005.636 5.636m12.728 12.728A9 9 0 015.636 5.636m12.728 12.728L5.636 5.636" />
          </svg>
          <span class="truncate">Escludi <span class="font-mono text-rose-200">"{filterTerm.slice(0, 24)}"</span></span>
        </button>

        <div class="h-px bg-white/[0.08] my-1"></div>
      {/if}

      <!-- Sezione Copia (Ctrl+C / Ctrl+Shift+C) P0.0 -->
      <div class="px-2 py-0.5 text-[9.5px] font-semibold text-[#7E879B] uppercase tracking-wider">
        Appunti
      </div>

      {#if contextMenu.selectedText}
        <button
          onclick={async () => {
            try {
              await navigator.clipboard.writeText(contextMenu!.selectedText!);
              onToast?.(`Testo copiato negli appunti`);
            } catch {}
            closeContextMenu();
          }}
          class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg hover:bg-white/[0.08] transition-colors cursor-pointer text-left text-[#B4BCD0] hover:text-white group"
        >
          <div class="flex items-center gap-2 truncate">
            <svg class="w-3.5 h-3.5 text-indigo-400 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
            </svg>
            <span class="truncate">Copia <span class="font-mono text-indigo-200">"{contextMenu.selectedText.slice(0, 16)}"</span></span>
          </div>
          <kbd class="text-[9.5px] font-mono px-1 py-0.2 rounded bg-black/30 text-[#B4BCD0]">Ctrl+C</kbd>
        </button>
      {/if}

      {#if selectedLineNumbers && selectedLineNumbers.size > 1}
        <button
          onclick={() => { closeContextMenu(); onCopySelected?.(false); }}
          class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg hover:bg-indigo-500/15 text-indigo-200 hover:text-white transition-colors cursor-pointer text-left group"
        >
          <div class="flex items-center gap-2">
            <svg class="w-3.5 h-3.5 text-indigo-400 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
            </svg>
            <span>Copia {selectedLineNumbers.size.toLocaleString()} righe</span>
          </div>
          <kbd class="text-[9.5px] font-mono px-1 py-0.2 rounded bg-black/30 text-[#B4BCD0]">Ctrl+C</kbd>
        </button>

        <button
          onclick={() => { closeContextMenu(); onCopySelected?.(true); }}
          class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg hover:bg-white/[0.08] transition-colors cursor-pointer text-left text-[#B4BCD0] hover:text-white group"
        >
          <div class="flex items-center gap-2">
            <svg class="w-3.5 h-3.5 text-[#7E879B] shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 20l4-16m2 16l4-16M6 9h14M4 15h14" />
            </svg>
            <span>Copia con numeri di riga</span>
          </div>
          <kbd class="text-[9.5px] font-mono px-1 py-0.2 rounded bg-black/30 text-[#B4BCD0]">Ctrl+Shift+C</kbd>
        </button>

        <button
          onclick={handleCopyLineContent}
          class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg hover:bg-white/[0.08] transition-colors cursor-pointer text-left text-[#B4BCD0] hover:text-white group"
        >
          <div class="flex items-center gap-2">
            <svg class="w-3.5 h-3.5 text-[#7E879B] shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
            </svg>
            <span>Copia solo riga #{contextMenu.lineData?.line_number ?? ''}</span>
          </div>
        </button>
      {:else}
        <button
          onclick={handleCopyLineContent}
          class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg hover:bg-indigo-500/15 text-indigo-200 hover:text-white transition-colors cursor-pointer text-left group"
        >
          <div class="flex items-center gap-2">
            <svg class="w-3.5 h-3.5 text-indigo-400 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
            </svg>
            <span>Copia riga intera</span>
          </div>
          <kbd class="text-[9.5px] font-mono px-1 py-0.2 rounded bg-black/30 text-[#B4BCD0]">Ctrl+C</kbd>
        </button>

        <button
          onclick={() => { closeContextMenu(); onCopySelected?.(true); }}
          class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg hover:bg-white/[0.08] transition-colors cursor-pointer text-left text-[#B4BCD0] hover:text-white group"
        >
          <div class="flex items-center gap-2">
            <svg class="w-3.5 h-3.5 text-[#7E879B] shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 20l4-16m2 16l4-16M6 9h14M4 15h14" />
            </svg>
            <span>Copia con numero riga</span>
          </div>
          <kbd class="text-[9.5px] font-mono px-1 py-0.2 rounded bg-black/30 text-[#B4BCD0]">Ctrl+Shift+C</kbd>
        </button>
      {/if}

      <div class="h-px bg-white/[0.08] my-1"></div>

      <!-- Sezione Delta Temporale T0 (P0.2) -->
      {#if contextMenu.lineData}
        {@const line = contextMenu.lineData}
        {@const isAnchor = timeAnchorLine?.line_number === line.line_number}
        {@const isBookmarked = bookmarks.includes(line.line_number)}
        <button
          onclick={handleToggleTimeAnchorFromMenu}
          class="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-amber-500/15 text-amber-200 hover:text-amber-100 transition-colors cursor-pointer text-left group"
        >
          <span class="text-xs">⏱️</span>
          <span>{isAnchor ? "Rimuovi riferimento Δt (T₀)" : `Imposta come riferimento Δt (T₀)`}</span>
          <kbd class="ml-auto text-[9.5px] font-mono px-1 py-0.2 rounded bg-black/30 text-[#B4BCD0]">T</kbd>
        </button>

        <button
          onclick={() => {
            onToggleBookmark?.(line.line_number);
            closeContextMenu();
          }}
          class="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-cyan-500/15 text-cyan-200 hover:text-cyan-100 transition-colors cursor-pointer text-left group"
        >
          <svg class="w-3.5 h-3.5 {isBookmarked ? 'fill-cyan-400 text-cyan-400' : 'text-cyan-300'}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
          </svg>
          <span>{isBookmarked ? "Rimuovi segnalibro" : "Aggiungi segnalibro"}</span>
          <kbd class="ml-auto text-[9.5px] font-mono px-1 py-0.2 rounded bg-black/30 text-[#B4BCD0]">F2</kbd>
        </button>

        <div class="h-px bg-white/[0.08] my-1"></div>
      {/if}

      {#if contextMenu.targetWord}
        <div class="px-2 py-1 text-[10px] font-semibold text-[#7E879B] uppercase tracking-wider flex items-center gap-1.5 truncate">
          <span>Mappa "{contextMenu.targetWord.slice(0, 16)}" come:</span>
        </div>

        <button
          onclick={() => handleMapWord("ERROR")}
          class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg hover:bg-white/[0.08] transition-colors cursor-pointer text-left group"
        >
          <span class="text-[#E2E8F0] group-hover:text-white">🔴 Error</span>
          <span class="text-[9.5px] font-mono text-rose-300 bg-rose-500/20 px-1.5 py-0.5 rounded">ERR</span>
        </button>

        <button
          onclick={() => handleMapWord("WARN")}
          class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg hover:bg-white/[0.08] transition-colors cursor-pointer text-left group"
        >
          <span class="text-[#E2E8F0] group-hover:text-white">🟡 Warning</span>
          <span class="text-[9.5px] font-mono text-amber-300 bg-amber-500/20 px-1.5 py-0.5 rounded">WARN</span>
        </button>

        <button
          onclick={() => handleMapWord("INFO")}
          class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg hover:bg-white/[0.08] transition-colors cursor-pointer text-left group"
        >
          <span class="text-[#E2E8F0] group-hover:text-white">🔵 Info</span>
          <span class="text-[9.5px] font-mono text-sky-300 bg-sky-500/20 px-1.5 py-0.5 rounded">INFO</span>
        </button>

        <button
          onclick={() => handleMapWord("DEBUG")}
          class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg hover:bg-white/[0.08] transition-colors cursor-pointer text-left group"
        >
          <span class="text-[#E2E8F0] group-hover:text-white">⚪ Debug</span>
          <span class="text-[9.5px] font-mono text-slate-300 bg-slate-500/20 px-1.5 py-0.5 rounded">DBG</span>
        </button>

        <button
          onclick={() => handleMapWord("TRACE")}
          class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg hover:bg-white/[0.08] transition-colors cursor-pointer text-left group"
        >
          <span class="text-[#E2E8F0] group-hover:text-white">🟣 Trace</span>
          <span class="text-[9.5px] font-mono text-purple-300 bg-purple-500/20 px-1.5 py-0.5 rounded">TRC</span>
        </button>

        <div class="h-px bg-white/[0.08] my-1"></div>
      {/if}

      <button
        onclick={() => { closeContextMenu(); onOpenLogLevels?.(); }}
        class="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-white/[0.08] transition-colors cursor-pointer text-left text-[#B4BCD0] hover:text-white"
      >
        <svg class="w-3.5 h-3.5 text-[#7E879B]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
        </svg>
        <span>Configura livelli di log...</span>
      </button>
    </div>
  {/if}
</div>

<style>
  @keyframes freshLineGlow {
    0% {
      background-color: rgba(56, 189, 248, 0.22);
      box-shadow: inset 3px 0 0 0 #38bdf8;
    }
    35% {
      background-color: rgba(56, 189, 248, 0.12);
      box-shadow: inset 3px 0 0 0 #38bdf8;
    }
    100% {
      background-color: transparent;
      box-shadow: inset 0px 0 0 0 transparent;
    }
  }

  @keyframes freshContentFade {
    0% {
      opacity: 0.55;
      transform: translateX(-4px);
    }
    100% {
      opacity: 1;
      transform: translateX(0);
    }
  }

  .animate-fresh-line {
    animation: freshLineGlow 900ms cubic-bezier(0.16, 1, 0.3, 1) forwards;
  }

  .animate-fresh-content {
    animation: freshContentFade 240ms cubic-bezier(0.16, 1, 0.3, 1) forwards;
    will-change: transform, opacity;
  }

  @media (prefers-reduced-motion: reduce) {
    .animate-fresh-line,
    .animate-fresh-content {
      animation: none !important;
    }
  }
</style>
