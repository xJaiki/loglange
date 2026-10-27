# LogViewer — Specifiche Complete di Revamp UI/UX ("Precision Studio")

> **Scopo di questo documento:** Questo file è un blueprint esecutivo completo progettato per essere passato direttamente a un Agente AI o a un LLM per riscrivere l'interfaccia grafica di **LogViewer**.
> Contiene l'architettura visiva, i token CSS/Tailwind v4 esatti, le modifiche strutturali file-per-file e i vincoli non negoziabili da rispettare.

---

## 1. Visione del Design: "Linear / Raycast Precision + Structured Log Grid"

L'obiettivo è trasformare LogViewer da una utility grezza "da terminale" in un'applicazione desktop **moderna, immediata, leggibile a colpo d'occhio e curata nei minimi dettagli**, unendo tre pilastri:

1. **Estetica "Linear / Raycast Precision":**
   - **Separazione Tipografica Netta:** Tutte le barre, i pulsanti, le modali, i badge di stato e gli empty state devono usare un font **Sans-Serif** moderno (`--font-sans`: Inter, Geist, system-ui). Il font **Monospace** (`--font-mono`: JetBrains Mono, Geist Mono) è riservato **esclusivamente** alle righe di log, ai numeri di riga, ai valori JSON/XML e alle scorciatoie `<kbd>`.
   - **Palette "Warm Obsidian / Zinc":** Abbandonare il vecchio blu/nero piatto da IDE anni '10 (`#0D0E11` / `#58A6FF`) per una scala di grigi ossidiana profonda (`#090A0F` canvas, `#11131A` superfici elevate) con bordi ultra-sottili semitrasparenti (`border-white/[0.07]`), accenti **Indigo/Violet** (`#6366F1` / `#818CF8`) per l'interfaccia e colori semantici ad alto contrasto ma bilanciati per i livelli di log.
   - **Rimozione del Rumore Tecnico:** Eliminare diciture ansiogene o inutili per l'utente come `16ms Throttle` nel footer o `Indexed in Xms` sempre visibile nell'header (spostare il tempo di indicizzazione solo in un tooltip sul file o nel toast iniziale).

2. **Griglia Visiva Allineata a Colonne (Datadog / BetterStack Style):**
   - Ogni riga di log (pur restando un singolo nodo riga ad altezza fissa `24px`) non è più un "muro di testo unico", ma viene divisa visivamente dal tokenizer in **4 colonne fisse allineate**:
     1. **Gutter Numero Riga** (allineato a destra, tenue, font-mono `11px`)
     2. **Colonna Timestamp Compatta** (`width: 96px` fissa se presente: mostra `HH:mm:ss.SSS` con data completa in `title` tooltip, così tutte le righe si allineano verticalmente)
     3. **Colonna Livello Semaforica** (`width: 58px` fissa: pill badge compatto `ERROR`, `WARN`, `INFO`, `DEBUG`, `TRACE` o vuoto se assente)
     4. **Colonna Messaggio + Payload Chips** (testo principale pulito dal timestamp/livello iniziale ridondante, con chip `{ } JSON` e `<> XML` moderni a destra).
   - **Row Tinting Semaforico:** Le righe `ERROR`/`FATAL` ricevono uno sfondo leggerissimo (`bg-rose-500/[0.04]` con bordo sinistro `border-l-2 border-rose-500/70`) e le righe `WARN` (`bg-amber-500/[0.03]` con bordo sinistro `border-l-2 border-amber-500/60`), permettendo di individuare i problemi anche scorrendo a 120fps.

3. **Empty State Accogliente ("Dropzone Hub") & Quick Filters 1-Click:**
   - **Quick-Filter Pills nella Top Bar:** Filtrare gli errori non deve richiedere di scrivere regex. Accanto alla barra di ricerca devono esserci pill cliccabili con 1 click (`All`, `Errors`, `Warnings`, `JSON`) che applicano istantaneamente la ricerca corrispondente.
   - **Empty State Curato:** Quando nessun file è aperto, mostrare una card centrale elegante con area Drag & Drop interattiva, bottone primario luminoso, griglia delle scorciatoie principali (`⌘O`, `⌘K`, `/`, `Space`) e box copiabile con 1 click per l'uso da terminale (`cat app.log | logviewer`).

