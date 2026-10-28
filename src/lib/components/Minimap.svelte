<script lang="ts">
  import { onMount } from "svelte";
  import type { SeverityHeatmap, HeatmapBin } from "$lib/types";

  interface Props {
    totalLines: number;
    visibleStartIndex: number;
    visibleCount: number;
    matchLines?: number[];
    bookmarks?: number[];
    heatmap?: SeverityHeatmap | null;
    onSeek: (targetLine: number) => void;
  }

  let {
    totalLines = 0,
    visibleStartIndex = 0,
    visibleCount = 50,
    matchLines = [],
    bookmarks = [],
    heatmap = null,
    onSeek
  }: Props = $props();

  let canvasEl = $state<HTMLCanvasElement | null>(null);
  let containerEl = $state<HTMLDivElement | null>(null);
  let isDragging = $state(false);

  let height = $state(600);
  const width = 22;

  // Hover Tooltip State
  let hoverY = $state<number | null>(null);
  let hoverLine = $state<number | null>(null);
  let hoverBin = $state<HeatmapBin | null>(null);

  // Viewport thumb math
  let thumbTop = $derived.by(() => {
    if (totalLines <= 0 || height <= 0) return 0;
    const ratio = Math.min(1, visibleStartIndex / totalLines);
    return ratio * height;
  });

  let thumbHeight = $derived.by(() => {
    if (totalLines <= 0 || height <= 0) return 20;
    const ratio = Math.min(1, visibleCount / totalLines);
    return Math.max(14, ratio * height);
  });

  function drawMinimap() {
    if (!canvasEl || height <= 0 || totalLines <= 0) return;
    const ctx = canvasEl.getContext("2d");
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    canvasEl.width = width * dpr;
    canvasEl.height = height * dpr;
    ctx.scale(dpr, dpr);

    ctx.clearRect(0, 0, width, height);

    // 1. Render Error & Warning Heatmap Bins (Layer 0)
    if (heatmap && heatmap.bins && heatmap.bins.length > 0) {
      const bins = heatmap.bins;
      const maxErr = Math.max(1, heatmap.max_errors_in_bin || 1);
      const maxWarn = Math.max(1, heatmap.max_warnings_in_bin || 1);
      const binH = Math.max(2, height / bins.length);

      for (let i = 0; i < bins.length; i++) {
        const b = bins[i];
        if (b.error_count === 0 && b.warn_count === 0) continue;

        const y = (i / bins.length) * height;

        // Warnings: amber glow & horizontal bar
        if (b.warn_count > 0) {
          const intensity = Math.min(1.0, 0.35 + 0.65 * Math.sqrt(b.warn_count / maxWarn));
          ctx.fillStyle = `rgba(245, 158, 11, ${intensity.toFixed(2)})`;
          ctx.fillRect(0, y, width - 4, binH);
        }

        // Errors: crimson/rose glow & horizontal bar
        if (b.error_count > 0) {
          const intensity = Math.min(1.0, 0.45 + 0.55 * Math.sqrt(b.error_count / maxErr));
          ctx.fillStyle = `rgba(244, 63, 94, ${intensity.toFixed(2)})`;
          ctx.fillRect(0, y, width - 2, binH);

          // Hot cluster indicator: bright pip on the edge
          if (b.error_count >= 3) {
            ctx.fillStyle = "#FDA4AF"; // Bright rose glow pip
            ctx.fillRect(0, y, 2.5, binH);
          }
        }
      }
    }

    // 2. Render Search Match Markers (Layer 1 - Indigo)
    if (matchLines && matchLines.length > 0) {
      ctx.fillStyle = "#818CF8"; // Indigo brillante for matches
      const tickH = 1.5;

      let lastY = -1;
      for (let i = 0; i < matchLines.length; i++) {
        const lineNum = matchLines[i];
        const y = Math.floor((lineNum / totalLines) * height);
        if (y === lastY) continue;
        lastY = y;
        ctx.fillRect(2, y, width - 4, tickH);
      }
    }

    // 3. Render Bookmark Markers (Layer 2 - Cyan)
    if (bookmarks && bookmarks.length > 0) {
      ctx.fillStyle = "#22D3EE"; // Vibrant Cyan for bookmarks
      const tickH = 2.5;
      for (let i = 0; i < bookmarks.length; i++) {
        const lineNum = bookmarks[i];
        const y = Math.floor((lineNum / totalLines) * height);
        ctx.fillRect(0, Math.max(0, y - 1), width, tickH);
      }
    }
  }

  $effect(() => {
    // Redraw whenever matches, bookmarks, heatmap, totalLines, or dimensions change
    if (canvasEl && height > 0) {
      drawMinimap();
    }
  });

  function getLineAtClientY(clientY: number): number {
    if (!containerEl || totalLines <= 0) return 1;
    const rect = containerEl.getBoundingClientRect();
    const relativeY = Math.max(0, Math.min(rect.height, clientY - rect.top));
    const targetLine = Math.round((relativeY / rect.height) * totalLines);
    return Math.max(1, Math.min(totalLines, targetLine));
  }

  function handleSeekAtClientY(clientY: number) {
    const line = getLineAtClientY(clientY);
    onSeek(line);
  }

  function handleMouseMove(e: MouseEvent) {
    if (!containerEl || totalLines <= 0) return;
    const rect = containerEl.getBoundingClientRect();
    const relY = Math.max(0, Math.min(rect.height, e.clientY - rect.top));
    hoverY = relY;
    hoverLine = Math.max(1, Math.min(totalLines, Math.round((relY / rect.height) * totalLines)));

    // Look up corresponding heatmap bin
    if (heatmap && heatmap.bins && heatmap.bins.length > 0) {
      const binIdx = Math.min(
        heatmap.bins.length - 1,
        Math.floor((relY / rect.height) * heatmap.bins.length)
      );
      hoverBin = heatmap.bins[binIdx] || null;
    } else {
      hoverBin = null;
    }
  }

  function handleMouseLeave() {
    hoverY = null;
    hoverLine = null;
    hoverBin = null;
  }

  function handleMouseDown(e: MouseEvent) {
    e.preventDefault();
    isDragging = true;
    handleSeekAtClientY(e.clientY);

    function onMouseMove(moveEvent: MouseEvent) {
      handleSeekAtClientY(moveEvent.clientY);
      handleMouseMove(moveEvent);
    }

    function onMouseUp() {
      isDragging = false;
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    }

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
  }

  function jumpToFirstError() {
    if (!heatmap || !heatmap.bins) return;
    const firstErrBin = heatmap.bins.find((b) => b.error_count > 0);
    if (firstErrBin) {
      onSeek(firstErrBin.start_line);
    }
  }

  onMount(() => {
    const updateSize = () => {
      if (containerEl) {
        height = containerEl.clientHeight;
      }
    };

    updateSize();
    const ro = new ResizeObserver(() => updateSize());
    if (containerEl) {
      ro.observe(containerEl);
    }

    return () => {
      ro.disconnect();
    };
  });
