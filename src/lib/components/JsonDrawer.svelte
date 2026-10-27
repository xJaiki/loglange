<script lang="ts">
  import { onMount } from "svelte";
  import { fly, fade } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import type { LogLine, ExtractedPayload } from "$lib/types";
  import { extractPayload } from "$lib/utils/jsonExtractor";
  import { tokenizeLogLine } from "$lib/utils/tokenizer";

  interface Props {
    line: LogLine | null;
    isOpen: boolean;
    isPinned?: boolean;
    onClose: () => void;
    onTogglePin?: () => void;
    onPrevLine?: () => void;
    onNextLine?: () => void;
  }

  let {
    line,
    isOpen,
    isPinned = false,
    onClose,
    onTogglePin,
    onPrevLine,
    onNextLine
  }: Props = $props();

  let activeTab = $state<"tree" | "formatted" | "raw" | "meta">("tree");
  let filterText = $state("");
  let copyToast = $state<string | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | null = null;
  let drawerWidth = $state(560);
  let isResizing = $state(false);

  // Track collapsed paths in the interactive tree
  let collapsedPaths = $state<Set<string>>(new Set());

  // Extract payload whenever `line` changes
  let payload = $derived<ExtractedPayload>(
    line ? extractPayload(line.content) : { type: "text", raw: "", formatted: "" }
  );

  // Extract semantic level & timestamp info from the selected line
  let parsedInfo = $derived(
    line ? tokenizeLogLine(line.content) : null
  );

  // Switch default tab if not JSON
  $effect(() => {
    if (payload.type !== "json" && activeTab === "tree") {
      activeTab = "formatted";
    } else if (payload.type === "json" && activeTab === "formatted") {
      activeTab = "tree";
    }
  });

  function showToast(message: string) {
    copyToast = message;
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
      copyToast = null;
    }, 2000);
  }

  async function copyToClipboard(text: string, label: string = "Copiato!") {
    try {
      await navigator.clipboard.writeText(text);
      showToast(label);
    } catch {
      const ta = document.createElement("textarea");
      ta.value = text;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand("copy");
      document.body.removeChild(ta);
      showToast(label);
    }
  }

  function toggleNode(path: string) {
    const next = new Set(collapsedPaths);
    if (next.has(path)) {
      next.delete(path);
    } else {
      next.add(path);
    }
    collapsedPaths = next;
  }

  function expandAll() {
    collapsedPaths = new Set();
  }

  function collapseAll(obj: any, currentPath = "root") {
    const next = new Set(collapsedPaths);
    function recurse(val: any, path: string) {
      if (val && typeof val === "object") {
        next.add(path);
        for (const [k, v] of Object.entries(val)) {
          recurse(v, `${path}.${k}`);
        }
      }
    }
    recurse(obj, currentPath);
    collapsedPaths = next;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (!isOpen) return;
    if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    } else if (e.altKey && e.key === "ArrowUp") {
      e.preventDefault();
      onPrevLine?.();
    } else if (e.altKey && e.key === "ArrowDown") {
      e.preventDefault();
      onNextLine?.();
    }
  }

  function startResize(e: MouseEvent) {
    e.preventDefault();
    isResizing = true;
    const startX = e.clientX;
    const startWidth = drawerWidth;

    function onPointerMove(moveEvent: MouseEvent) {
      const delta = startX - moveEvent.clientX;
      const maxWidth = Math.max(380, window.innerWidth - 180);
      const nextWidth = Math.min(maxWidth, Math.max(340, startWidth + delta));
      drawerWidth = nextWidth;
    }

    function onPointerUp() {
      isResizing = false;
      window.removeEventListener("mousemove", onPointerMove);
      window.removeEventListener("mouseup", onPointerUp);
      try {
        localStorage.setItem("logviewer:drawerWidth", String(drawerWidth));
      } catch {}
    }

    window.addEventListener("mousemove", onPointerMove);
    window.addEventListener("mouseup", onPointerUp);
  }

  onMount(() => {
    window.addEventListener("keydown", handleKeydown);
    try {
      const savedWidth = localStorage.getItem("logviewer:drawerWidth");
      if (savedWidth) {
        const parsed = parseInt(savedWidth, 10);
        if (!isNaN(parsed) && parsed >= 340 && parsed <= 1800) {
          drawerWidth = parsed;
        }
      }
    } catch {}

    return () => {
      window.removeEventListener("keydown", handleKeydown);
      if (toastTimer) clearTimeout(toastTimer);
    };
  });
</script>

