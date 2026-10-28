<script lang="ts">
  import {
    logLevelsManager,
    LOG_PRESETS,
    type StandardLogLevel,
  } from "$lib/stores/logLevels.svelte";

  interface Props {
    isOpen: boolean;
    onClose: () => void;
  }

  let { isOpen = false, onClose }: Props = $props();

  const levelList: StandardLogLevel[] = ["ERROR", "WARN", "INFO", "DEBUG", "TRACE"];

  let inputValues = $state<Record<StandardLogLevel, string>>({
    ERROR: "",
    WARN: "",
    INFO: "",
    DEBUG: "",
    TRACE: "",
  });

  function handleAdd(level: StandardLogLevel) {
    const val = inputValues[level].trim();
    if (!val) return;
    logLevelsManager.addKeyword(level, val);
    inputValues[level] = "";
  }

  function handleRemove(level: StandardLogLevel, keyword: string) {
    logLevelsManager.removeKeyword(level, keyword);
  }

  function handlePresetChange(e: Event) {
    const presetId = (e.target as HTMLSelectElement).value;
    logLevelsManager.applyPreset(presetId);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if isOpen}
  <!-- Backdrop -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm"
    role="dialog"
    tabindex="-1"
    aria-modal="true"
    aria-labelledby="modal-title"
    onclick={onClose}
  >
    <!-- Modal Dialog -->
    <div
      class="bg-[#0E1117] border border-white/10 rounded-xl shadow-2xl max-w-xl w-full flex flex-col overflow-hidden text-[#E6EDF3] animate-in"
      role="document"
      onclick={(e) => e.stopPropagation()}
    >
      <!-- Header -->
      <div class="px-5 py-3.5 border-b border-white/[0.08] flex items-center justify-between">
        <h2 id="modal-title" class="text-sm font-semibold tracking-tight text-white">
          Livelli di log
        </h2>

        <div class="flex items-center gap-2">
          <!-- Preset Dropdown -->
          <label class="flex items-center gap-1.5 text-xs text-[#8B949E]">
            <span>Preset:</span>
            <select
              value={logLevelsManager.activePresetId}
              onchange={handlePresetChange}
              class="bg-white/[0.05] hover:bg-white/[0.08] text-[#C9D1D9] border border-white/10 rounded-md px-2 py-1 text-xs outline-none cursor-pointer focus:border-indigo-400"
            >
              {#each LOG_PRESETS as preset}
                <option value={preset.id} class="bg-[#0E1117] text-[#E6EDF3]">
                  {preset.name}
                </option>
              {/each}
              {#if logLevelsManager.activePresetId === "custom"}
                <option value="custom" class="bg-[#0E1117] text-[#E6EDF3]">
                  Personalizzato
                </option>
              {/if}
            </select>
          </label>

          <button
            onclick={onClose}
            class="text-[#8B949E] hover:text-white p-1 rounded-md hover:bg-white/[0.08] transition-colors ml-1 cursor-pointer"
            aria-label="Chiudi"
          >
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
      </div>

      <!-- Unified Level List -->
      <div class="p-5 space-y-4 max-h-[70vh] overflow-y-auto">
        {#each levelList as lvl}
          {@const config = logLevelsManager.levels[lvl]}
          <div class="flex items-start gap-3 pb-3 border-b border-white/[0.06] last:border-b-0 last:pb-0">
            <!-- Level Badge Column -->
            <div class="w-16 shrink-0 pt-0.5">
              <span class="inline-block w-full text-center py-0.5 rounded font-mono font-semibold text-[10px] tracking-wider uppercase leading-tight {config.badgeClass}">
                {lvl}
              </span>
            </div>

            <!-- Tags + Inline Add Input -->
            <div class="flex-1 flex flex-wrap items-center gap-1.5 min-w-0">
              {#each config.keywords as kw}
                <span class="inline-flex items-center gap-1 px-2 py-0.5 rounded bg-white/[0.05] hover:bg-white/[0.08] border border-white/[0.08] text-xs font-mono text-[#C9D1D9] group transition-colors">
                  <span>{kw}</span>
                  <button
                    onclick={() => handleRemove(lvl, kw)}
                    class="text-[#8B949E] hover:text-rose-400 transition-colors p-0.5 -mr-0.5 cursor-pointer"
                    title="Rimuovi {kw}"
                    aria-label="Rimuovi {kw}"
                  >
                    <svg class="w-2.5 h-2.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M6 18L18 6M6 6l12 12" />
                    </svg>
                  </button>
                </span>
              {/each}

              <!-- Inline Quick Input -->
              <div class="inline-flex items-center">
                <input
                  type="text"
                  placeholder="+ parola..."
                  bind:value={inputValues[lvl]}
                  onkeydown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      handleAdd(lvl);
                    }
                  }}
                  class="w-24 focus:w-36 transition-all duration-150 bg-transparent placeholder-[#6E7681] text-xs font-mono text-white px-2 py-0.5 rounded border border-dashed border-white/20 focus:border-white/40 focus:bg-white/[0.04] outline-none"
                />
              </div>
            </div>
          </div>
        {/each}
      </div>

      <!-- Footer -->
      <div class="px-5 py-3 border-t border-white/[0.08] bg-black/20 flex items-center justify-between text-xs text-[#8B949E]">
        <button
          onclick={() => logLevelsManager.resetToDefaults()}
          class="hover:text-[#C9D1D9] transition-colors cursor-pointer"
        >
          Ripristina default
        </button>

        <div class="flex items-center gap-3">
          <span class="hidden sm:inline text-[11px] text-[#6E7681]">Premi Esc per chiudere</span>
          <button
            onclick={onClose}
            class="px-3.5 py-1.5 rounded-md bg-white/10 hover:bg-white/15 text-white font-medium transition-colors cursor-pointer"
          >
            Fatto
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  @keyframes modalIn {
    from {
      opacity: 0;
      transform: scale(0.97);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }

  .animate-in {
    animation: modalIn 0.12s ease-out forwards;
  }
</style>
