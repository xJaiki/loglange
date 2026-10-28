<script lang="ts">
  import { onMount } from "svelte";
  import { fly } from "svelte/transition";
  import type { SearchDisplayMode } from "$lib/types";

  export type QuickFilterPreset = "all" | "errors" | "warnings" | "json";

  interface Props {
    query: string;
    isRegex: boolean;
    caseSensitive: boolean;
    invert?: boolean;
    mode: SearchDisplayMode;
    totalMatches: number;
    totalLines: number;
    searchTimeMs: number;
    currentMatchIndex?: number;
    error?: string | null;
    isLoading?: boolean;
    quickPreset?: QuickFilterPreset | null;
    canExport?: boolean;
    onSearchChange: (query: string) => void;
    onToggleRegex: () => void;
    onToggleCase: () => void;
    onToggleInvert?: () => void;
    onModeChange: (mode: SearchDisplayMode) => void;
    onNextMatch: () => void;
    onPrevMatch: () => void;
    onClear: () => void;
    onApplyQuickPreset?: (preset: QuickFilterPreset) => void;
    onExport?: () => void;
    onCopy?: () => void;
  }

  let {
    query = "",
    isRegex = false,
    caseSensitive = false,
    invert = false,
    mode = "filter",
    totalMatches = 0,
    totalLines = 0,
    searchTimeMs = 0,
    currentMatchIndex = 0,
    error = null,
    isLoading = false,
    quickPreset = "all",
    canExport = false,
    onSearchChange,
    onToggleRegex,
    onToggleCase,
    onToggleInvert,
    onModeChange,
    onNextMatch,
    onPrevMatch,
    onClear,
    onApplyQuickPreset,
    onExport,
    onCopy
  }: Props = $props();

  let inputEl = $state<HTMLInputElement | null>(null);
  let isFocused = $state(false);
  let isPresetDropdownOpen = $state(false);
  let presetMenuContainer = $state<HTMLDivElement | null>(null);

  export function focusInput() {
    if (inputEl) {
      inputEl.focus();
      inputEl.select();
    }
  }

  function handleInput(e: Event) {
    const val = (e.target as HTMLInputElement).value;
    onSearchChange(val);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      if (e.shiftKey) {
        onPrevMatch();
      } else {
        onNextMatch();
      }
    } else if (e.key === "Escape") {
      e.preventDefault();
      if (isPresetDropdownOpen) {
        isPresetDropdownOpen = false;
        return;
      }
      if (query.length > 0) {
        onClear();
      } else {
        inputEl?.blur();
      }
    } else if (e.altKey && e.key.toLowerCase() === "r") {
      e.preventDefault();
      onToggleRegex();
    } else if (e.altKey && e.key.toLowerCase() === "c") {
      e.preventDefault();
      onToggleCase();
    } else if (e.altKey && e.key.toLowerCase() === "i") {
      e.preventDefault();
      onToggleInvert?.();
    }
  }

  onMount(() => {
    function handleClickOutside(event: MouseEvent) {
      if (presetMenuContainer && !presetMenuContainer.contains(event.target as Node)) {
        isPresetDropdownOpen = false;
      }
    }
    window.addEventListener("mousedown", handleClickOutside);
    return () => {
      window.removeEventListener("mousedown", handleClickOutside);
    };
  });
</script>

<div
  class="bg-[#0B0D13]/95 rounded-xl px-2.5 py-1.5 flex items-center gap-2 transition-all duration-200 font-sans w-full max-w-2xl {invert ? 'ring-1 ring-rose-500/50 shadow-[inset_0_1px_0_rgba(255,255,255,0.1),0_0_0_1px_rgba(244,63,94,0.3),0_4px_12px_rgba(0,0,0,0.5)]' : isFocused ? 'ring-2 ring-indigo-500/45 shadow-[inset_0_1px_0_rgba(255,255,255,0.12),0_0_0_1px_rgba(129,140,248,0.5),0_8px_24px_-4px_rgba(99,102,241,0.22)]' : 'shadow-[inset_0_1px_0_rgba(255,255,255,0.08),0_0_0_1px_rgba(255,255,255,0.1),0_2px_6px_rgba(0,0,0,0.4)] hover:shadow-[inset_0_1px_0_rgba(255,255,255,0.12),0_0_0_1px_rgba(255,255,255,0.15),0_4px_10px_rgba(0,0,0,0.45)]'}"
  role="search"
