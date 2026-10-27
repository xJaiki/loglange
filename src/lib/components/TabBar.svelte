<script lang="ts">
  import type { LogTab } from "$lib/types";

  interface Props {
    tabs: LogTab[];
    activeTabId: string | null;
    onSelectTab: (tabId: string) => void;
    onCloseTab: (tabId: string) => void;
    onNewTab: () => void;
  }

  let {
    tabs,
    activeTabId,
    onSelectTab,
    onCloseTab,
    onNewTab
  }: Props = $props();

  let tabStripRef = $state<HTMLDivElement | null>(null);

  function handleWheel(e: WheelEvent) {
    if (!tabStripRef) return;
    if (Math.abs(e.deltaX) === 0 && Math.abs(e.deltaY) > 0) {
      tabStripRef.scrollLeft += e.deltaY;
      e.preventDefault();
    }
  }

  function formatBytes(bytes: number): string {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
  }
</script>

<div
  class="flex items-center h-9 px-2 bg-[#0D1017] border-b border-white/[0.08] select-none text-xs shrink-0 z-10 gap-1.5"
  role="tablist"
  aria-label="Schede log aperte"
>
  <!-- Scrollable tabs strip -->
  <div
    bind:this={tabStripRef}
    onwheel={handleWheel}
    class="flex items-center flex-1 h-full overflow-x-auto overflow-y-hidden gap-1 scrollbar-none"
  >
    {#each tabs as tab, index (tab.id)}
      {@const isActive = tab.id === activeTabId}
      {@const hasUnread = tab.unreadLines > 0 && !isActive}

      <div
        role="tab"
        tabindex="0"
        aria-selected={isActive}
        onclick={() => onSelectTab(tab.id)}
        onkeydown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onSelectTab(tab.id);
          }
        }}
        onauxclick={(e) => {
          if (e.button === 1) {
            e.preventDefault();
            e.stopPropagation();
            onCloseTab(tab.id);
          }
        }}
        class="group relative flex items-center h-[30px] px-2.5 rounded-t-md transition-all duration-150 cursor-pointer max-w-[230px] min-w-[120px] shrink-0 gap-2 border-t-2 {isActive ? 'bg-[#171B26] text-white border-t-amber-400 border-x border-white/[0.09] shadow-[0_2px_8px_rgba(0,0,0,0.3)]' : 'bg-transparent hover:bg-white/[0.04] text-[#9AA5BE] hover:text-[#E2E8F0] border-t-transparent border-x border-transparent'}"
        title="{tab.fileName} — {tab.path} (Ctrl/⌘+{index < 9 ? index + 1 : 'Tab'})"
      >
        <!-- File Icon -->
        <svg
          class="w-3.5 h-3.5 shrink-0 transition-colors {isActive ? 'text-amber-400' : 'text-[#7D889E] group-hover:text-[#B4BCD0]'}"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"
          />
        </svg>

        <!-- File Name -->
        <span class="truncate font-medium text-[11.5px] tracking-tight flex-1">
          {tab.fileName}
        </span>

        <!-- Tab shortcut hint on early tabs (first 5 tabs) -->
        {#if index < 5 && !hasUnread}
          <span class="hidden group-hover:inline text-[9px] font-mono text-[#5A6478] shrink-0 opacity-70">
            ⌘{index + 1}
          </span>
        {/if}

        <!-- Unread Badge (when background tab receives new lines) -->
        {#if hasUnread}
          <span
            class="flex items-center gap-1 px-1.5 py-0.2 rounded-full bg-amber-500/20 text-amber-300 ring-1 ring-inset ring-amber-400/40 text-[10px] font-mono font-semibold animate-pulse shrink-0"
            title="{tab.unreadLines.toLocaleString()} nuove righe arrivate"
          >
            +{tab.unreadLines > 999 ? "999+" : tab.unreadLines}
          </span>
        {:else}
          <!-- File Size indicator (compact) -->
          <span class="text-[10px] font-mono text-[#687389] shrink-0 {isActive ? 'opacity-90' : 'opacity-60 group-hover:opacity-90'}">
            {formatBytes(tab.fileSize)}
          </span>
        {/if}

        <!-- Close Tab Button (x) -->
        <button
          type="button"
          aria-label="Chiudi {tab.fileName}"
          onclick={(e) => {
            e.stopPropagation();
            onCloseTab(tab.id);
          }}
          class="flex items-center justify-center w-4 h-4 rounded-md text-[#7D889E] hover:text-white hover:bg-white/10 active:scale-95 transition-all duration-100 shrink-0 {isActive ? 'opacity-80 hover:opacity-100' : 'opacity-0 group-hover:opacity-100'}"
          title="Chiudi scheda (Ctrl/⌘+W)"
        >
          <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>
    {/each}
  </div>

  <!-- New Tab (+) Button -->
  <button
    type="button"
    onclick={onNewTab}
    class="flex items-center justify-center w-7 h-7 rounded-lg text-[#8E9AB0] hover:text-white hover:bg-white/[0.08] active:scale-95 transition-all duration-150 shrink-0 cursor-pointer"
    title="Apri un altro file di log (Ctrl/⌘+T o Ctrl/⌘+O)"
    aria-label="Apri nuovo file"
  >
    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
    </svg>
  </button>
</div>

<style>
  .scrollbar-none::-webkit-scrollbar {
    display: none;
  }
  .scrollbar-none {
    -ms-overflow-style: none;
    scrollbar-width: none;
  }
</style>
