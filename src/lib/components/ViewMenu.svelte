<script lang="ts">
  import { onMount } from "svelte";
  import { fade, fly } from "svelte/transition";
  import type { FoldingStatus } from "$lib/types";

  interface Props {
    wrapLines: boolean;
    onToggleWrap: () => void;
    isTimelineOpen: boolean;
    onToggleTimeline: () => void;
    foldingStatus: FoldingStatus;
    onToggleFolding: () => void;
    onOpenLogLevels: () => void;
  }

  let {
    wrapLines,
    onToggleWrap,
    isTimelineOpen,
    onToggleFolding,
    foldingStatus,
    onToggleTimeline,
    onOpenLogLevels
  }: Props = $props();

  let isOpen = $state(false);
  let menuContainer = $state<HTMLDivElement | null>(null);

  // Badge active indicators count
  let activeIndicatorsCount = $derived(
    (wrapLines ? 1 : 0) +
    (isTimelineOpen ? 1 : 0) +
    (foldingStatus.total_groups > 0 && foldingStatus.auto_collapse_enabled ? 1 : 0)
  );

  function toggleMenu() {
    isOpen = !isOpen;
  }

  function closeMenu() {
    isOpen = false;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && isOpen) {
      isOpen = false;
    }
  }

  onMount(() => {
    function handleClickOutside(event: MouseEvent) {
      if (menuContainer && !menuContainer.contains(event.target as Node)) {
        isOpen = false;
      }
    }
    window.addEventListener("mousedown", handleClickOutside);
    window.addEventListener("keydown", handleKeydown);
    return () => {
      window.removeEventListener("mousedown", handleClickOutside);
      window.removeEventListener("keydown", handleKeydown);
    };
  });
</script>