{#if isOpen && line}
  <aside
    transition:fly={{ x: 420, duration: 240, easing: cubicOut }}
    class="{isPinned ? 'relative h-full flex flex-col z-30 select-text font-sans bg-[#12151E] border-l border-white/[0.08] shadow-[-8px_0_24px_rgba(0,0,0,0.4)] shrink-0' : 'fixed right-0 top-12 bottom-7 bg-[#12151E]/95 backdrop-blur-2xl shadow-[-16px_0_48px_rgba(0,0,0,0.65),inset_1px_0_0_rgba(255,255,255,0.1)] flex flex-col z-40 select-text font-sans'}"
    style="width: {drawerWidth}px;"
    aria-label="Log Line Inspector Drawer"
  >
    <!-- Resize Handle on left border -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      onmousedown={startResize}
      role="separator"
      aria-orientation="vertical"
      tabindex="-1"
      class="absolute left-0 top-0 bottom-0 w-2.5 -translate-x-1.5 cursor-col-resize z-50 group select-none flex items-center justify-center hover:bg-indigo-500/20 transition-colors"
      title="Trascina per ridimensionare il pannello"
    >
      <div class="w-0.5 h-8 rounded-full bg-white/20 group-hover:bg-indigo-400 group-hover:h-12 transition-all"></div>
    </div>
    <!-- 1. Header del Drawer (font-sans) -->
    <div class="flex items-center justify-between px-4 h-12 border-b border-white/[0.08] bg-gradient-to-b from-white/[0.03] to-transparent shrink-0 text-xs font-sans">
      <div class="flex items-center gap-2.5 truncate">
        <!-- Badge del Livello della riga selezionata -->
        {#if parsedInfo?.level}
          <span class="px-2 py-0.5 rounded-[5px] font-sans font-bold text-[10px] tracking-wider uppercase {parsedInfo.levelBadgeClass}">
            {parsedInfo.level}
          </span>
        {:else if payload.type === "json"}
          <span class="px-2 py-0.5 rounded-[5px] font-sans font-bold text-[10px] bg-emerald-500/20 text-emerald-200 ring-1 ring-inset ring-emerald-400/35">
            JSON
          </span>
        {:else if payload.type === "xml"}
          <span class="px-2 py-0.5 rounded-[5px] font-sans font-bold text-[10px] bg-amber-500/20 text-amber-200 ring-1 ring-inset ring-amber-400/35">
            XML
          </span>
        {:else}
          <span class="px-2 py-0.5 rounded-[5px] font-sans font-bold text-[10px] bg-indigo-500/20 text-indigo-200 ring-1 ring-inset ring-indigo-400/35">
            LOG
          </span>
        {/if}

        <span class="font-semibold text-[#F4F6FB] text-[13px]">
          Riga #{line.line_number.toLocaleString()}
        </span>

        {#if parsedInfo?.extractedTimestamp}
          <span
            class="font-mono text-[11px] text-[#B4BCD0] bg-white/[0.05] px-2 py-0.5 rounded-md ring-1 ring-inset ring-white/[0.08]"
            title={parsedInfo.fullTimestamp ?? ""}
          >
            {parsedInfo.extractedTimestamp}
          </span>
        {/if}
      </div>

      <!-- Bottoni di navigazione e chiusura -->
      <div class="flex items-center gap-1.5 shrink-0">
        <button
          onclick={onPrevLine}
          class="tactile-button px-2 py-1 rounded-lg text-[#B4BCD0] hover:text-white cursor-pointer flex items-center gap-1 text-[11px] font-medium"
          title="Riga precedente (Alt + ↑)"
        >
          <span>↑</span>
          <span class="hidden sm:inline">Prec</span>
        </button>
        <button
          onclick={onNextLine}
          class="tactile-button px-2 py-1 rounded-lg text-[#B4BCD0] hover:text-white cursor-pointer flex items-center gap-1 text-[11px] font-medium"
          title="Riga successiva (Alt + ↓)"
        >
          <span>↓</span>
          <span class="hidden sm:inline">Succ</span>
        </button>
        <div class="h-4 w-px bg-white/[0.09] mx-0.5"></div>
        {#if onTogglePin}
          <button
            onclick={onTogglePin}
            class="tactile-button px-2 py-1 rounded-lg flex items-center gap-1.5 text-[11px] font-medium transition-all {isPinned ? 'bg-indigo-500/25 text-indigo-200 ring-1 ring-inset ring-indigo-400/40 shadow-xs' : 'text-[#B4BCD0] hover:text-white'}"
            title={isPinned ? "Sgancia e visualizza in overlay" : "Affianca a destra come colonna fissa (non copre i log)"}
          >
            <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              {#if isPinned}
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 17V7m0 10a2 2 0 01-2 2H5a2 2 0 01-2-2V7a2 2 0 012-2h2a2 2 0 012 2m0 10a2 2 0 002 2h2a2 2 0 002-2M9 7a2 2 0 012-2h2a2 2 0 012 2m0 10V7m0 10a2 2 0 002 2h2a2 2 0 002-2V7a2 2 0 00-2-2h-2a2 2 0 00-2 2" />
              {:else}
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 5a1 1 0 011-1h14a1 1 0 011 1v14a1 1 0 01-1 1H5a1 1 0 01-1-1V5zm10 0v14" />
              {/if}
            </svg>
            <span class="hidden sm:inline">{isPinned ? 'Affiancato' : 'Affianca'}</span>
          </button>
          <div class="h-4 w-px bg-white/[0.09] mx-0.5"></div>
        {/if}
        <button
          onclick={onClose}
          class="tactile-button px-2.5 py-1 rounded-lg text-[#B4BCD0] hover:text-white cursor-pointer flex items-center gap-1 text-[11px] font-medium"
          title="Chiudi pannello (Esc)"
        >
          <span>✕</span>
          <span class="hidden sm:inline">Chiudi</span>
        </button>
      </div>
    </div>

    <!-- 2. Riepilogo Rapido Sopra i Tab -->
    <div class="flex items-center justify-between px-4 py-2.5 border-b border-white/[0.07] bg-[#0B0D13]/70 text-[11.5px] font-sans shrink-0">
      <div class="text-[#B4BCD0] flex items-center gap-2 truncate">
        <span>Offset: <strong class="font-mono text-[#F4F6FB] font-medium">{line.byte_offset.toLocaleString()} B</strong></span>
        <span class="text-white/25">·</span>
        <span>Lunghezza: <strong class="font-mono text-[#F4F6FB] font-medium">{line.content.length.toLocaleString()} char</strong></span>
      </div>

      <div class="flex items-center gap-2 shrink-0">
        <button
          onclick={() => copyToClipboard(line.content, "Riga intera copiata!")}
          class="tactile-button px-2.5 py-1 rounded-lg text-[#F4F6FB] cursor-pointer text-[11px] font-medium"
        >
          Copia riga intera
        </button>
        {#if payload.type === "json" || payload.type === "xml"}
          <button
            onclick={() => copyToClipboard(payload.formatted || payload.raw, `${payload.type.toUpperCase()} copiato!`)}
            class="px-2.5 py-1 rounded-lg bg-indigo-500/20 hover:bg-indigo-500/30 text-indigo-200 ring-1 ring-inset ring-indigo-400/40 shadow-[inset_0_1px_0_rgba(255,255,255,0.12)] transition-all duration-150 active:scale-95 cursor-pointer text-[11px] font-medium"
          >
            Copia {payload.type.toUpperCase()}
          </button>
        {/if}
      </div>
    </div>

    <!-- Toolbar: Tabs & Tree Controls -->
    <div class="flex items-center justify-between px-4 py-2 border-b border-white/[0.08] bg-[#12151E] text-xs font-sans shrink-0">
      <!-- Tabs -->
      <div class="flex items-center gap-1 bg-[#0B0D13] p-1 rounded-xl ring-1 ring-inset ring-white/[0.08]">
        {#if payload.type === "json"}
          <button
            onclick={() => (activeTab = "tree")}
            class="px-3 py-1 rounded-lg text-xs transition-all duration-150 cursor-pointer font-medium {activeTab === 'tree' ? 'bg-indigo-500/25 text-indigo-200 ring-1 ring-inset ring-indigo-400/40 shadow-xs' : 'text-[#B4BCD0] hover:text-white'}"
          >
            Tree
          </button>
        {/if}
        <button
          onclick={() => (activeTab = "formatted")}
          class="px-3 py-1 rounded-lg text-xs transition-all duration-150 cursor-pointer font-medium {activeTab === 'formatted' ? 'bg-indigo-500/25 text-indigo-200 ring-1 ring-inset ring-indigo-400/40 shadow-xs' : 'text-[#B4BCD0] hover:text-white'}"
        >
          Formatted
        </button>
        <button
          onclick={() => (activeTab = "raw")}
          class="px-3 py-1 rounded-lg text-xs transition-all duration-150 cursor-pointer font-medium {activeTab === 'raw' ? 'bg-indigo-500/25 text-indigo-200 ring-1 ring-inset ring-indigo-400/40 shadow-xs' : 'text-[#B4BCD0] hover:text-white'}"
        >
          Raw
        </button>
        <button
          onclick={() => (activeTab = "meta")}
          class="px-3 py-1 rounded-lg text-xs transition-all duration-150 cursor-pointer font-medium {activeTab === 'meta' ? 'bg-indigo-500/25 text-indigo-200 ring-1 ring-inset ring-indigo-400/40 shadow-xs' : 'text-[#B4BCD0] hover:text-white'}"
        >
          Metadata
        </button>
      </div>

      <!-- Tree Expand/Collapse Actions -->
      {#if activeTab === "tree" && payload.type === "json"}
        <div class="flex items-center gap-1">
          <button
            onclick={expandAll}
            class="px-2 py-1 rounded-lg hover:bg-white/[0.07] text-[11px] text-[#B4BCD0] hover:text-white transition cursor-pointer font-medium"
            title="Espandi tutti i nodi"
          >
            Espandi tutto
          </button>
          <button
            onclick={() => collapseAll(payload.parsed)}
            class="px-2 py-1 rounded-lg hover:bg-white/[0.07] text-[11px] text-[#B4BCD0] hover:text-white transition cursor-pointer font-medium"
            title="Comprimi tutti i nodi"
          >
            Comprimi tutto
          </button>
        </div>
      {/if}
    </div>

    <!-- Filter Bar for Tree View -->
    {#if activeTab === "tree" && payload.type === "json"}
      <div class="px-4 py-2.5 border-b border-white/[0.07] bg-[#12151E] shrink-0 font-sans">
        <div class="relative flex items-center">
          <svg class="w-3.5 h-3.5 text-[#7E879B] absolute left-3 pointer-events-none" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
          </svg>
          <input
            type="text"
            bind:value={filterText}
            placeholder="Filtra chiavi o valori nel JSON..."
            class="w-full bg-[#0B0D13] text-[#F4F6FB] placeholder-[#7E879B] text-xs pl-8 pr-7 py-1.5 rounded-lg ring-1 ring-inset ring-white/[0.1] focus:ring-2 focus:ring-indigo-500/50 outline-none font-sans transition-all"
          />
          {#if filterText}
            <button
              onclick={() => (filterText = "")}
              class="absolute right-2.5 text-[#7E879B] hover:text-white cursor-pointer"
            >
              ✕
            </button>
          {/if}
        </div>
      </div>
    {/if}

    <!-- 3. Content Body (Albero JSON Interattivo Più Leggibile) -->
    <div class="flex-1 overflow-y-auto overflow-x-auto p-4 text-[12.5px] font-mono bg-[#0B0D13]/50">
      {#if activeTab === "tree" && payload.type === "json"}
        <div class="space-y-0.5">
          {#snippet renderNode(val: any, keyName: string | null, path: string, depth: number)}
            {@const isObject = val !== null && typeof val === "object"}
            {@const isArray = Array.isArray(val)}
            {@const isCollapsed = collapsedPaths.has(path)}
            {@const entries = isObject ? Object.entries(val) : []}
            {@const matchesFilter = !filterText ||
              (keyName && keyName.toLowerCase().includes(filterText.toLowerCase())) ||
              (!isObject && String(val).toLowerCase().includes(filterText.toLowerCase()))}

            {#if matchesFilter || isObject}
              <div class="group py-0.5" style="margin-left: {depth * 14}px;">
                <div class="flex items-center gap-1.5 hover:bg-white/[0.05] rounded-lg px-2 py-0.5 -mx-2 transition-colors">
                  <!-- Expand/Collapse Chevron -->
                  {#if isObject}
                    <button
                      onclick={() => toggleNode(path)}
                      class="w-4 h-4 flex items-center justify-center text-[#7E879B] hover:text-white cursor-pointer shrink-0"
                      title={isCollapsed ? "Espandi" : "Comprimi"}
                    >
                      <svg
                        class="w-3 h-3 transition-transform duration-150 {isCollapsed ? '' : 'rotate-90'}"
                        fill="none"
                        stroke="currentColor"
                        viewBox="0 0 24 24"
                      >
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M9 5l7 7-7 7" />
                      </svg>
                    </button>
                  {:else}
                    <span class="w-4 shrink-0"></span>
                  {/if}

                  <!-- Key Name -->
                  {#if keyName !== null}
                    <span class="text-indigo-300 font-medium">{keyName}:</span>
                  {/if}

                  <!-- Value or Collapsed Object Summary -->
                  {#if isObject}
                    {#if isCollapsed}
                      <button
                        type="button"
                        class="text-[#B4BCD0] cursor-pointer hover:text-white text-left inline bg-white/[0.06] px-2 py-0.5 rounded-md text-[11px] ring-1 ring-inset ring-white/[0.09]"
                        onclick={() => toggleNode(path)}
                      >
                        {isArray ? `[ ${entries.length} items ]` : `{ ${entries.length} keys }`}
                      </button>
                    {:else}
                      <span class="text-[#7E879B]">{isArray ? "[" : "{"}</span>
                    {/if}
                  {:else if typeof val === "string"}
                    <span class="text-emerald-300 break-all">"{val}"</span>
                  {:else if typeof val === "number"}
                    <span class="text-amber-300 font-medium">{val}</span>
                  {:else if typeof val === "boolean"}
                    <span class="text-rose-400 font-semibold">{val}</span>
                  {:else if val === null}
                    <span class="text-rose-400 italic">null</span>
                  {:else}
                    <span class="text-[#F4F6FB]">{String(val)}</span>
                  {/if}

                  <!-- Inline Quick Copy Pills on Hover -->
                  <div class="opacity-0 group-hover:opacity-100 transition-opacity duration-150 flex items-center gap-1 ml-auto font-sans shrink-0">
                    <button
                      onclick={() => copyToClipboard(typeof val === "object" ? JSON.stringify(val, null, 2) : String(val), "Valore copiato")}
                      class="px-2 py-0.5 rounded-full bg-[#181C28] hover:bg-indigo-500/25 text-[10px] text-[#B4BCD0] hover:text-indigo-200 ring-1 ring-inset ring-white/[0.1] cursor-pointer transition"
                      title="Copia valore"
                    >
                      Copy val
                    </button>
                    {#if keyName !== null}
                      <button
                        onclick={() => copyToClipboard(path.replace(/^root\./, ""), "JSON path copiato")}
                        class="px-2 py-0.5 rounded-full bg-[#181C28] hover:bg-indigo-500/25 text-[10px] text-[#B4BCD0] hover:text-indigo-200 ring-1 ring-inset ring-white/[0.1] cursor-pointer transition"
                        title="Copia percorso chiave JSON"
                      >
                        Copy path
                      </button>
                    {/if}
                  </div>
                </div>

                <!-- Children when expanded -->
                {#if isObject && !isCollapsed}
                  <div class="border-l border-white/[0.07] ml-2 pl-1">
                    {#each entries as [childKey, childVal]}
                      {@render renderNode(childVal, childKey, `${path}.${childKey}`, depth + 1)}
                    {/each}
                  </div>
                  <div class="text-[#7E879B] pl-4">{isArray ? "]" : "}"}</div>
                {/if}
              </div>
            {/if}
          {/snippet}

          {@render renderNode(payload.parsed, null, "root", 0)}
        </div>
      {:else if activeTab === "formatted"}
        <pre class="whitespace-pre text-[#F4F6FB] leading-relaxed select-text font-mono text-[12.5px]">{payload.formatted || payload.raw}</pre>
      {:else if activeTab === "raw"}
        <pre class="whitespace-pre-wrap break-all text-[#F4F6FB] leading-relaxed select-text font-mono text-[12.5px]">{line.content}</pre>
      {:else if activeTab === "meta"}
        <div class="space-y-3 select-text font-sans">
          <div class="tactile-surface p-4 rounded-xl space-y-3">
            <h4 class="text-indigo-300 font-semibold text-[11px] uppercase tracking-wider">Proprietà Riga</h4>
            <div class="grid grid-cols-2 gap-2.5 text-xs">
              <span class="text-[#B4BCD0]">Numero Riga:</span>
              <span class="text-white font-mono font-semibold">{line.line_number.toLocaleString()}</span>

              <span class="text-[#B4BCD0]">Byte Offset:</span>
              <span class="text-white font-mono">{line.byte_offset.toLocaleString()} bytes</span>

              <span class="text-[#B4BCD0]">Lunghezza:</span>
              <span class="text-white font-mono">{line.content.length.toLocaleString()} caratteri</span>

              <span class="text-[#B4BCD0]">Formato Rilevato:</span>
              <span class="text-white uppercase font-mono font-semibold">{payload.type}</span>

              <span class="text-[#B4BCD0]">Stacktrace Frame:</span>
              <span class="text-white">{line.is_stacktrace ? "Sì" : "No"}</span>
            </div>
          </div>
        </div>
      {/if}
    </div>

    <!-- Toast Notification Overlay -->
    {#if copyToast}
      <div
        transition:fade={{ duration: 150 }}
        class="absolute bottom-5 left-1/2 -translate-x-1/2 px-3.5 py-1.5 rounded-xl bg-indigo-500 text-white font-sans font-semibold text-xs shadow-[0_8px_24px_rgba(99,102,241,0.45)]"
      >
        {copyToast}
      </div>
    {/if}
  </aside>
{/if}