---

## 2. REGOLE ARCHITETTURALI NON NEGOZIABILI (DA NON VIOLARE MAI)

> [!CAUTION]
> L'LLM che implementa questo revamp **NON DEVE MAI** modificare i file Rust (`src-tauri/`) né alterare la logica reattiva IPC o i vincoli "Zero-Lag" descritti qui sotto.

1. **Altezza Riga Fissa `24px` (`itemHeight = 24`):**
   - In `VirtualList.svelte`, ogni riga deve mantenere rigorosamente `height: 24px; line-height: 24px;` e `whitespace-pre`. Mai usare `word-wrap` o righe multi-linea nella lista virtuale.
2. **Virtualizzazione $O(1)$ Intatta:**
   - Non aggiungere componenti Svelte figli dentro il ciclo `{#each Array(renderedCount) as _, i}` di `VirtualList.svelte`. Usare esclusivamente HTML nativo leggero (`div`, `span`, `button`) e `{@html parsed.html}`.
3. **Zero-XSS:**
   - In `src/lib/utils/tokenizer.ts`, qualsiasi testo proveniente dal log deve continuare a passare per `escapeHtml(...)` prima di essere avvolto in tag `<span>` o `<mark>`.
4. **Svelte 5 Runes:**
   - Usare esclusivamente la sintassi Svelte 5 (`$state`, `$derived`, `$effect`, `$props`, `onclick={...}`). Non usare mai `export let` o `on:click`.
5. **Contratti Tauri IPC Invariati:**
   - Non rinominare alcuna chiamata `invoke(...)` o evento `listen(...)` (`open_file`, `get_lines_slice`, `get_filtered_lines_slice`, `start_search`, `clear_search`, ecc.).

---

## 3. Design System & Token CSS (`src/app.css`)

Sostituire il contenuto di `src/app.css` con questo sistema di token completo che definisce sia `--font-sans` per la UI sia `--font-mono` per i dati, oltre alla nuova palette **Obsidian & Indigo**:

```css
@import "tailwindcss";

@theme {
  /* Surfaces (Warm Obsidian / Zinc scale) */
  --color-canvas: #090A0F;
  --color-surface: #11131A;
  --color-surface-elevated: #171A24;
  --color-surface-hover: #1E2230;
  --color-border-subtle: rgba(255, 255, 255, 0.07);
  --color-border-strong: rgba(255, 255, 255, 0.14);

  /* Legacy aliases per retrocompatibilità nei componenti */
  --color-dark-bg: #090A0F;
  --color-dark-elevated: #11131A;
  --color-dark-border: #1E2230;
  --color-dark-text: #F3F4F6;
  --color-dark-muted: #8E95A5;

  /* Text Hierarchy */
  --color-text-primary: #F3F4F6;
  --color-text-secondary: #9CA3AF;
  --color-text-muted: #646B7C;

  /* Brand & Accent (Indigo / Violet) */
  --color-accent: #6366F1;
  --color-accent-hover: #818CF8;

  /* Semantic Log Levels */
  --color-log-error: #F43F5E;
  --color-log-warn: #F59E0B;
  --color-log-info: #38BDF8;
  --color-log-debug: #94A3B8;
  --color-log-trace: #A855F7;

  /* Typography */
  --font-sans: 'Inter', -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  --font-mono: 'JetBrains Mono', 'Geist Mono', ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
}

:root {
  color-scheme: dark;
}

body {
  margin: 0;
  padding: 0;
  background-color: var(--color-canvas);
  color: var(--color-text-primary);
  font-family: var(--font-sans);
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  overflow: hidden;
  user-select: none;
}

/* Scrollbar custom sottile ed elegante per VirtualList e Drawer */
::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}
::-webkit-scrollbar-track {
  background: transparent;
}
::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.12);
  border-radius: 9999px;
}
::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.24);
}
```

