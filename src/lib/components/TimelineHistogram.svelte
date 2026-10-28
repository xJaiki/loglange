<script lang="ts">
  import type { TimeHistogram, TimeBucket, TimelineBrushRange } from "$lib/types";

  interface Props {
    histogram: TimeHistogram | null;
    isLoading?: boolean;
    isFilterActive?: boolean;
    onSelectLine: (lineNumber: number) => void;
    onBrushChange?: (range: TimelineBrushRange | null) => void;
    onFilterTimeRange?: (range: TimelineBrushRange) => void;
    onClose: () => void;
  }

  let {
    histogram,
    isLoading = false,
    isFilterActive = false,
    onSelectLine,
    onBrushChange,
    onFilterTimeRange,
    onClose
  }: Props = $props();

  let containerEl = $state<HTMLDivElement | null>(null);
  let containerWidth = $state(800);
  let hoveredBucket = $state<TimeBucket | null>(null);
  let hoveredBucketIndex = $state<number | null>(null);
  let tooltipX = $state(0);
  let tooltipY = $state(0);

  // Brushing State
  let isDragging = $state(false);
  let dragStartX = $state<number | null>(null);
  let currentDragX = $state<number | null>(null);
  let activeBrush = $state<TimelineBrushRange | null>(null);

  const CHART_HEIGHT = 52;
  const BAR_GAP = 1.5;

  let buckets = $derived(histogram?.buckets ?? []);
  let hasData = $derived(Boolean(histogram && histogram.has_timestamps && buckets.length > 0));

  // Max count with sqrt scaling
  let maxBucketCount = $derived.by(() => {
    if (!buckets.length) return 1;
    let m = 1;
    for (const b of buckets) {
      if (b.count > m) m = b.count;
    }
    return m;
  });

  // Calculate bucket width based on container width
  let bucketWidth = $derived.by(() => {
    if (!buckets.length || containerWidth <= 0) return 4;
    return Math.max(2, (containerWidth - (buckets.length - 1) * BAR_GAP) / buckets.length);
  });

  function getBarCoordinates(index: number) {
    const x = index * (bucketWidth + BAR_GAP);
    return { x, width: bucketWidth };
  }

  function handleMouseDown(e: MouseEvent) {
    if (!containerEl || !buckets.length) return;
    const rect = containerEl.getBoundingClientRect();
    const x = Math.max(0, Math.min(e.clientX - rect.left, rect.width));

    isDragging = true;
    dragStartX = x;
    currentDragX = x;
  }

  function handleMouseMove(e: MouseEvent) {
    if (!containerEl || !buckets.length) return;
    const rect = containerEl.getBoundingClientRect();
    const x = Math.max(0, Math.min(e.clientX - rect.left, rect.width));

    if (isDragging && dragStartX !== null) {
      currentDragX = x;
      updateBrushFromDrag(dragStartX, x);
    } else {
      // Hover detection
      const totalBarSpace = bucketWidth + BAR_GAP;
      const index = Math.floor(x / totalBarSpace);
      if (index >= 0 && index < buckets.length) {
        hoveredBucket = buckets[index];
        hoveredBucketIndex = index;
        tooltipX = Math.min(Math.max(x, 120), rect.width - 120);
        tooltipY = -8;
      } else {
        hoveredBucket = null;
        hoveredBucketIndex = null;
      }
    }
  }

  function handleMouseUp(e: MouseEvent) {
    if (isDragging && dragStartX !== null && currentDragX !== null) {
      const dist = Math.abs(currentDragX - dragStartX);
      if (dist < 4) {
        // Simple click on a bar
        const rect = containerEl?.getBoundingClientRect();
        if (rect) {
          const x = Math.max(0, Math.min(e.clientX - rect.left, rect.width));
          const totalBarSpace = bucketWidth + BAR_GAP;
          const index = Math.floor(x / totalBarSpace);
          if (index >= 0 && index < buckets.length && buckets[index].start_line > 0) {
            onSelectLine(buckets[index].start_line);
          }
        }
        activeBrush = null;
        onBrushChange?.(null);
      } else {
        updateBrushFromDrag(dragStartX, currentDragX);
      }
    }
    isDragging = false;
    dragStartX = null;
    currentDragX = null;
  }

  function handleMouseLeave() {
    if (isDragging) {
      isDragging = false;
      dragStartX = null;
      currentDragX = null;
    }
    hoveredBucket = null;
    hoveredBucketIndex = null;
  }

  function updateBrushFromDrag(x1: number, x2: number) {
    if (!buckets.length) return;
    const totalBarSpace = bucketWidth + BAR_GAP;
    const startX = Math.min(x1, x2);
    const endX = Math.max(x1, x2);

    const startIndex = Math.max(0, Math.min(Math.floor(startX / totalBarSpace), buckets.length - 1));
    const endIndex = Math.max(0, Math.min(Math.floor(endX / totalBarSpace), buckets.length - 1));

    let totalCount = 0;
    let errorCount = 0;
    let warnCount = 0;
    let startLine = Infinity;
    let endLine = 0;

    for (let i = startIndex; i <= endIndex; i++) {
      const b = buckets[i];
      totalCount += b.count;
      errorCount += b.error_count;
      warnCount += b.warn_count;
      if (b.start_line > 0 && b.start_line < startLine) startLine = b.start_line;
      if (b.end_line > endLine) endLine = b.end_line;
    }

    if (startLine === Infinity) startLine = 1;

    activeBrush = {
      startIndex,
      endIndex,
      startTime: buckets[startIndex].start_time,
      endTime: buckets[endIndex].end_time,
      startLine,
      endLine,
      totalCount,
      errorCount,
      warnCount
    };

    onBrushChange?.(activeBrush);
  }

  function clearBrush() {
    activeBrush = null;
    onBrushChange?.(null);
  }

  // Calculate pixel bounds of active brush
  let brushOverlay = $derived.by(() => {
    if (isDragging && dragStartX !== null && currentDragX !== null) {
      const left = Math.min(dragStartX, currentDragX);
      const width = Math.max(2, Math.abs(currentDragX - dragStartX));
      return { left, width };
    }
    if (activeBrush && buckets.length) {
      const totalBarSpace = bucketWidth + BAR_GAP;
      const left = activeBrush.startIndex * totalBarSpace;
      const right = (activeBrush.endIndex + 1) * totalBarSpace - BAR_GAP;
      return { left, width: Math.max(4, right - left) };
    }
    return null;
  });
