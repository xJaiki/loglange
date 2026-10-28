<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import type { FileOpenResult, LogTab, SearchDisplayMode } from "$lib/types";

  interface Props {
    isOpen: boolean;
    currentFile: FileOpenResult | null;
    isLive: boolean;
    searchMode: SearchDisplayMode;
    isRegex: boolean;
    caseSensitive: boolean;
    invert?: boolean;
    isDrawerOpen: boolean;
    foldingEnabled: boolean;
    totalLines: number;
    liveAnimations?: boolean;
    tabs?: LogTab[];
    activeTabId?: string | null;
    onClose: () => void;
    onAction: (actionId: string, payload?: any) => void;
  }

  let {
    isOpen,
    currentFile,
    isLive,
    liveAnimations = true,
    searchMode,
    isRegex,
    caseSensitive,
    invert = false,
    isDrawerOpen,
    foldingEnabled,
    totalLines,
    tabs = [],
    activeTabId = null,
    onClose,
    onAction
  }: Props = $props();

  let query = $state("");
  let selectedIndex = $state(0);
  let inputElement = $state<HTMLInputElement | null>(null);

  interface CommandItem {
    id: string;
    title: string;
    subtitle?: string;
    category: "Navigation" | "Stream & View" | "Search & Filter" | "File Operations";
    shortcut?: string;
    icon: string;
    payload?: any;
  }

  // Base list of commands
  const allCommands = $derived<CommandItem[]>([
    // Navigation
    {
      id: "jump_to_top",
      title: "Jump to Top (First Line)",
      subtitle: "Scroll to line 1",
      category: "Navigation",
      shortcut: "Home",
      icon: "top"
    },
    {
      id: "jump_to_bottom",
      title: "Jump to Bottom (End of Log)",
      subtitle: `Scroll to line ${totalLines > 0 ? totalLines.toLocaleString() : "end"}`,
      category: "Navigation",
      shortcut: "End",
      icon: "bottom"
    },
    {
      id: "next_match",
      title: "Next Search Match",
      subtitle: "Step to following match in current query",
      category: "Navigation",
      shortcut: "↵",
      icon: "down"
    },
    {
      id: "next_bookmark",
      title: "Next Bookmark (Segnalibro successivo)",
      subtitle: "Jump to the next bookmarked line",
      category: "Navigation",
      shortcut: "F2",
      icon: "down"
    },
    {
      id: "prev_bookmark",
      title: "Previous Bookmark (Segnalibro precedente)",
      subtitle: "Jump to the previous bookmarked line",
      category: "Navigation",
      shortcut: "⇧F2",
      icon: "top"
    },
    {
      id: "prev_match",
      title: "Previous Search Match",
      subtitle: "Step to preceding match in current query",
      category: "Navigation",
      shortcut: "⇧↵",
      icon: "up"
    },

    // Stream & View
    {
      id: "toggle_live",
      title: isLive ? "Pause Live Tail" : "Resume Live Tail",
      subtitle: isLive ? "Stop auto-scrolling on file appends" : "Lock to bottom and stream updates",
      category: "Stream & View",
      shortcut: "Space",
      icon: isLive ? "pause" : "play"
    },
    {
      id: "toggle_live_animations",
      title: liveAnimations ? "Disattiva animazioni live" : "Attiva animazioni live",
      subtitle: liveAnimations ? "Disattiva glow e micro-fade per i log in arrivo" : "Abilita glow e micro-fade fluido per i log in tempo reale",
      category: "Stream & View",
      icon: "sparkles"
    },
    {
      id: "toggle_folding",
      title: foldingEnabled ? "Disable Stacktrace Folding" : "Enable Stacktrace Folding",
      subtitle: foldingEnabled ? "Show all stacktrace frames expanded" : "Auto-collapse multi-line stacktraces into pills",
      category: "Stream & View",
      shortcut: "⌥S",
      icon: "fold"
    },
    {
      id: "expand_all_stacktraces",
      title: "Expand All Stacktrace Groups",
      subtitle: "Unfold every collapsed group in the document",
      category: "Stream & View",
      icon: "expand"
    },
    {
      id: "collapse_all_stacktraces",
      title: "Collapse All Stacktrace Groups",
      subtitle: "Fold all stacktrace frames into compact pills",
      category: "Stream & View",
      icon: "collapse"
    },
    {
      id: "toggle_drawer",
      title: isDrawerOpen ? "Close JSON / XML Inspector" : "Open JSON / XML Inspector",
      subtitle: "View structured payloads, tree view & raw data",
      category: "Stream & View",
      shortcut: "Esc",
      icon: "code"
    },
    {
      id: "toggle_drawer_pin",
      title: "Affianca / Sgancia Sidebar a destra",
      subtitle: "Alterna tra colonna affiancata fissa (non overlay) e slide-over",
      category: "Stream & View",
      icon: "code"
    },
    {
      id: "configure_log_levels",
      title: "Configure Log Levels & Keywords",
      subtitle: "Customize keywords for Error, Warning (e.g. ATTENZIONE), Info, Debug",
      category: "Stream & View",
      shortcut: "Alt+L",
      icon: "settings"
    },
    {
      id: "toggle_wrap",
      title: "Toggle Word Wrap (A capo automatico)",
      subtitle: "Alterna tra visualizzazione a capo e scorrimento orizzontale",
      category: "Stream & View",
      shortcut: "Alt+Z",
      icon: "refresh"
    },
    {
      id: "toggle_timeline",
      title: "Toggle Timeline & Time Histogram (Timeline Temporale)",
      subtitle: "Mostra o nasconde l'istogramma temporale con selezione ad area (brushing)",
      category: "Stream & View",
      shortcut: "Alt+T",
      icon: "sparkles"
    },
    {
      id: "toggle_bookmark",
      title: "Toggle Bookmark on Line (Aggiungi/Rimuovi Segnalibro)",
      subtitle: "Aggiunge o rimuove il segnalibro sulla riga selezionata",
      category: "Stream & View",
      shortcut: "Ctrl+B",
      icon: "bookmark"
    },
    {
      id: "clear_all_bookmarks",
      title: "Clear All Bookmarks (Rimuovi tutti i segnalibri)",
      subtitle: "Rimuove tutti i segnalibri impostati in questa scheda",
      category: "Stream & View",
      icon: "clear"
    },

    // Search & Filter
    {
      id: "focus_filter",
      title: "Focus Search / Filter Bar",
      subtitle: "Jump input cursor to search bar",
      category: "Search & Filter",
      shortcut: "/",
      icon: "search"
    },
    {
      id: "toggle_search_mode",
      title: searchMode === "filter" ? "Switch to Highlight Mode" : "Switch to Filter Mode",
      subtitle: searchMode === "filter" ? "Keep all lines visible and highlight matches" : "Hide lines that do not match query",
      category: "Search & Filter",
      shortcut: "Mode",
      icon: "filter"
    },
    {
      id: "toggle_regex",
      title: isRegex ? "Disable Regex Mode" : "Enable Regex Mode",
      subtitle: "Toggle SIMD regular expression evaluation",
      category: "Search & Filter",
      shortcut: ".*",
      icon: "regex"
    },
    {
      id: "toggle_case",
      title: caseSensitive ? "Case Insensitive Search" : "Case Sensitive Search",
      subtitle: "Toggle case matching",
      category: "Search & Filter",
      shortcut: "Aa",
      icon: "case"
    },
    {
      id: "toggle_invert",
      title: invert ? "Invert Filter: Turn OFF (Show matching lines)" : "Invert Filter: Turn ON (Exclude matching lines)",
      subtitle: "Mostra solo le righe che NON contengono la query",
      category: "Search & Filter",
      shortcut: "Alt+I",
      icon: "filter"
    },
    {
      id: "clear_filter",
      title: "Clear Search Filter",
      subtitle: "Reset search query and display all rows",
      category: "Search & Filter",
      icon: "clear"
    },
    {
      id: "export_filtered",
      title: "Export Filtered Lines to File...",
      subtitle: "Salva tutte le righe attualmente filtrate su disco",
      category: "Search & Filter",
      icon: "folder"
    },
    {
      id: "copy_filtered",
      title: "Copy All Filtered Lines to Clipboard",
      subtitle: "Copia negli appunti fino a 5.000 righe filtrate",
      category: "Search & Filter",
      icon: "copy"
    },
    {
      id: "copy_selected_lines",
      title: "Copy Selected Lines to Clipboard",
      subtitle: "Copia le righe selezionate negli appunti",
      category: "Navigation",
      shortcut: "⌘C",
      icon: "copy"
    },
    {
      id: "copy_selected_lines_with_numbers",
      title: "Copy Selected Lines with Line Numbers",
      subtitle: "Copia le righe selezionate includendo i numeri di riga",
      category: "Navigation",
      shortcut: "⌘⇧C",
      icon: "copy"
    },
    {
      id: "select_all_lines",
      title: "Select All Lines",
      subtitle: "Seleziona tutte le righe del documento",
      category: "Navigation",
      shortcut: "⌘A",
      icon: "line"
    },

    // File Operations
    {
      id: "open_file",
      title: "Open File...",
      subtitle: "Open a log file via native system picker",
      category: "File Operations",
      shortcut: "⌘O",
      icon: "folder"
    },
    {
      id: "reload_file",
      title: "Reload Current File",
      subtitle: currentFile ? currentFile.file_name : "Re-read file from disk",
      category: "File Operations",
      shortcut: "⌘R",
      icon: "refresh"
    },
    {
      id: "copy_file_path",
      title: "Copy File Path",
      subtitle: currentFile ? currentFile.path : "No file open",
      category: "File Operations",
      icon: "copy"
    },
    ...(currentFile
      ? [
          {
            id: "close_current_tab",
            title: `Close Tab (${currentFile.file_name})`,
            subtitle: "Close the currently active log tab",
            category: "File Operations" as const,
            shortcut: "⌘W",
            icon: "clear"
          }
        ]
      : []),
    ...(tabs.length > 1
      ? [
          {
            id: "close_other_tabs",
            title: "Close Other Tabs",
            subtitle: "Close all open tabs except the active one",
            category: "File Operations" as const,
            icon: "clear"
          },
          ...tabs
            .filter((t) => t.id !== activeTabId)
            .map((t) => ({
              id: "switch_to_tab",
              title: `Switch to Tab: ${t.fileName}`,
              subtitle: t.path,
              category: "Navigation" as const,
              icon: "folder",
              payload: t.id
            }))
        ]
      : [])
  ]);

  // Derived filtered commands handling direct syntax (:line, @timestamp) or text search
  let filteredCommands = $derived.by<CommandItem[]>(() => {
    const raw = query.trim();
    if (!raw) return allCommands;

    // Direct Jump to Line syntax: :1234
    if (raw.startsWith(":")) {
      const lineStr = raw.slice(1).trim();
      const lineNum = parseInt(lineStr, 10);
      if (!isNaN(lineNum) && lineNum > 0) {
        return [{
          id: "jump_to_line",
          title: `Jump to Line ${lineNum.toLocaleString()}`,
          subtitle: `Navigate directly to line ${lineNum}`,
          category: "Navigation",
          shortcut: "↵",
          icon: "line",
          payload: lineNum
        }];
      } else {
        return [{
          id: "noop",
          title: "Jump to Line...",
          subtitle: "Type a line number (e.g. :42)",
          category: "Navigation",
          icon: "line"
        }];
      }
    }

    // Direct Jump to Timestamp syntax: @12:34:56
    if (raw.startsWith("@")) {
      const timeStr = raw.slice(1).trim();
      if (timeStr.length > 0) {
        return [{
          id: "jump_to_timestamp",
          title: `Jump to Timestamp "${timeStr}"`,
          subtitle: `Find first log line matching timestamp prefix "${timeStr}"`,
          category: "Navigation",
          shortcut: "↵",
          icon: "time",
          payload: timeStr
        }];
      } else {
        return [{
          id: "noop",
          title: "Jump to Timestamp...",
          subtitle: "Type a timestamp prefix (e.g. @12:00 or @2026-10-27)",
          category: "Navigation",
          icon: "time"
        }];
      }
    }

    // General substring/fuzzy search
    const lower = raw.toLowerCase();
    return allCommands.filter((cmd) => {
      return (
        cmd.title.toLowerCase().includes(lower) ||
        (cmd.subtitle && cmd.subtitle.toLowerCase().includes(lower)) ||
        cmd.category.toLowerCase().includes(lower) ||
        (cmd.shortcut && cmd.shortcut.toLowerCase().includes(lower))
      );
    });
  });

  // Keep selected index within bounds
  $effect(() => {
    const list = filteredCommands;
    if (selectedIndex >= list.length) {
      selectedIndex = Math.max(0, list.length - 1);
    }
  });

  // Focus input when modal opens
  $effect(() => {
    if (isOpen) {
      query = "";
      selectedIndex = 0;
      setTimeout(() => {
        inputElement?.focus();
      }, 30);
    }
  });

  function handleKeyDown(e: KeyboardEvent) {
    if (!isOpen) return;

    if (e.key === "Escape") {
      e.preventDefault();
      onClose();
      return;
    }

    if (e.key === "ArrowDown" || (e.ctrlKey && e.key === "n")) {
      e.preventDefault();
      const list = filteredCommands;
      if (list.length > 0) {
        selectedIndex = (selectedIndex + 1) % list.length;
      }
      return;
    }

    if (e.key === "ArrowUp" || (e.ctrlKey && e.key === "p")) {
      e.preventDefault();
      const list = filteredCommands;
      if (list.length > 0) {
        selectedIndex = (selectedIndex - 1 + list.length) % list.length;
      }
      return;
    }

    if (e.key === "Enter") {
      e.preventDefault();
      const list = filteredCommands;
      if (list.length > 0 && selectedIndex < list.length) {
        const item = list[selectedIndex];
        if (item.id !== "noop") {
          onAction(item.id, item.payload);
          onClose();
        }
      }
      return;
    }
  }