---

## 4. Specifiche Dettagliate File-per-File

### 4.1. Tokenizer & Estrazione Colonne (`src/lib/utils/tokenizer.ts`)

Per ottenere l'allineamento a colonne stile Datadog/BetterStack senza appesantire il DOM, aggiornare `ParsedLineInfo` e `tokenizeLogLine` in `src/lib/utils/tokenizer.ts`:

#### Nuovi Campi in `ParsedLineInfo`:
```ts
export interface ParsedLineInfo {
  level: LogLevel | null;
  levelBadgeClass: string;
  rowTintClass: string;          // Classe CSS di sfondo/bordo per l'intera riga in base al livello
  extractedTimestamp: string | null; // Orario compatto es. "14:22:09.104" estratto da inizio riga
  fullTimestamp: string | null;      // Timestamp completo per l'attributo title=""
  hasJson: boolean;
  hasXml: boolean;
  isStacktrace: boolean;
  html: string;                  // HTML del messaggio (senza il timestamp/livello già estratti a inizio riga)
}
```

#### Logica di Estrazione a Inizio Riga:
1. **Estrazione Timestamp Iniziale:**
   - Verificare se la riga inizia (opzionalmente dopo parentesi quadre `[` o spazi) con un timestamp riconosciuto da `TIMESTAMP_REGEX`.
   - Se presente nei primi 35 caratteri della riga:
     - Salvare il match intero in `fullTimestamp`.
     - Estrarre la sola parte oraria `HH:mm:ss` (più eventuali millisecondi `.SSS`, max 12 char) in `extractedTimestamp`.
     - Rimuovere il timestamp (ed eventuali `]` o spazi adiacenti) dalla stringa `workingContent` così non viene duplicato nel corpo del messaggio.
2. **Estrazione Livello:**
   - Rilevare il livello con `detectLogLevel(content)`.
   - Se il token del livello compare nei primi 25 caratteri di `workingContent` (es. `[ERROR]`, `INFO:`, `WARN -`), rimuoverlo da `workingContent` perché verrà già mostrato nella **Colonna Livello dedicata**! (Se invece compare più avanti nel testo, lasciarlo pure).
3. **Colori Semaforici (`detectLogLevel`):**
   - `ERROR`:
     - `badgeClass`: `"bg-rose-500/15 text-rose-400 border border-rose-500/30"`
     - `rowTintClass`: `"bg-rose-500/[0.04] hover:bg-rose-500/[0.08] border-l-2 border-rose-500/80"`
   - `WARN`:
     - `badgeClass`: `"bg-amber-500/15 text-amber-300 border border-amber-500/30"`
     - `rowTintClass`: `"bg-amber-500/[0.03] hover:bg-amber-500/[0.07] border-l-2 border-amber-500/70"`
   - `INFO`:
     - `badgeClass`: `"bg-sky-500/15 text-sky-400 border border-sky-500/25"`
     - `rowTintClass`: `"hover:bg-white/[0.03] border-l-2 border-transparent"`
   - `DEBUG`:
     - `badgeClass`: `"bg-slate-500/15 text-slate-400 border border-slate-500/25"`
     - `rowTintClass`: `"hover:bg-white/[0.03] border-l-2 border-transparent text-slate-300"`
   - `TRACE`:
     - `badgeClass`: `"bg-purple-500/15 text-purple-400 border border-purple-500/25"`
     - `rowTintClass`: `"hover:bg-white/[0.03] border-l-2 border-transparent text-slate-400"`
   - `null` (nessun livello):
     - `badgeClass`: `""`
     - `rowTintClass`: `"hover:bg-white/[0.03] border-l-2 border-transparent"`
4. **Evidenziazione `<mark>` dei Match di Ricerca:**
   - Stile moderno ad alto contrasto:
     `<mark class="bg-amber-400/25 text-amber-200 ring-1 ring-amber-400/50 rounded-[3px] px-0.5 font-medium">${escapeHtml(m[0])}</mark>`

---

### 4.2. Griglia Virtuale a Colonne (`src/lib/components/VirtualList.svelte`)

