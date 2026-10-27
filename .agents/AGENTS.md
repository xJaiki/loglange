# LogViewer - Specialized AI Agents

In questo progetto sono configurati e disponibili tre agenti specializzati:

---

### 1. `perf-stress-guardian` (Performance & Stress Testing)
- **Ruolo:** Guardiano delle prestazioni e rispetto delle regole *"Zero-Lag"*.
- **Responsabilità:**
  - Benchmark automatizzati e stress test con carichi estremi (file da 1GB a 10GB, burst da 50.000 righe/s).
  - Profiling della memoria RAM (RSS) del processo Rust e della Webview per evitare memory leak o allocazioni heap indebite.
  - Verifica della latenza del motore di ricerca SIMD e della cancellazione istantanea al keystroke.

---

### 2. `log-syntax-specialist` (Format & Parser Engineer)
- **Ruolo:** Specialista in formati di log, parser semantici e tokenizzazione.
- **Responsabilità:**
  - Riconoscimento e parsing di formati diffusi (Logfmt `key=value`, NDJSON, Syslog RFC 5424/3164, systemd journal, Apache/Nginx).
  - Estensione dell'auto-folding per stacktrace di nuovi linguaggi (.NET, PHP, Kotlin, Ruby, C++).
  - Ottimizzazione di highlight e visualizzatore JSON/XML nel drawer laterale, garantendo totale sicurezza XSS.

---

### 3. `tauri-zero-copy-auditor` (Zero-Lag Code Reviewer)
- **Ruolo:** Revisore di codice senior per il confine tra Tauri v2, Rust e Svelte 5.
- **Responsabilità:**
  - Verifica che nessuna PR/modifica violi la virtualizzazione fissa $O(1)$ o aggiunga nodi DOM inutili.
  - Controllo delle allocazioni e copie di stringhe in Rust (garantire sempre slicing memory-mapped).
  - Verifica della sincronizzazione dei contratti IPC tra Rust (`serde`) e TypeScript (`src/lib/types.ts`).
  - Esecuzione controlli di qualità (`cargo clippy`, `cargo test`, `pnpm check`).