</script>

<svelte:window onkeydown={handleKeyDown} />

{#if isOpen}
  <!-- Backdrop -->
  <div
    transition:fade={{ duration: 140 }}
    class="fixed inset-0 bg-black/65 backdrop-blur-md z-50 flex items-start justify-center pt-[11vh] px-4 font-sans"
    onclick={(e) => {
      if (e.target === e.currentTarget) onClose();
    }}
    role="presentation"
  >
    <!-- Modal Card -->
    <div
      transition:scale={{ duration: 180, start: 0.96, easing: cubicOut }}
      class="tactile-surface rounded-2xl w-full max-w-xl overflow-hidden flex flex-col max-h-[75vh] shadow-[0_24px_64px_rgba(0,0,0,0.75)]"
      role="dialog"
      aria-modal="true"
      aria-label="Command Palette"
    >
      <!-- Search Input Bar -->
      <div class="flex items-center px-4 py-3.5 border-b border-white/[0.08] bg-white/[0.02] gap-3">
        <svg class="w-4 h-4 text-indigo-400 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
        </svg>

        <input
          bind:this={inputElement}
          bind:value={query}
          type="text"
          placeholder="Digita un comando, :riga, oppure @orario..."
          class="flex-1 bg-transparent text-[#F4F6FB] placeholder-[#7E879B] text-sm outline-none font-sans"
        />

        {#if query}
          <button
            onclick={() => (query = "")}
            class="text-[#B4BCD0] hover:text-white text-xs px-2 py-0.5 rounded-md bg-white/[0.07] cursor-pointer transition"
          >
            Pulisci
          </button>
        {/if}

        <kbd class="tactile-kbd text-[10px] px-1.5 py-0.5 rounded-md text-[#B4BCD0] select-none">
          ESC
        </kbd>
      </div>

      <!-- Command List -->
      <div class="flex-1 overflow-y-auto p-2 space-y-0.5">
        {#if filteredCommands.length === 0}
          <div class="py-12 text-center text-[#B4BCD0] text-xs space-y-1.5 font-sans">
            <p class="font-semibold text-[#F4F6FB] text-sm">Nessun comando trovato</p>
            <p>Prova a cercare un comando o usa <code class="font-mono text-indigo-300 bg-white/[0.06] px-1.5 py-0.5 rounded">:120</code> per saltare a una riga</p>
          </div>
        {:else}
          {#each filteredCommands as cmd, idx}
            <button
              class="w-full text-left px-3 py-2.5 rounded-xl text-xs flex items-center justify-between transition-all duration-100 group cursor-pointer font-sans {idx === selectedIndex ? 'bg-indigo-500/20 ring-1 ring-inset ring-indigo-400/40 text-white shadow-[inset_0_1px_0_rgba(255,255,255,0.1)]' : 'text-[#B4BCD0] hover:bg-white/[0.04] hover:text-white'}"
              onclick={() => {
                if (cmd.id !== "noop") {
                  onAction(cmd.id, cmd.payload);
                  onClose();
                }
              }}
              onmouseenter={() => (selectedIndex = idx)}
            >
              <div class="flex items-center gap-3 truncate">
                <!-- Icon badge -->
                <span class="w-7 h-7 rounded-lg flex items-center justify-center shrink-0 transition-colors {idx === selectedIndex ? 'bg-indigo-500/30 text-indigo-200 ring-1 ring-inset ring-indigo-400/40' : 'bg-white/[0.05] text-[#B4BCD0] group-hover:text-white'}">
                  {#if cmd.icon === "top"}
                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 10l7-7m0 0l7 7m-7-7v18" /></svg>
                  {:else if cmd.icon === "bottom"}
                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 14l-7 7m0 0l-7-7m7 7V3" /></svg>
                  {:else if cmd.icon === "line"}
                    <span class="text-[11px] font-mono font-bold">#</span>
                  {:else if cmd.icon === "time"}
                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" /></svg>
                  {:else if cmd.icon === "folder"}
                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 19a2 2 0 01-2-2V7a2 2 0 012-2h4l2 2h4a2 2 0 012 2v1M5 19h14a2 2 0 002-2v-5a2 2 0 00-2-2H9a2 2 0 00-2 2v5a2 2 0 01-2 2z" /></svg>
                  {:else if cmd.icon === "play"}
                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" /></svg>
                  {:else if cmd.icon === "pause"}
                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 9v6m4-6v6m7-3a9 9 0 11-18 0 9 9 0 0118 0z" /></svg>
                  {:else if cmd.icon === "fold" || cmd.icon === "collapse"}
                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" /></svg>
                  {:else if cmd.icon === "expand"}
                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 8V4m0 0h4M4 4l5 5m11-1V4m0 0h-4m4 0l-5 5M4 16v4m0 0h4m-4 0l5-5m11 5l-5-5m5 5v-4m0 4h-4" /></svg>
                  {:else if cmd.icon === "code"}
                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4" /></svg>
                  {:else if cmd.icon === "search" || cmd.icon === "filter"}
                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 4a1 1 0 011-1h16a1 1 0 011 1v2.586a1 1 0 01-.293.707l-6.414 6.414a1 1 0 00-.293.707V17l-4 4v-6.586a1 1 0 00-.293-.707L3.293 7.293A1 1 0 013 6.586V4z" /></svg>
                  {:else if cmd.icon === "copy"}
                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" /></svg>
                  {:else if cmd.icon === "sparkles"}
                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 3v4M3 5h4M6 17v4m-2-2h4m5-16l2.286 6.857L21 12l-5.714 2.143L13 21l-2.286-6.857L5 12l5.714-2.143L13 3z" /></svg>
                  {:else}
                    <span class="text-[10px] font-bold">›</span>
                  {/if}
                </span>

                <div class="truncate font-sans">
                  <div class="font-semibold truncate {idx === selectedIndex ? 'text-white' : 'text-[#F4F6FB]'}">
                    {cmd.title}
                  </div>
                  {#if cmd.subtitle}
                    <div class="text-[11px] text-[#B4BCD0] truncate font-sans">
                      {cmd.subtitle}
                    </div>
                  {/if}
                </div>
              </div>

              <!-- Shortcut / Category Badge -->
              <div class="flex items-center gap-2 shrink-0 ml-3">
                <span class="text-[10px] font-sans px-2 py-0.5 rounded-md bg-black/30 text-[#B4BCD0] ring-1 ring-inset ring-white/[0.07]">
                  {cmd.category}
                </span>
                {#if cmd.shortcut}
                  <kbd class="tactile-kbd text-[10px] px-1.5 py-0.5 rounded-md text-indigo-300">
                    {cmd.shortcut}
                  </kbd>
                {/if}
              </div>
            </button>
          {/each}
        {/if}
      </div>

      <!-- Footer Hints -->
      <div class="px-4 py-2.5 border-t border-white/[0.08] bg-[#0B0D13]/90 text-[11px] text-[#B4BCD0] font-sans flex items-center justify-between select-none">
        <div class="flex items-center gap-3">
          <span><kbd class="tactile-kbd px-1.5 py-0.5 rounded text-[10px] mr-1">↑↓</kbd> naviga</span>
          <span><kbd class="tactile-kbd px-1.5 py-0.5 rounded text-[10px] mr-1">↵</kbd> esegui</span>
          <span><kbd class="tactile-kbd px-1.5 py-0.5 rounded text-[10px] mr-1">esc</kbd> chiudi</span>
        </div>
        <div class="flex items-center gap-2">
          <span><code class="bg-white/[0.06] px-1.5 py-0.5 rounded ring-1 ring-inset ring-white/[0.08] text-emerald-300 font-mono text-[10px]">:riga</code> salta</span>
          <span><code class="bg-white/[0.06] px-1.5 py-0.5 rounded ring-1 ring-inset ring-white/[0.08] text-indigo-300 font-mono text-[10px]">@ora</code> cerca</span>
        </div>
      </div>
    </div>
  </div>
{/if}
