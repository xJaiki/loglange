<script lang="ts">
  import type { SearchDisplayMode } from "$lib/types";

  interface Props {
    query: string;
    isRegex: boolean;
    caseSensitive: boolean;
    mode: SearchDisplayMode;
    totalMatches: number;
    totalLines: number;
    searchTimeMs: number;
    currentMatchIndex?: number;
    error?: string | null;
    isLoading?: boolean;
    onSearchChange: (query: string) => void;
    onToggleRegex: () => void;
    onToggleCase: () => void;
    onModeChange: (mode: SearchDisplayMode) => void;
    onNextMatch: () => void;
    onPrevMatch: () => void;
    onClear: () => void;
  }

  let {
    query = "",
    isRegex = false,
    caseSensitive = false,
    mode = "filter",
    totalMatches = 0,
    totalLines = 0,
    searchTimeMs = 0,
    currentMatchIndex = 0,
    error = null,
    isLoading = false,
    onSearchChange,
    onToggleRegex,
    onToggleCase,
    onModeChange,
    onNextMatch,
    onPrevMatch,
    onClear
  }: Props = $props();

  let inputEl = $state<HTMLInputElement | null>(null);
  let isFocused = $state(false);

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
    }
  }
</script>

<div
  class="bg-[#0B0D13]/95 rounded-xl px-3 py-1.5 flex items-center gap-2.5 transition-all duration-200 font-sans w-full max-w-xl {isFocused ? 'ring-2 ring-indigo-500/45 shadow-[inset_0_1px_0_rgba(255,255,255,0.12),0_0_0_1px_rgba(129,140,248,0.5),0_8px_24px_-4px_rgba(99,102,241,0.22)]' : 'shadow-[inset_0_1px_0_rgba(255,255,255,0.08),0_0_0_1px_rgba(255,255,255,0.1),0_2px_6px_rgba(0,0,0,0.4)] hover:shadow-[inset_0_1px_0_rgba(255,255,255,0.12),0_0_0_1px_rgba(255,255,255,0.15),0_4px_10px_rgba(0,0,0,0.45)]'}"
  role="search"