>
  <!-- Integrated Preset Filter Pill / Dropdown -->
  <div class="relative shrink-0" bind:this={presetMenuContainer}>
    {#if quickPreset && quickPreset !== 'all'}
      <!-- Active preset pill with dismiss button -->
      <div class="flex items-center gap-1.5 px-2 py-0.5 rounded-lg text-[11px] font-medium {quickPreset === 'errors' ? 'bg-rose-500/25 text-rose-200 ring-1 ring-rose-400/40' : quickPreset === 'warnings' ? 'bg-amber-500/25 text-amber-200 ring-1 ring-amber-400/40' : 'bg-emerald-500/25 text-emerald-200 ring-1 ring-emerald-400/40'}">
        <span class="w-1.5 h-1.5 rounded-full {quickPreset === 'errors' ? 'bg-rose-400' : quickPreset === 'warnings' ? 'bg-amber-400' : 'bg-emerald-400'}"></span>
        <span class="capitalize">{quickPreset}</span>
        <button
          onclick={() => onApplyQuickPreset?.('all')}
          class="hover:opacity-75 p-0.5 rounded cursor-pointer transition"
          title="Rimuovi filtro rapido"
        >
          <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>
    {:else}
      <!-- Compact Preset Selector Trigger -->
      <button
        type="button"
        onclick={() => (isPresetDropdownOpen = !isPresetDropdownOpen)}
        class="flex items-center gap-1 px-1.5 py-0.5 rounded-lg text-[11px] font-medium text-[#7E879B] hover:text-white hover:bg-white/[0.06] transition cursor-pointer"
        title="Filtri rapidi per livello log"
      >
        <svg class="w-3.5 h-3.5 text-[#7E879B]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 4a1 1 0 011-1h16a1 1 0 011 1v2.586a1 1 0 01-.293.707l-6.414 6.414a1 1 0 00-.293.707V17l-4 4v-6.586a1 1 0 00-.293-.707L3.293 7.293A1 1 0 013 6.586V4z" />
        </svg>
        <svg class="w-2.5 h-2.5 text-[#7E879B] transition-transform {isPresetDropdownOpen ? 'rotate-180' : ''}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M19 9l-7 7-7-7" />
        </svg>
      </button>

      {#if isPresetDropdownOpen}
        <div
          transition:fly={{ y: 4, duration: 120 }}
          class="absolute left-0 mt-1.5 w-40 rounded-xl bg-[#12151E]/95 backdrop-blur-xl border border-white/[0.12] shadow-2xl p-1 z-50 text-xs font-sans ring-1 ring-black/40"
        >
          <button
            onclick={() => { isPresetDropdownOpen = false; onApplyQuickPreset?.('all'); }}
            class="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg text-left text-[11px] font-medium hover:bg-white/[0.08] text-[#F4F6FB] transition cursor-pointer"
          >
            <span class="w-1.5 h-1.5 rounded-full bg-slate-400"></span>
            <span>Tutti i log</span>
          </button>
          <button
            onclick={() => { isPresetDropdownOpen = false; onApplyQuickPreset?.('errors'); }}
            class="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg text-left text-[11px] font-medium hover:bg-rose-500/15 text-rose-300 transition cursor-pointer"
          >
            <span class="w-1.5 h-1.5 rounded-full bg-rose-400"></span>
            <span>Solo Errori</span>
          </button>
          <button
            onclick={() => { isPresetDropdownOpen = false; onApplyQuickPreset?.('warnings'); }}
            class="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg text-left text-[11px] font-medium hover:bg-amber-500/15 text-amber-300 transition cursor-pointer"
          >
            <span class="w-1.5 h-1.5 rounded-full bg-amber-400"></span>
            <span>Solo Avvisi</span>
          </button>
          <button
            onclick={() => { isPresetDropdownOpen = false; onApplyQuickPreset?.('json'); }}
            class="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg text-left text-[11px] font-medium hover:bg-emerald-500/15 text-emerald-300 transition cursor-pointer"
          >
            <span class="font-mono text-[10px] text-emerald-400 font-bold">&#123;&#125;</span>
            <span>Strutture JSON</span>
          </button>
        </div>
      {/if}
    {/if}
  </div>

  <!-- Search / Exclude Icon -->
  <div class="shrink-0 transition-colors duration-150 {invert ? 'text-rose-400' : isFocused ? 'text-indigo-400' : 'text-[#7E879B]'}">
    {#if invert}
      <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M18.364 18.364A9 9 0 005.636 5.636m12.728 12.728A9 9 0 015.636 5.636m12.728 12.728L5.636 5.636" />
      </svg>
    {:else}
      <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
      </svg>
    {/if}
  </div>

  <!-- Search Input -->
  <div class="relative flex-1 min-w-[120px] flex items-center">
    <input
      bind:this={inputEl}
      type="text"
      value={query}
      oninput={handleInput}
      onkeydown={handleKeydown}
      onfocus={() => (isFocused = true)}
      onblur={() => (isFocused = false)}
      placeholder={invert ? "Escludi pattern dai log... (Alt+I)" : "Filtra o cerca nei log... (/ o ⌘F)"}
      class="w-full bg-transparent text-[#F4F6FB] placeholder-[#7E879B] text-[12px] focus:outline-none pr-6 {isRegex ? 'font-mono text-indigo-200' : 'font-sans'}"
      spellcheck="false"
      autocomplete="off"
    />
    {#if query.length > 0}
      <button
        onclick={onClear}
        class="absolute right-0 p-0.5 rounded-md text-[#7E879B] hover:text-white hover:bg-white/[0.08] transition-all duration-150 active:scale-90 cursor-pointer"
        title="Pulisci ricerca (Esc)"
      >
        <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
        </svg>
      </button>
    {/if}
  </div>

  <!-- Toggles: Invert, Regex, Case -->
  <div class="flex items-center gap-1 border-l border-white/[0.09] pl-2 shrink-0">
    <button
      onclick={onToggleInvert}
      class="px-1.5 py-0.5 rounded-md text-[11px] font-mono font-bold transition-all duration-150 active:scale-95 cursor-pointer {invert ? 'bg-rose-500/25 text-rose-200 ring-1 ring-inset ring-rose-400/50 shadow-[0_0_10px_rgba(244,63,94,0.3)]' : 'bg-white/[0.04] text-[#B4BCD0] ring-1 ring-inset ring-white/[0.08] hover:text-white hover:bg-white/[0.08]'}"
      title="Inverti / Escludi corrispondenze (Alt+I)"
    >
      !
    </button>
    <button
      onclick={onToggleRegex}
      class="px-1.5 py-0.5 rounded-md text-[11px] font-mono font-semibold transition-all duration-150 active:scale-95 cursor-pointer {isRegex ? 'bg-indigo-500/25 text-indigo-200 ring-1 ring-inset ring-indigo-400/50 shadow-[0_0_10px_rgba(99,102,241,0.25)]' : 'bg-white/[0.04] text-[#B4BCD0] ring-1 ring-inset ring-white/[0.08] hover:text-white hover:bg-white/[0.08]'}"
      title="Regex (Alt+R)"
    >
      .*
    </button>
    <button
      onclick={onToggleCase}
      class="px-1.5 py-0.5 rounded-md text-[11px] font-sans font-semibold transition-all duration-150 active:scale-95 cursor-pointer {caseSensitive ? 'bg-indigo-500/25 text-indigo-200 ring-1 ring-inset ring-indigo-400/50 shadow-[0_0_10px_rgba(99,102,241,0.25)]' : 'bg-white/[0.04] text-[#B4BCD0] ring-1 ring-inset ring-white/[0.08] hover:text-white hover:bg-white/[0.08]'}"
      title="Case Sensitive (Alt+C)"
    >
      Aa
    </button>
  </div>

  <!-- Dual Mode Segmented Control: Filter vs Highlight -->
  <div class="hidden sm:flex items-center bg-[#12151E] p-0.5 rounded-lg ring-1 ring-inset ring-white/[0.09] shrink-0 font-sans text-[11px] font-medium">
    <button
      onclick={() => onModeChange("filter")}
      class="px-2 py-0.5 rounded-[6px] transition-all duration-150 active:scale-95 cursor-pointer {mode === 'filter' ? 'bg-indigo-500/25 text-indigo-200 ring-1 ring-inset ring-indigo-400/40 shadow-xs' : 'text-[#B4BCD0] hover:text-white'}"
      title="Filtra righe"
    >
      Filtro
    </button>
    <button
      onclick={() => onModeChange("highlight")}
      class="px-2 py-0.5 rounded-[6px] transition-all duration-150 active:scale-95 cursor-pointer {mode === 'highlight' ? 'bg-indigo-500/25 text-indigo-200 ring-1 ring-inset ring-indigo-400/40 shadow-xs' : 'text-[#B4BCD0] hover:text-white'}"
      title="Evidenzia nei log"
    >
      Evidenzia
    </button>
  </div>

  <!-- Match Info & Actions (Export / Copy / Prev / Next) -->
  {#if error || isLoading || query.trim().length > 0}
    <div
      class="flex items-center gap-1.5 border-l border-white/[0.09] pl-2 text-[11px] font-sans shrink-0"
      title={query.trim().length > 0 && !error ? `Completato in ${searchTimeMs.toFixed(1)}ms su ${totalLines.toLocaleString()} righe` : ""}
    >
      {#if error}
        <span class="text-rose-300 font-medium truncate max-w-[120px]" title={error}>
          {error}
        </span>
      {:else if isLoading}
        <span class="text-indigo-300 animate-pulse font-medium">Ricerca...</span>
      {:else if query.trim().length > 0}
        <span class="px-1.5 py-0.5 rounded-md ring-1 ring-inset font-mono font-medium text-[11px] {invert ? 'bg-rose-500/15 ring-rose-400/35 text-rose-200' : 'bg-white/[0.06] ring-white/[0.08] text-[#F4F6FB]'} {totalMatches === 0 ? '!text-amber-300' : ''}">
          {#if mode === 'highlight' && totalMatches > 0 && currentMatchIndex > 0}
            {currentMatchIndex}/{totalMatches.toLocaleString()}
          {:else}
            {totalMatches.toLocaleString()}
          {/if}
        </span>
      {/if}

      <!-- Contextual Export / Copy if matching -->
      {#if canExport && totalMatches > 0}
        <div class="flex items-center gap-0.5">
          <button
            onclick={onExport}
            class="p-1 rounded-md text-emerald-400 hover:text-emerald-200 hover:bg-emerald-500/20 transition cursor-pointer"
            title="Esporta {totalMatches.toLocaleString()} righe filtrate su file"
          >
            <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
            </svg>
          </button>
          <button
            onclick={onCopy}
            class="p-1 rounded-md text-emerald-400 hover:text-emerald-200 hover:bg-emerald-500/20 transition cursor-pointer"
            title="Copia negli appunti (max 5000)"
          >
            <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
            </svg>
          </button>
        </div>
      {/if}

      <!-- Navigation Arrows -->
      {#if totalMatches > 0}
        <div class="flex items-center gap-0.5">
          <button
            onclick={onPrevMatch}
            class="p-0.5 rounded hover:bg-white/[0.08] text-[#B4BCD0] hover:text-white transition active:scale-90 cursor-pointer"
            title="Precedente (Shift+Invio)"
          >
            <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M5 15l7-7 7 7" />
            </svg>
          </button>
          <button
            onclick={onNextMatch}
            class="p-0.5 rounded hover:bg-white/[0.08] text-[#B4BCD0] hover:text-white transition active:scale-90 cursor-pointer"
            title="Successivo (Invio)"
          >
            <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M19 9l-7 7-7-7" />
            </svg>
          </button>
        </div>
      {/if}
    </div>
  {/if}
</div>