</script>

<div
  bind:this={containerEl}
  onmousedown={handleMouseDown}
  onmousemove={handleMouseMove}
  onmouseleave={handleMouseLeave}
  role="slider"
  tabindex="0"
  aria-valuemin="1"
  aria-valuemax={totalLines}
  aria-valuenow={visibleStartIndex + 1}
  aria-label="Minimap Scrollbar"
  class="relative h-full select-none cursor-pointer border-l border-white/[0.06] bg-[#090A0F]/95 hover:bg-[#0E1017] transition-colors shrink-0"
  style="width: {width}px;"
>
  <!-- Background canvas for Heatmap + SIMD match ticks + Bookmarks -->
  <canvas
    bind:this={canvasEl}
    class="absolute left-0 top-0 pointer-events-none"
    style="width: {width}px; height: {height}px;"
  ></canvas>

  <!-- Viewport Thumb Overlay -->
  {#if totalLines > 0}
    <div
      class="absolute left-0 w-full rounded-xs transition-colors pointer-events-none {isDragging ? 'bg-white/30 border border-white/50 shadow-sm' : 'bg-white/15 border border-white/20 hover:bg-white/25'}"
      style="top: {thumbTop}px; height: {thumbHeight}px;"
    ></div>
  {/if}

  <!-- Floating Hover Tooltip (appears to the left of the minimap track) -->
  {#if hoverY !== null && hoverLine !== null && !isDragging}
    <div
      class="absolute right-full mr-2 pointer-events-none z-30 transform -translate-y-1/2 px-2.5 py-1.5 rounded-lg bg-[#0F131D]/95 border border-white/[0.15] shadow-2xl text-[10px] font-sans text-white backdrop-blur-md whitespace-nowrap min-w-[100px]"
      style="top: {hoverY}px;"
    >
      <div class="flex items-center justify-between gap-2 border-b border-white/[0.08] pb-0.5 mb-1 font-mono">
        <span class="text-cyan-300 font-bold">Riga #{hoverLine.toLocaleString()}</span>
        <span class="text-[#7E879B] text-[9px]">{Math.round((hoverLine / totalLines) * 100)}%</span>
      </div>

      {#if hoverBin && (hoverBin.error_count > 0 || hoverBin.warn_count > 0)}
        <div class="flex flex-col gap-0.5 text-[9px]">
          {#if hoverBin.error_count > 0}
            <div class="flex items-center justify-between text-rose-300 font-bold">
              <span>🔴 Errori:</span>
              <span>{hoverBin.error_count.toLocaleString()}</span>
            </div>
          {/if}
          {#if hoverBin.warn_count > 0}
            <div class="flex items-center justify-between text-amber-300 font-bold">
              <span>🟡 Warning:</span>
              <span>{hoverBin.warn_count.toLocaleString()}</span>
            </div>
          {/if}
        </div>
      {:else}
        <div class="text-[#7E879B] text-[9px]">Nessuna anomalia</div>
      {/if}
    </div>
  {/if}
</div>