<div class="relative inline-block text-left" bind:this={menuContainer}>
  <!-- Trigger Button -->
  <button
    type="button"
    onclick={toggleMenu}
    class="tactile-button flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-xs font-medium cursor-pointer transition-all duration-150 {isOpen ? 'bg-white/[0.1] text-white ring-1 ring-white/[0.2]' : 'text-[#B4BCD0] hover:text-white'}"
    title="Opzioni visualizzazione (A capo, Timeline, Stacktrace...)"
    aria-expanded={isOpen}
    aria-haspopup="true"
  >
    <svg class="w-3.5 h-3.5 text-indigo-400 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6V4m0 2a2 2 0 100 4m0-4a2 2 0 110 4m-6 8a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4m6 6v10m6-2a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4" />
    </svg>
    <span class="hidden sm:inline">Vista</span>
    {#if activeIndicatorsCount > 0}
      <span class="w-1.5 h-1.5 rounded-full bg-indigo-400 shadow-[0_0_6px_rgba(129,140,248,0.8)]"></span>
    {/if}
    <svg class="w-3 h-3 text-[#7E879B] transition-transform duration-150 {isOpen ? 'rotate-180' : ''}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
    </svg>
  </button>

  <!-- Dropdown Popover -->
  {#if isOpen}
    <div
      transition:fly={{ y: 6, duration: 150 }}
      class="absolute right-0 mt-1.5 w-64 rounded-xl bg-[#12151E]/95 backdrop-blur-xl border border-white/[0.12] shadow-2xl p-1.5 z-50 text-xs font-sans text-[#F4F6FB] focus:outline-none ring-1 ring-black/40"
      role="menu"
      tabindex="-1"
    >
      <div class="px-2.5 py-1.5 text-[10.5px] font-semibold text-[#7E879B] uppercase tracking-wider">
        Visualizzazione
      </div>

      <!-- Item: Word Wrap -->
      <button
        onclick={() => { onToggleWrap(); }}
        class="w-full flex items-center justify-between px-2.5 py-2 rounded-lg hover:bg-white/[0.08] transition text-left cursor-pointer group"
        role="menuitem"
      >
        <div class="flex items-center gap-2">
          <svg class="w-4 h-4 text-[#7E879B] group-hover:text-indigo-300 transition" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 10h10a4 4 0 014 4v0a4 4 0 01-4 4H9m4 0l-3-3m3 3l-3 3M3 6h18M3 14h4" />
          </svg>
          <div class="flex flex-col">
            <span class="font-medium text-[12px] {wrapLines ? 'text-indigo-200' : 'text-[#F4F6FB]'}">A capo automatico</span>
            <span class="text-[10px] text-[#7E879B]">Wrap su linee lunghe</span>
          </div>
        </div>
        <div class="flex items-center gap-2">
          <kbd class="tactile-kbd text-[9.5px] text-[#7E879B]">Alt+Z</kbd>
          <div class="w-7 h-4 rounded-full transition-colors p-0.5 flex items-center {wrapLines ? 'bg-indigo-500 justify-end' : 'bg-white/10 justify-start'}">
            <div class="w-3 h-3 rounded-full bg-white shadow-xs"></div>
          </div>
        </div>
      </button>

      <!-- Item: Timeline Histogram -->
      <button
        onclick={() => { onToggleTimeline(); }}
        class="w-full flex items-center justify-between px-2.5 py-2 rounded-lg hover:bg-white/[0.08] transition text-left cursor-pointer group"
        role="menuitem"
      >
        <div class="flex items-center gap-2">
          <svg class="w-4 h-4 text-[#7E879B] group-hover:text-indigo-300 transition" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
          </svg>
          <div class="flex flex-col">
            <span class="font-medium text-[12px] {isTimelineOpen ? 'text-indigo-200' : 'text-[#F4F6FB]'}">Timeline eventi</span>
            <span class="text-[10px] text-[#7E879B]">Grafico temporale e zoom</span>
          </div>
        </div>
        <div class="flex items-center gap-2">
          <kbd class="tactile-kbd text-[9.5px] text-[#7E879B]">Alt+T</kbd>
          <div class="w-7 h-4 rounded-full transition-colors p-0.5 flex items-center {isTimelineOpen ? 'bg-indigo-500 justify-end' : 'bg-white/10 justify-start'}">
            <div class="w-3 h-3 rounded-full bg-white shadow-xs"></div>
          </div>
        </div>
      </button>

      <!-- Item: Stacktrace Folding -->
      <button
        onclick={() => { onToggleFolding(); }}
        disabled={foldingStatus.total_groups === 0}
        class="w-full flex items-center justify-between px-2.5 py-2 rounded-lg hover:bg-white/[0.08] transition text-left cursor-pointer group disabled:opacity-40 disabled:cursor-not-allowed"
        role="menuitem"
      >
        <div class="flex items-center gap-2">
          <svg class="w-4 h-4 text-[#7E879B] group-hover:text-purple-300 transition" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
          </svg>
          <div class="flex flex-col">
            <span class="font-medium text-[12px] {foldingStatus.auto_collapse_enabled && foldingStatus.total_groups > 0 ? 'text-purple-200' : 'text-[#F4F6FB]'}">
              Comprimi stacktrace
            </span>
            <span class="text-[10px] text-[#7E879B]">
              {foldingStatus.total_groups > 0 ? `${foldingStatus.total_groups} blocchi individuati` : 'Nessun blocco'}
            </span>
          </div>
        </div>
        <div class="flex items-center gap-2">
          <kbd class="tactile-kbd text-[9.5px] text-[#7E879B]">Alt+S</kbd>
          <div class="w-7 h-4 rounded-full transition-colors p-0.5 flex items-center {foldingStatus.auto_collapse_enabled && foldingStatus.total_groups > 0 ? 'bg-purple-500 justify-end' : 'bg-white/10 justify-start'}">
            <div class="w-3 h-3 rounded-full bg-white shadow-xs"></div>
          </div>
        </div>
      </button>

      <div class="h-px bg-white/[0.08] my-1 mx-1.5"></div>

      <!-- Item: Livelli di log -->
      <button
        onclick={() => { closeMenu(); onOpenLogLevels(); }}
        class="w-full flex items-center justify-between px-2.5 py-2 rounded-lg hover:bg-white/[0.08] transition text-left cursor-pointer group"
        role="menuitem"
      >
        <div class="flex items-center gap-2">
          <svg class="w-4 h-4 text-amber-400 group-hover:scale-110 transition-transform" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
          </svg>
          <div class="flex flex-col">
            <span class="font-medium text-[12px] text-[#F4F6FB]">Regole Livelli Log</span>
            <span class="text-[10px] text-[#7E879B]">Error, Warn, Info, Debug</span>
          </div>
        </div>
        <svg class="w-3.5 h-3.5 text-[#7E879B]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
        </svg>
      </button>
    </div>
  {/if}
</div>