Mantenendo **invariata** tutta la logica di virtualizzazione (`itemHeight = 24`, `linesMap`, `scrollTop`, `ResizeObserver`), aggiornare il template della singola riga dentro `{#each Array(renderedCount) as _, i}`:

#### Struttura DOM di Ogni Riga (`height: 24px`):
```svelte
<!-- Riga normale (non collapsed group) -->
<div
  role="row"
  tabindex="-1"
  onclick={() => { if (lineData) onLineClick?.(lineData); }}
  onkeydown={(e) => {
    if ((e.key === "Enter" || e.key === " ") && lineData) onLineClick?.(lineData);
  }}
  class="group flex items-center w-full min-w-max cursor-pointer transition-colors duration-75 {isSelected ? '!bg-indigo-500/15 !border-l-2 !border-indigo-400' : parsed.rowTintClass}"
  style="height: {itemHeight}px; line-height: {itemHeight}px;"
>
  <!-- 1. Colonna Numero Riga (Gutter) -->
  <div
    class="shrink-0 text-right pr-3 select-none font-mono text-[11px] text-[#646B7C] group-hover:text-[#9CA3AF] border-r border-white/[0.06] h-full flex items-center justify-end"
    style="width: {gutterWidth}px;"
  >
    {lineNum}
  </div>

  <!-- 2. Colonna Timestamp Compatta (Solo per righe non-stacktrace) -->
  {#if !lineData?.is_stacktrace}
    <div
      class="shrink-0 w-[98px] px-2.5 font-mono text-[11px] text-[#8E95A5] select-text truncate border-r border-white/[0.04] h-full flex items-center"
      title={parsed.fullTimestamp ?? ""}
    >
      {#if parsed.extractedTimestamp}
        {parsed.extractedTimestamp}
      {:else}
        <span class="opacity-20">—</span>
      {/if}
    </div>

    <!-- 3. Colonna Livello Semaforica (Fissa 60px) -->
    <div class="shrink-0 w-[60px] px-1.5 h-full flex items-center justify-center border-r border-white/[0.04] select-none">
      {#if parsed.level}
        <span class="w-full text-center py-[1px] rounded-[4px] font-sans font-bold text-[10px] tracking-wider uppercase leading-tight {parsed.levelBadgeClass}">
          {parsed.level}
        </span>
      {/if}
    </div>
  {/if}

  <!-- 4. Colonna Messaggio + Chip Payload -->
  <div class="px-3 whitespace-pre text-[12.5px] font-mono select-text flex items-center h-full {lineData?.is_stacktrace ? 'text-purple-200/75 pl-8 bg-purple-500/[0.02]' : 'text-[#F3F4F6]'}">
    {#if lineData}
      <span class="inline-block">{@html parsed.html}</span>

      {#if parsed.hasJson}
        <button
          onclick={(e) => { e.stopPropagation(); onLineClick?.(lineData); }}
          class="ml-2.5 px-1.5 py-0.5 rounded-[4px] bg-emerald-500/10 text-emerald-300 hover:bg-emerald-500/20 border border-emerald-500/25 text-[10px] font-sans font-semibold cursor-pointer transition select-none flex items-center gap-1 leading-none"
          title="Ispeziona JSON nel pannello laterale"
        >
          <span class="font-mono text-emerald-400">&#123;&#125;</span>
          <span>JSON</span>
        </button>
      {:else if parsed.hasXml}
        <button
          onclick={(e) => { e.stopPropagation(); onLineClick?.(lineData); }}
          class="ml-2.5 px-1.5 py-0.5 rounded-[4px] bg-amber-500/10 text-amber-300 hover:bg-amber-500/20 border border-amber-500/25 text-[10px] font-sans font-semibold cursor-pointer transition select-none flex items-center gap-1 leading-none"
          title="Ispeziona XML nel pannello laterale"
        >
          <span class="font-mono text-amber-400">&lt;&gt;</span>
          <span>XML</span>
        </button>
      {/if}
    {:else}
      <span class="inline-block h-3 w-48 rounded bg-white/[0.05]"></span>
    {/if}
  </div>
</div>
```

