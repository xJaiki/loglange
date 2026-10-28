<script lang="ts">
  import { onMount } from "svelte";

  interface Props {
    totalLines: number;
    visibleStartIndex: number;
    visibleCount: number;
    matchLines?: number[];
    onSeek: (targetLine: number) => void;
  }

  let {
    totalLines = 0,
    visibleStartIndex = 0,
    visibleCount = 50,
    matchLines = [],
    onSeek
  }: Props = $props();

  let canvasEl = $state<HTMLCanvasElement | null>(null);
  let containerEl = $state<HTMLDivElement | null>(null);
  let isDragging = $state(false);

  let height = $state(600);
  const width = 18;

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

    // Render search match markers
    if (matchLines && matchLines.length > 0) {
      ctx.fillStyle = "#818CF8"; // Indigo brillante for matches
      const tickH = 1.5;

      let lastY = -1;
      for (let i = 0; i < matchLines.length; i++) {
        const lineNum = matchLines[i];
        const y = Math.floor((lineNum / totalLines) * height);
        if (y === lastY) continue;
        lastY = y;
        ctx.fillRect(1, y, width - 2, tickH);
      }
    }
  }

  $effect(() => {
    // Redraw whenever matches, totalLines, or dimensions change
    if (canvasEl && height > 0) {
      drawMinimap();
    }
  });

  function handleSeekAtClientY(clientY: number) {
    if (!containerEl || totalLines <= 0) return;
    const rect = containerEl.getBoundingClientRect();
    const relativeY = Math.max(0, Math.min(rect.height, clientY - rect.top));
    const targetLine = Math.round((relativeY / rect.height) * totalLines);
    onSeek(Math.max(1, Math.min(totalLines, targetLine)));
  }

  function handleMouseDown(e: MouseEvent) {
    e.preventDefault();
    isDragging = true;
    handleSeekAtClientY(e.clientY);

    function onMouseMove(moveEvent: MouseEvent) {
      handleSeekAtClientY(moveEvent.clientY);
    }

    function onMouseUp() {
      isDragging = false;
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    }

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
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
  role="slider"
  tabindex="0"
  aria-valuemin="1"
  aria-valuemax={totalLines}
  aria-valuenow={visibleStartIndex + 1}
  aria-label="Minimap Scrollbar"
  class="relative h-full select-none cursor-pointer border-l border-white/[0.06] bg-[#090A0F]/90 hover:bg-[#11131A] transition-colors shrink-0"
  style="width: {width}px;"
>
  <!-- Background canvas for SIMD match ticks -->
  <canvas
    bind:this={canvasEl}
    class="absolute left-0 top-0 pointer-events-none"
    style="width: {width}px; height: {height}px;"
  ></canvas>

  <!-- Viewport Thumb Overlay -->
  {#if totalLines > 0}
    <div
      class="absolute left-0 w-full rounded-sm transition-colors pointer-events-none {isDragging ? 'bg-white/25 border border-white/40' : 'bg-white/15 border border-white/20 hover:bg-white/25'}"
      style="top: {thumbTop}px; height: {thumbHeight}px;"
    ></div>
  {/if}
</div>