</script>

<div class="relative w-full bg-[#0B0D13]/95 border-b border-white/[0.08] backdrop-blur-md px-4 py-2 font-sans select-none z-15">
  <!-- Top Bar: Title, Range, Legend, Close -->
  <div class="flex items-center justify-between text-xs mb-1.5 gap-2">
    <!-- Left: Title & Range -->
    <div class="flex items-center gap-2">
      <div class="flex items-center gap-1.5 text-[#F4F6FB] font-medium text-[11px]">
        <svg class="w-3.5 h-3.5 text-indigo-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
        </svg>
        <span>Timeline {isFilterActive ? "Filtrata" : "Log"}</span>
      </div>

      {#if hasData && histogram}
        <div class="flex items-center gap-1.5 px-2 py-0.5 rounded-md bg-white/[0.05] ring-1 ring-inset ring-white/[0.08] text-[10px] text-[#B4BCD0] font-mono">
          <span>{histogram.min_time_str}</span>
          <span class="text-white/40">→</span>
          <span>{histogram.max_time_str}</span>
        </div>
      {/if}

      {#if isLoading}
        <span class="text-indigo-400 text-[10px] animate-pulse">Aggiornamento...</span>
      {/if}
    </div>

    <!-- Center: Brush Selection Stats & Jump Actions -->
    {#if activeBrush}
      <div class="flex items-center gap-2 px-2.5 py-0.5 rounded-lg bg-indigo-950/60 ring-1 ring-inset ring-indigo-500/40 text-xs shadow-xs animate-in fade-in duration-150">
        <div class="flex items-center gap-1.5 text-[11px] font-mono text-indigo-200">
          <span class="font-bold text-white">{activeBrush.totalCount.toLocaleString()}</span> righe
          {#if activeBrush.errorCount > 0}
            <span class="text-rose-400 font-semibold">({activeBrush.errorCount.toLocaleString()} err)</span>
          {/if}
          {#if activeBrush.warnCount > 0}
            <span class="text-amber-400 font-semibold">({activeBrush.warnCount.toLocaleString()} warn)</span>
          {/if}
        </div>

        <button
          onclick={() => activeBrush && onSelectLine(activeBrush.startLine)}
          class="px-2 py-0.5 rounded bg-indigo-500/25 hover:bg-indigo-500/40 text-indigo-100 font-medium text-[10px] transition-colors cursor-pointer"
          title="Salta alla riga iniziale selezionata"
        >
          Salta a riga {activeBrush.startLine.toLocaleString()}
        </button>

        {#if onFilterTimeRange}
          <button
            onclick={() => activeBrush && onFilterTimeRange(activeBrush)}
            class="px-2 py-0.5 rounded bg-emerald-500/25 hover:bg-emerald-500/40 text-emerald-200 font-medium text-[10px] transition-colors cursor-pointer"
            title="Isola e filtra solo le righe in questo intervallo temporale"
          >
            Filtra intervallo
          </button>
        {/if}

        <button
          onclick={clearBrush}
          class="text-[#B4BCD0] hover:text-white p-0.5 rounded transition-colors cursor-pointer"
          title="Deseleziona intervallo (Esc)"
        >
          <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>
    {/if}

    <!-- Right: Legend & Close -->
    <div class="flex items-center gap-3">
      <div class="flex items-center gap-2 text-[10px] text-[#B4BCD0]">
        <div class="flex items-center gap-1">
          <span class="w-2 h-2 rounded-full bg-rose-500 shadow-[0_0_6px_rgba(244,63,94,0.6)]"></span>
          <span>Error</span>
        </div>
        <div class="flex items-center gap-1">
          <span class="w-2 h-2 rounded-full bg-amber-400 shadow-[0_0_6px_rgba(251,191,36,0.6)]"></span>
          <span>Warn</span>
        </div>
        <div class="flex items-center gap-1">
          <span class="w-2 h-2 rounded-full bg-indigo-400"></span>
          <span>Info/Altri</span>
        </div>
      </div>

      <button
        onclick={onClose}
        class="text-[#7E879B] hover:text-white p-1 rounded-md hover:bg-white/[0.06] transition-colors cursor-pointer"
        title="Chiudi Timeline (Alt + T)"
      >
        <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
        </svg>
      </button>
    </div>
  </div>

  <!-- Main Chart Canvas / SVG Container -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    bind:this={containerEl}
    bind:clientWidth={containerWidth}
    onmousedown={handleMouseDown}
    onmousemove={handleMouseMove}
    onmouseup={handleMouseUp}
    onmouseleave={handleMouseLeave}
    role="presentation"
    class="relative w-full h-[52px] bg-[#07090E] rounded-lg border border-white/[0.06] overflow-hidden cursor-crosshair select-none"
  >
    {#if !hasData}
      <div class="w-full h-full flex items-center justify-center text-xs text-[#7E879B]">
        {#if isLoading}
          <span>Calcolo istogramma temporale in corso...</span>
        {:else}
          <span>Nessun timestamp riconosciuto nei log per generare la timeline</span>
        {/if}
      </div>
    {:else}
      <!-- Stacked SVG Bars -->
      <svg class="w-full h-full block overflow-visible" preserveAspectRatio="none">
        {#each buckets as bucket, i}
          {@const coords = getBarCoordinates(i)}
          {@const total = bucket.count}
          {@const hasItems = total > 0}
          <!-- Square-root height scaling for dynamic range -->
          {@const normalizedH = hasItems ? Math.max(3, (Math.sqrt(total) / Math.sqrt(maxBucketCount)) * CHART_HEIGHT) : 0}
          {@const yStart = CHART_HEIGHT - normalizedH}

          <!-- Stacked components -->
          {@const errRatio = hasItems ? bucket.error_count / total : 0}
          {@const warnRatio = hasItems ? bucket.warn_count / total : 0}
          {@const infoRatio = hasItems ? bucket.info_count / total : 0}

          {@const errH = normalizedH * errRatio}
          {@const warnH = normalizedH * warnRatio}
          {@const infoH = normalizedH * infoRatio}

          {#if hasItems}
            <g class="transition-opacity duration-100 {hoveredBucketIndex === i ? 'opacity-100' : 'opacity-85'}">
              <!-- Info / Other layer (bottom of the bar) -->
              {#if infoH > 0}
                <rect
                  x={coords.x}
                  y={yStart + errH + warnH}
                  width={coords.width}
                  height={infoH}
                  fill="#6366F1"
                  class="fill-indigo-500/80"
                  rx="0.5"
                />
              {/if}

              <!-- Warning layer (middle) -->
              {#if warnH > 0}
                <rect
                  x={coords.x}
                  y={yStart + errH}
                  width={coords.width}
                  height={warnH}
                  fill="#F59E0B"
                  class="fill-amber-400"
                  rx="0.5"
                />
              {/if}

              <!-- Error layer (top of the bar) -->
              {#if errH > 0}
                <rect
                  x={coords.x}
                  y={yStart}
                  width={coords.width}
                  height={errH}
                  fill="#F43F5E"
                  class="fill-rose-500"
                  rx="0.5"
                />
              {/if}
            </g>
          {:else}
            <!-- Empty bucket placeholder dot -->
            <rect
              x={coords.x}
              y={CHART_HEIGHT - 1.5}
              width={coords.width}
              height="1.5"
              fill="rgba(255,255,255,0.06)"
            />
          {/if}
        {/each}
      </svg>

      <!-- Brushing Selection Overlay -->
      {#if brushOverlay}
        <div
          class="absolute top-0 bottom-0 bg-indigo-500/20 border-x-2 border-indigo-400 pointer-events-none transition-[width] duration-75 shadow-[0_0_12px_rgba(99,102,241,0.35)]"
          style="left: {brushOverlay.left}px; width: {brushOverlay.width}px;"
        >
          <div class="absolute top-0 left-0 bg-indigo-400 text-black text-[9px] font-mono px-1 py-0.2 rounded-br font-bold">
            START
          </div>
          <div class="absolute bottom-0 right-0 bg-indigo-400 text-black text-[9px] font-mono px-1 py-0.2 rounded-tl font-bold">
            END
          </div>
        </div>
      {/if}

      <!-- Interactive Hover Tooltip -->
      {#if hoveredBucket && !isDragging}
        <div
          class="absolute top-1 pointer-events-none z-30 transform -translate-x-1/2 -translate-y-full mb-1.5 px-2.5 py-1.5 rounded-lg bg-[#0F131D]/95 border border-white/[0.15] shadow-xl text-[11px] font-sans text-white backdrop-blur-md min-w-[150px]"
          style="left: {tooltipX}px; top: {tooltipY}px;"
        >
          <div class="flex items-center justify-between gap-3 text-[10px] text-[#B4BCD0] font-mono border-b border-white/[0.08] pb-1 mb-1">
            <span class="font-bold text-indigo-300">{hoveredBucket.label}</span>
            <span>{hoveredBucket.count.toLocaleString()} righe</span>
          </div>

          <div class="flex flex-col gap-0.5 text-[10px]">
            {#if hoveredBucket.error_count > 0}
              <div class="flex items-center justify-between text-rose-300">
                <span>Errori:</span>
                <span class="font-bold">{hoveredBucket.error_count.toLocaleString()}</span>
              </div>
            {/if}
            {#if hoveredBucket.warn_count > 0}
              <div class="flex items-center justify-between text-amber-300">
                <span>Warning:</span>
                <span class="font-bold">{hoveredBucket.warn_count.toLocaleString()}</span>
              </div>
            {/if}
            <div class="flex items-center justify-between text-indigo-300">
              <span>Info/Altri:</span>
              <span class="font-bold">{hoveredBucket.info_count.toLocaleString()}</span>
            </div>

            {#if hoveredBucket.start_line > 0}
              <div class="mt-1 pt-1 border-t border-white/[0.06] text-[9px] text-[#7E879B] flex items-center justify-between">
                <span>Riga iniziale:</span>
                <span class="text-cyan-300 font-mono">#{hoveredBucket.start_line.toLocaleString()}</span>
              </div>
            {/if}
          </div>
        </div>
      {/if}
    {/if}
  </div>
</div>