#### Riga Stacktrace Collassata (`isCollapsedGroup === true`):
- Sostituire il testo tecnico `[+ N stacktrace lines — click to expand]` con un badge moderno in font Sans-Serif:
  - Icona chevron + badge `Stacktrace · {lineData?.group_line_count} frames nascosti` + micro-label `Espandi`.

---

### 4.3. Layout Principale, Header Unificato, Quick Filters & Empty State (`src/routes/+page.svelte`)

Riorganizzare il layout di `+page.svelte` in **3 Zone Moderne** (senza toccare le funzioni di business logic, ma aggiungendo una funzione helper per i Quick Filters):

#### A. Helper per i Quick Filters (da aggiungere nello `<script>` di `+page.svelte`):
```ts
type QuickFilterPreset = "all" | "errors" | "warnings" | "json";

let activeQuickPreset = $derived.by<QuickFilterPreset | null>(() => {
  if (!searchQuery) return "all";
  if (isRegex && searchQuery === "\\b(FATAL|CRITICAL|PANIC|ERROR|ERR)\\b") return "errors";
  if (isRegex && searchQuery === "\\b(WARN|WARNING)\\b") return "warnings";
  if (!isRegex && searchQuery === "{") return "json";
  return null;
});

function applyQuickPreset(preset: QuickFilterPreset) {
  if (preset === "all") {
    clearSearch();
    return;
  }
  searchMode = "filter";
  caseSensitive = false;
  if (preset === "errors") {
    isRegex = true;
    searchQuery = "\\b(FATAL|CRITICAL|PANIC|ERROR|ERR)\\b";
  } else if (preset === "warnings") {
    isRegex = true;
    searchQuery = "\\b(WARN|WARNING)\\b";
  } else if (preset === "json") {
    isRegex = false;
    searchQuery = "{";
  }
  scheduleSearch(true);
}
```

#### B. Zona 1: Top Header Command Bar (`height: 48px` / `h-12`)
Unica barra superiore ordinata in font **Sans-Serif** (`font-sans`) con 3 sezioni:
1. **Sinistra (Brand & File Pill):**
   - Logo minimale: icona quadrata arrotondata `w-6 h-6 rounded-md bg-indigo-500/15 border border-indigo-500/30 flex items-center justify-center text-indigo-400` + nome `LogViewer` (`font-sans font-semibold text-sm tracking-tight text-white`).
   - Se `currentFile` è presente: pill del file cliccabile (per cambiare file) con icona documento, `currentFile.file_name` in grassetto, dimensione `formatBytes(currentFile.file_size)` in testo secondario, e `title="Indexed in {currentFile.index_time_ms.toFixed(1)}ms — Path: {currentFile.path}"`.
2. **Centro (Search & Filter Bar Integrata):**
   - Spostare `<FilterBar />` dal footer al **centro dell'Header** (o subito sotto in una sub-bar dedicata se si preferisce ancora più respiro, ma averla in alto al centro come in Linear/Raycast rende l'app immediatamente familiare).
3. **Destra (Quick Filters + Azioni Globali):**
   - **Quick Filter Pills** (visibili quando un file è aperto):
     - `All` (attivo quando `activeQuickPreset === 'all'`)
     - `Errors` (pallino rosso `bg-rose-500` + label `Errors`, attiva regex errori con 1 click)
     - `Warnings` (pallino ambra `bg-amber-400` + label `Warnings`, attiva regex warning con 1 click)
     - `JSON` (icona `{}` verde + label `JSON`)
   - Pulsante **Stacktrace Folding** (solo se `foldingStatus.total_groups > 0`): stile pill moderno con icona layers.
   - Pulsante **Command Palette** (`⌘K`) e pulsante **Open** (`⌘O`).

