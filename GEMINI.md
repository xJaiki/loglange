# LogViewer — Core Context & Architectural Rules

> Questo file viene letto automaticamente da **Antigravity** ad ogni sessione/richiesta come regola di workspace per guidare lo sviluppo delle prossime evolutive.

---

## 1. Stato Attuale del Progetto (Baseline)
Il progetto ha completato con successo tutti e 5 gli step fondamentali della roadmap iniziale:
- **Step 1 (Zero-Copy Core):** Indicizzatore memory-mapped (`memmap2`), scansione SIMD (`memchr` + `rayon`), apertura file 1GB in $<40\text{ ms}$, lista virtuale Svelte 5 con altezza riga fissa (24px).
- **Step 2 (Live Watcher & Smart Tail):** File watcher (`notify`) con lettura incrementale da `old_size` a `new_EOF`, batching e frame pacing a 16ms (60/120fps), Smart Tail Lock con pausa automatica su scroll-up/selezione testo e ripristino con `Space`.
- **Step 3 (SIMD Search):** Ricerca letterale (`memmem::Finder`) e Regex a byte SIMD, cancellazione istantanea su thread worker al keystroke, indice secondario `filtered_indices: Vec<u32>`, modalità Filter vs Highlight, minimap canvas a 16px.
- **Step 4 (Log Ergonomics):** Tokenizer semantico XSS-safe (livelli, timestamp, IPv4, UUID, HTTP), auto-folding stacktrace multi-linguaggio (`Alt + S`) a riga singola senza salti di layout, slide-over drawer per JSON/XML con albero interattivo e copia JSON path.
- **Step 5 (Keyboard UX & Stdin):** Command Palette `Cmd/Ctrl + K` (salto `:linea` e `@orario`), streaming da pipe Unix standard (`cat app.log | logviewer`), drag-and-drop nativo webview, Makefile e packaging di release.

---

## 2. Regole Architetturali Non Negoziabili ("Zero-Lag Rules")
Tutte le future evolutive devono tassativamente rispettare queste regole. Qualsiasi modifica che violi questi principi deve essere rigettata.

1. **Zero-Heap sul File:**
   - Il file non deve **MAI** essere caricato interamente in memoria heap (né vettori `Vec<u8>` completi né `Vec<String>`).
   - Usare sempre `memmap2::Mmap`. Gli indici in memoria sono esclusivamente offset byte `Vec<u64>` o indici riga secondari `Vec<u32>`.
2. **Virtualizzazione Viewport-Only Rigorosa $O(1)$:**
   - La Webview Svelte 5 renderizza unicamente le righe visibili a schermo (50–80 nodi DOM max).
   - L'altezza di ciascuna riga virtuale deve essere rigorosamente fissa a `24px` (`itemHeight = 24`), per garantire calcoli di scroll $O(1)$ senza reflow del browser.
   - Rust converte a stringa (`String::from_utf8_lossy`) solo ed esclusivamente la fetta di righe richiesta via IPC (`get_lines_slice` o `get_filtered_lines_slice`).
3. **Cancellazione Istantanea Ricerche:**
   - I thread paralleli di ricerca (`rayon`) devono verificare un token atomico di cancellazione (`AtomicBool`) a intervalli regolari ($\le 512$ righe) per abortire immediatamente al cambio di input utente.
4. **Watcher a 16ms & Append Incrementale:**
   - Il watcher non deve mai re-indicizzare il file da capo su un append: legge solo dal byte `old_size` al nuovo EOF.
   - Gli eventi `log:appended` devono essere raggruppati in una finestra temporale di 16ms per sincronizzarsi con il refresh rate del monitor.
5. **Sicurezza Assoluta (Zero XSS):**
   - Qualsiasi elaborazione di testo per l'interfaccia deve eseguire l'escaping completo delle entità HTML (`&`, `<`, `>`, `"`, `'`) **prima** di iniettare tag `<mark>` o chip grafici.

---

## 3. Mappa dei Componenti della Codebase