>
  <!-- Search Icon -->
  <div class="shrink-0 transition-colors duration-150 {isFocused ? 'text-indigo-400' : 'text-[#7E879B]'}">
    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
    </svg>
  </div>

  <!-- Search Input -->
  <div class="relative flex-1 min-w-[140px] flex items-center">
    <input
      bind:this={inputEl}
      type="text"
      value={query}
      oninput={handleInput}
      onkeydown={handleKeydown}
      onfocus={() => (isFocused = true)}
      onblur={() => (isFocused = false)}
      placeholder="Filtra o cerca nei log... (/ o ⌘F)"
      class="w-full bg-transparent text-[#F4F6FB] placeholder-[#7E879B] text-[12.5px] focus:outline-none pr-6 {isRegex ? 'font-mono text-indigo-200' : 'font-sans'}"
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

  <!-- Toggles: Regex & Case -->
  <div class="flex items-center gap-1 border-l border-white/[0.09] pl-2.5 shrink-0">
    <!-- Regex Mode Button -->
    <button
      onclick={onToggleRegex}
      class="px-1.5 py-0.5 rounded-md text-[11px] font-mono font-semibold transition-all duration-150 active:scale-95 cursor-pointer {isRegex ? 'bg-indigo-500/25 text-indigo-200 ring-1 ring-inset ring-indigo-400/50 shadow-[0_0_10px_rgba(99,102,241,0.25)]' : 'bg-white/[0.04] text-[#B4BCD0] ring-1 ring-inset ring-white/[0.08] hover:text-white hover:bg-white/[0.08]'}"
      title="Modalità Regex (Alt+R)"
    >
      .*
    </button>

    <!-- Case Sensitive Button -->
    <button
      onclick={onToggleCase}
      class="px-1.5 py-0.5 rounded-md text-[11px] font-sans font-semibold transition-all duration-150 active:scale-95 cursor-pointer {caseSensitive ? 'bg-indigo-500/25 text-indigo-200 ring-1 ring-inset ring-indigo-400/50 shadow-[0_0_10px_rgba(99,102,241,0.25)]' : 'bg-white/[0.04] text-[#B4BCD0] ring-1 ring-inset ring-white/[0.08] hover:text-white hover:bg-white/[0.08]'}"
      title="Case Sensitive (Alt+C)"
    >
      Aa
    </button>
  </div>

  <!-- Dual Mode Segmented Control: Filter vs Highlight -->
  <div class="flex items-center bg-[#12151E] p-0.5 rounded-lg ring-1 ring-inset ring-white/[0.09] shrink-0 font-sans text-[11px] font-medium">
    <button
      onclick={() => onModeChange("filter")}
      class="px-2.5 py-0.5 rounded-[6px] transition-all duration-150 active:scale-95 cursor-pointer {mode === 'filter' ? 'bg-indigo-500/25 text-indigo-200 ring-1 ring-inset ring-indigo-400/40 shadow-xs' : 'text-[#B4BCD0] hover:text-white'}"
      title="Modalità Filtro: mostra solo le righe corrispondenti"
    >
      Filter
    </button>
    <button
      onclick={() => onModeChange("highlight")}
      class="px-2.5 py-0.5 rounded-[6px] transition-all duration-150 active:scale-95 cursor-pointer {mode === 'highlight' ? 'bg-indigo-500/25 text-indigo-200 ring-1 ring-inset ring-indigo-400/40 shadow-xs' : 'text-[#B4BCD0] hover:text-white'}"
      title="Modalità Evidenzia: mostra tutte le righe ed evidenzia i match"
    >
      Highlight
    </button>
  </div>

  <!-- Match Info & Status -->
  {#if error || isLoading || query.trim().length > 0}
    <div
      class="flex items-center gap-1.5 border-l border-white/[0.09] pl-2.5 text-[11px] font-sans shrink-0"
      title={query.trim().length > 0 && !error ? `Ricerca completata in ${searchTimeMs.toFixed(1)}ms su ${totalLines.toLocaleString()} righe` : ""}
    >
      {#if error}
        <span class="text-rose-300 font-medium truncate max-w-[140px]" title={error}>
          {error}
        </span>
      {:else if isLoading}
        <span class="text-indigo-300 animate-pulse font-medium">Ricerca...</span>
      {:else if query.trim().length > 0}
        <span class="px-2 py-0.5 rounded-md bg-white/[0.06] ring-1 ring-inset ring-white/[0.08] font-mono font-medium {totalMatches > 0 ? 'text-[#F4F6FB]' : 'text-amber-300'}">
          {#if mode === 'highlight' && totalMatches > 0 && currentMatchIndex > 0}
            {currentMatchIndex} / {totalMatches.toLocaleString()}
          {:else}
            {totalMatches.toLocaleString()} trovati
          {/if}
        </span>
      {/if}
    </div>
  {/if}

  <!-- Navigation Arrows: Prev / Next Match -->
  {#if query.trim().length > 0 && totalMatches > 0}
    <div class="flex items-center gap-0.5 border-l border-white/[0.09] pl-1.5 shrink-0">
      <button
        onclick={onPrevMatch}
        class="p-1 rounded-md hover:bg-white/[0.08] text-[#B4BCD0] hover:text-white transition-all duration-150 active:scale-90 cursor-pointer"
        title="Match precedente (Shift+Invio)"
      >
        <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M5 15l7-7 7 7" />
        </svg>
      </button>
      <button
        onclick={onNextMatch}
        class="p-1 rounded-md hover:bg-white/[0.08] text-[#B4BCD0] hover:text-white transition-all duration-150 active:scale-90 cursor-pointer"
        title="Match successivo (Invio)"
      >
        <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M19 9l-7 7-7-7" />
        </svg>
      </button>
    </div>
  {/if}
</div>