#### C. Zona 2: Empty State "Dropzone Hub" (quando `!currentFile && !isLoading`)
Sostituire il vecchio empty state scarno con una **Hero Card Centrale** (`max-w-lg w-full mx-auto p-8 rounded-2xl bg-[#11131A]/90 border border-white/[0.08] shadow-2xl backdrop-blur-xl`):
- **Zona Drop Interattiva:** Icona moderna illustrata in contenitore `w-14 h-14 rounded-2xl bg-indigo-500/10 border border-indigo-500/20 text-indigo-400 mx-auto flex items-center justify-center`.
- **Titolo & Sottotitolo Umani:**
  - Titolo: `"Apri o trascina un file di log"` (`text-lg font-semibold text-white font-sans`)
  - Sottotitolo: `"Esplora file fino a multi-gigabyte in tempo reale, filtra errori istantaneamente e ispeziona payload JSON."` (`text-xs text-[#8E95A5] leading-relaxed`)
- **CTA Primaria:** Bottone `"Sfoglia file dal computer"` (`bg-indigo-500 hover:bg-indigo-400 text-white font-medium text-xs px-4 py-2.5 rounded-lg shadow-lg shadow-indigo-500/20 transition cursor-pointer inline-flex items-center gap-2`).
- **Griglia Scorciatoie Rapide (3 mini-card):**
  - `⌘O` → Apri file
  - `/` oppure `⌘F` → Ricerca istantanea
  - `⌘K` → Vai a riga `:120` o orario `@14:30`
- **Terminale Pipe Snippet (in fondo alla card):**
  - Piccolo box scuro `bg-[#090A0F] border border-white/[0.06] rounded-lg px-3 py-2 flex items-center justify-between text-[11px] font-mono text-[#8E95A5]` con `cat app.log | logviewer` e micro-bottone `"Copia"`.

#### D. Zona 3: Bottom Status Bar Minimalista (`height: 28px` / `h-7`)
Una barra di stato ultra-compatta (`h-7 px-3 bg-[#11131A] border-t border-white/[0.06] text-[11px] font-sans flex items-center justify-between`):
- **Sinistra:**
  - Badge **LIVE / PAUSED** ridisegnato: pill con dot pulsante verde smeraldo (`LIVE — Tail attivo`) o ambra (`PAUSED` + `+N nuove righe`).
  - Conteggio righe chiaro: es. `1,240,500 righe totali` (o `42 risultati su 1,240,500 righe` se filtrato).
- **Destra:**
  - Indicatore compatto colonne (`Colonne: Riga · Orario · Livello · Messaggio`).
  - Bottone rapido `Vai alla fine (Space)` se non si è in fondo.
  - **Rimuovere del tutto** la scritta `16ms Throttle`.

---

### 4.4. Ridisegno `<FilterBar />` (`src/lib/components/FilterBar.svelte`)

Rendere `FilterBar.svelte` una **Omnibar di ricerca moderna** ispirata a Raycast/Linear:
- **Contenitore:** `bg-[#090A0F]/90 border border-white/[0.09] focus-within:border-indigo-500/60 focus-within:ring-2 focus-within:ring-indigo-500/15 rounded-lg px-2.5 py-1 flex items-center gap-2 transition-all`.
- **Input:** Usa `font-sans text-xs text-white placeholder-[#646B7C]` (passa a `font-mono` solo quando `isRegex === true`!).
- **Controlli Segmentati (Filter vs Highlight):**
  - Pill morbida in `font-sans text-[11px] font-medium`: attivo `bg-indigo-500/20 text-indigo-300 border border-indigo-500/30`, inattivo `text-[#8E95A5] hover:text-white`.
- **Badge Risultati:**
  - Mostra `{totalMatches.toLocaleString()} trovati` con badge chiaro; il tempo in ms (`{searchTimeMs.toFixed(1)}ms`) va messo in `title` tooltip invece di occupare spazio fisso.

---

### 4.5. Ridisegno `<JsonDrawer />` (`src/lib/components/JsonDrawer.svelte`)