### Backend Rust (`src-tauri/src/`)
- `indexer.rs`: Core memory-mapped (`Mmap`), scansione SIMD newline (`memchr_iter`), chunking parallelo `rayon`, estrazione slice UTF-8, scansione rapida timestamp `find_timestamp_line`.
- `watcher.rs`: Watcher filesystem con `notify`, batching a 16ms, append incrementale e riconciliazione righe parziali.
- `search.rs`: Motore di ricerca letterale/regex, gestione `filtered_indices: Vec<u32>`, token di cancellazione atomica e campionamento minimap.
- `folding.rs`: Rilevamento pattern stacktrace (Java, Python, Node, Go, Rust, .NET, PHP), mapping righe virtuali $O(\log N)$ in `line_to_display_row`.
- `cli.rs`: Parsing argomenti CLI, rilevamento stdin pipe (`!is_terminal()`) e streaming bufferizzato su file temporaneo gestito da `tempfile`.
- `lib.rs`: Stato globale `AppState` (`Arc<RwLock<...>>`), coordinamento lifecycle e registrazione comandi IPC Tauri.

### Frontend Svelte 5 (`src/`)
- `src/routes/+page.svelte`: Orchestratore principale, gestione scorciatoie da tastiera, coordinamento stato (ricerca, selezione, visualizzazione drawer, toast).
- `src/lib/components/VirtualList.svelte`: Scroller virtuale a 24px, cache sparsa LRU (max 10.000 righe), Smart Tail Lock (`LIVE` vs `PAUSED`).
- `src/lib/components/FilterBar.svelte`: Barra inferiore con scorciatoia Vim `/`, toggle regex/case, selettore Filter vs Highlight.
- `src/lib/components/Minimap.svelte`: Scrollbar laterale su `<canvas>` (16px) ad alto DPI con match tick e scrubber interattivo.
- `src/lib/components/JsonDrawer.svelte`: Drawer laterale con albero interattivo, formattazione, copia percorso chiavi (`data.items[0].id`).
- `src/lib/components/CommandPalette.svelte`: Modale `Cmd+K` stile Raycast con jump `:linea` e `@orario`.
- `src/lib/utils/tokenizer.ts`: Tokenizer semantico (livelli log, timestamp, IPv4, UUID, HTTP) e rilevamento chip `{}` JSON / `<>` XML.
- `src/lib/utils/highlight.ts`: Escaping sicuro HTML e wrapping dei match in tag `<mark>`.
- `src/lib/utils/jsonExtractor.ts`: Estrattore JSON a parentesi bilanciate da righe ibride.
- `src/lib/types.ts`: Contratti TypeScript allineati con le strutture Serde Rust.

---

## 4. Agenti Specializzati Disponibili
Sono configurati sia a livello di workspace ([`.agents/agents/`](file:///home/jaiki/projects/personale/logViewer/.agents/agents)) che globale in Antigravity:
1. `perf-stress-guardian`: Esecuzione benchmark ($<100\text{ ms}$ per 1GB), stress test di carico (10k-50k righe/s) e profiling memoria RSS.
2. `log-syntax-specialist`: Aggiunta parser per nuovi formati (Logfmt, NDJSON, Syslog RFC 5424, Nginx/Apache) ed estensione auto-folding stacktrace.
3. `tauri-zero-copy-auditor`: Revisione codice prima del commit, verifica assenza di allocazioni heap o violazioni di virtualizzazione $O(1)$.

---

## 5. Workflow Obbligatorio per Nuove Evolutive
Ogni volta che viene richiesta una nuova funzionalità:
1. **Analisi Preventiva:** Verificare che non violi i principi Zero-Lag (zero copie heap, altezza riga fissa, non bloccante).
2. **Sviluppo Backend (se applicabile):** Implementare la logica in Rust con relativi unit test in `src-tauri/src/`.
3. **Allineamento Contratti:** Aggiornare le interfacce in `src/lib/types.ts` in perfetta corrispondenza con le struct Serde.
4. **Sviluppo Frontend:** Usare rigorosamente Svelte 5 Runes (`$state`, `$derived`, `$props`). Nessun componente DOM pesante dentro le righe virtuali.
5. **Pipeline di Verifica Finale (Obbligatoria):**
   ```bash
   pnpm check
   cargo check --manifest-path src-tauri/Cargo.toml
   cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
   cargo test --manifest-path src-tauri/Cargo.toml
   ```
   Tutti i check devono superare con **0 errori e 0 warnings**.
