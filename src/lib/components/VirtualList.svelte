<script lang="ts">
  import { onMount } from "svelte";
  import type { LogLine } from "$lib/types";
  import Minimap from "$lib/components/Minimap.svelte";
  import { tokenizeLogLine } from "$lib/utils/tokenizer";
  import { logLevelsManager, type StandardLogLevel } from "$lib/stores/logLevels.svelte";

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

  // Gutter width calculation based on total digits
  let gutterWidth = $derived(
    Math.max(48, `${totalLines}`.length * 9.5 + 24)
  );

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
  }
  let contextMenu = $state<ContextMenuState | null>(null);

  function handleRowContextMenu(e: MouseEvent, lineData: LogLine | null | undefined, rawContent: string) {
    e.preventDefault();
    e.stopPropagation();

    // Check if user selected some text
    const sel = window.getSelection()?.toString().trim();
    let wordToMap: string | null = null;

    if (sel && sel.length > 0 && sel.length <= 40) {
      // Clean selected text from brackets/quotes
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

    const menuWidth = 240;
    const menuHeight = 260;
    const posX = Math.min(e.clientX, window.innerWidth - menuWidth - 10);
    const posY = Math.min(e.clientY, window.innerHeight - menuHeight - 10);

    contextMenu = {
      x: Math.max(10, posX),
      y: Math.max(10, posY),
      lineData: lineData ?? null,
      rawContent,
      targetWord: wordToMap && wordToMap.length > 0 ? wordToMap : null,
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
    } catch (e) {
      console.error("Errore copia:", e);
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
      if (e.key === "Escape" && contextMenu) {
        closeContextMenu();
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
    class="relative flex-1 h-full overflow-y-auto overflow-x-auto select-text font-mono text-[13px] leading-[24px] outline-none"
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
              class="flex items-center w-full min-w-max hover:bg-purple-500/[0.09] transition-colors duration-100 border-l-2 border-purple-400/75 bg-purple-500/[0.045] select-none {isFresh ? 'animate-fresh-line' : ''}"
              style="height: {itemHeight}px; line-height: {itemHeight}px;"
            >
              <!-- Gutter -->
              <div
                class="shrink-0 text-right pr-3 select-none text-purple-300/75 font-mono text-[11.5px] border-r border-white/[0.08] h-full flex items-center justify-end {isFresh ? 'text-sky-300 font-semibold' : ''}"
                style="width: {gutterWidth}px;"
              >
                {lineNum}
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
              class="group flex items-center w-full min-w-max cursor-pointer transition-colors duration-75 {isSelected ? '!bg-indigo-500/25 !border-l-2 !border-indigo-400 shadow-[inset_0_1px_0_rgba(129,140,248,0.2)]' : (isFresh ? 'animate-fresh-line ' : '') + parsed.rowTintClass}"
              style="height: {itemHeight}px; line-height: {itemHeight}px;"
            >
              <!-- 1. Colonna Numero Riga (Gutter) -->
              <div
                class="shrink-0 text-right pr-3 select-none font-mono text-[11.5px] border-r border-white/[0.08] h-full flex items-center justify-end {isSelected ? 'bg-indigo-500/20 text-indigo-200 font-bold' : isFresh ? 'text-sky-300 font-semibold bg-black/15' : 'text-[#7E879B] group-hover:text-[#B4BCD0] bg-black/15'}"
                style="width: {gutterWidth}px;"
              >
                {lineNum}
              </div>

              <!-- 2. Colonna Timestamp Compatta (Solo per righe non-stacktrace) -->
              {#if !lineData?.is_stacktrace}
                <div
                  class="shrink-0 w-[102px] px-2.5 font-mono text-[11.5px] text-[#B4BCD0] group-hover:text-[#E2E8F0] select-text truncate border-r border-white/[0.06] h-full flex items-center"
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
              <div class="px-3.5 whitespace-pre text-[13px] tracking-[-0.005em] font-mono select-text flex items-center h-full {lineData?.is_stacktrace ? 'text-purple-200/85 pl-8 bg-purple-500/[0.025]' : 'text-[#F4F6FB]'} {isFresh ? 'animate-fresh-content' : ''}">
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
      onSeek={(line) => scrollToIndex(line - 1)}
    />
  {/if}

  <!-- Context Menu Fluttuante per In-Place Mapping & Azioni -->
  {#if contextMenu}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      role="menu"
      tabindex="-1"
      class="fixed z-50 min-w-[210px] bg-[#0E121A]/95 backdrop-blur-md border border-white/[0.12] rounded-xl shadow-2xl p-1.5 text-xs text-[#F4F6FB] animate-scale-up select-none"
      style="left: {contextMenu.x}px; top: {contextMenu.y}px;"
      onclick={(e) => e.stopPropagation()}
    >
      {#if contextMenu.targetWord}
        <div class="px-2 py-1.5 text-[11px] text-[#7E879B] border-b border-white/[0.08] mb-1 flex items-center gap-1.5 truncate">
          <span>Mappa</span>
          <span class="font-mono font-bold text-white max-w-[120px] truncate">"{contextMenu.targetWord}"</span>
          <span>come:</span>
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
        onclick={handleCopyLineContent}
        class="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-white/[0.08] transition-colors cursor-pointer text-left text-[#B4BCD0] hover:text-white"
      >
        <svg class="w-3.5 h-3.5 text-[#7E879B]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
        </svg>
        <span>Copia riga</span>
      </button>

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