Trasformare il drawer laterale da "pannello grezzo JSON" a **"Log Line Inspector"** completo:
1. **Header del Drawer (`font-sans`):**
   - Mostra il **Badge del Livello** della riga selezionata (es. `ERROR` in rosso, `INFO` in azzurro) + `Riga #{line.line_number.toLocaleString()}` + eventuale orario estratto.
   - Bottoni di navigazione `↑ Riga precedente` / `↓ Riga successiva` e `✕ Chiudi` con bordi morbidi e hover `hover:bg-white/[0.06]`.
2. **Riepilogo Rapido Sopra i Tab:**
   - Una fascia compatta che permette di copiare l'intera riga raw con 1 click (`Copia riga intera`) o copiare solo il JSON formattato (`Copia JSON`).
3. **Albero JSON Interattivo Più Leggibile:**
   - Aumentare leggermente il respiro verticale dei nodi dell'albero (`py-1`), usare colori distinti e moderni per le chiavi (`text-indigo-300`), stringhe (`text-emerald-300`), numeri (`text-amber-300`), booleani/null (`text-rose-400`), e mostrare il pulsante `Copy path` come una pill elegante a comparsa su hover.

---

### 4.6. Ridisegno `<CommandPalette />` e `<Minimap />`

- **`CommandPalette.svelte`:**
  - Usare `font-sans` per titoli, categorie e sottotitoli dei comandi.
  - Usare un overlay con `backdrop-blur-md bg-black/60` e modale `bg-[#11131A] border border-white/[0.1] rounded-xl shadow-2xl`.
  - Evidenziare l'elemento selezionato con `bg-indigo-500/15 border-l-2 border-indigo-400 text-white`.
- **`Minimap.svelte`:**
  - Allargare leggermente da `16px` a `18px`, sfondo `bg-[#090A0F]/90 border-l border-white/[0.06]`, thumb `rounded-sm bg-white/15 hover:bg-white/25`, e tick dei match con colore `#818CF8` (Indigo brillante).

---

## 5. Ordine di Esecuzione Consigliato per l'Agente / LLM

Per eseguire il revamp senza errori di compilazione, procedere esattamente in questo ordine:

1. **Step 1 — `src/app.css`**: Aggiornare i token CSS, le variabili colore, i font (`--font-sans` e `--font-mono`) e lo stile scrollbar.
2. **Step 2 — `src/lib/utils/tokenizer.ts`**: Aggiornare `ParsedLineInfo`, implementare l'estrazione di `extractedTimestamp`, `fullTimestamp`, `rowTintClass` e aggiornare i colori dei badge semantici (assicurandosi che tutti i test o i tipi TypeScript compilino).
3. **Step 3 — `src/lib/components/VirtualList.svelte`**: Aggiornare il template delle righe con le 4 colonne allineate (`Gutter`, `Timestamp`, `Livello`, `Messaggio + Chips`) mantenendo `itemHeight = 24`.
4. **Step 4 — `src/lib/components/FilterBar.svelte` & `src/lib/components/Minimap.svelte`**: Applicare il nuovo stile Linear/Raycast.
5. **Step 5 — `src/lib/components/JsonDrawer.svelte` & `src/lib/components/CommandPalette.svelte`**: Applicare tipografia `font-sans` alla UI e migliorare la gerarchia visiva.
6. **Step 6 — `src/routes/+page.svelte`**: Riorganizzare l'Header con la ricerca e le Quick Filter Pills (`All`, `Errors`, `Warnings`, `JSON`), inserire il nuovo Empty State "Dropzone Hub" e snellire la Status Bar inferiore.

---

## 6. Checklist di Verifica Finale Obbligatoria

Al termine delle modifiche grafiche, l'agente **DEVE** eseguire questi comandi e verificare che terminino con **0 errori e 0 warning**:

```bash
pnpm check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

Inoltre, verificare manualmente nel codice che:
- [ ] Nessun file dentro `src-tauri/` sia stato alterato in modo incompatibile.
- [ ] `itemHeight` in `VirtualList.svelte` sia rimasto `24`.
- [ ] Tutte le scorciatoie da tastiera (`Cmd/Ctrl+K`, `Cmd/Ctrl+O`, `/`, `Space`, `Alt+S`, `Home`, `End`) siano ancora collegate in `+page.svelte`.
