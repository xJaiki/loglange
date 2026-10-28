# Loglance ⚡

Un visualizzatore di log per desktop (Linux, macOS, Windows) pensato per essere **ultra-veloce, minimale e leggero**.  
Apre file di log enormi (anche di diversi gigabyte) all'istante e permette di monitorare stream in tempo reale senza rallentare né consumare memoria RAM.

Costruito con **Rust + Tauri v2** per il backend e **Svelte 5 + Tailwind CSS** per l'interfaccia.

---

## 💡 Come funziona (in parole semplici)

1. **Non carica il file in RAM (Memory Mapping):**  
   Invece di leggere tutto il file in memoria, usa `memmap2` e indicizza al volo solo le posizioni delle righe usando istruzioni SIMD. Un file da 1 GB viene aperto in meno di **40 millisecondi**.

2. **Renderizza solo quello che vedi a schermo:**  
   Che il file abbia 100 righe o 10 milioni di righe, l'interfaccia disegna solo le 50-80 righe visibili nella finestra. La memoria occupata rimane costante e lo scroll è sempre fluido a 60/120 fps.

3. **Live Tail intelligente:**  
   Quando il file cresce (ad esempio un'applicazione che scrive log in continuo), l'app fa lo scroll automatico all'ultima riga.  
   - Se muovi la rotellina in su o selezioni del testo con il mouse, si mette **automaticamente in pausa** per non farti perdere il punto.  
   - Basta premere `Spazio` o cliccare sul badge per tornare all'ultima riga in tempo reale.

4. **Ricerca istantanea (Filtro o Evidenziazione):**  
   Premendo `/` puoi cercare per testo o Regex.  
   - **Filter Mode:** mostra solo le righe che contengono il termine cercato.  
   - **Highlight Mode:** mantiene tutto il file ed evidenzia i risultati, mostrandoli anche su una minimap a lato per saltarci al volo.

5. **Ergonomia dei log (Stacktrace e JSON):**  
   - Collassa in automatico i lunghi stacktrace (Java, Python, Node, Go, Rust) in un singolo bottoncino espandibile.  
   - Se una riga contiene un oggetto JSON o XML, compare un pulsantino `{}` che apre un pannello laterale per visualizzarlo indentato o ad albero ed esplorare le chiavi.

6. **Supporto CLI e Pipe Unix:**  
   Puoi aprirlo da terminale con il percorso del file oppure inviargli i log direttamente in pipe:
   ```bash
   logviewer ./app.log
   cat server.log | logviewer
   journalctl -f | logviewer
   ```

---

## ⌨️ Scorciatoie Rapide

- `Cmd/Ctrl + K`: **Command Palette** (apri file, salta a `:linea` o cerca `@orario`)
- `/` oppure `Cmd/Ctrl + F`: Cerca o filtra nel log
- `Spazio`: Torna in fondo e riattiva il live tailing
- `Alt + S`: Espandi / collassa tutti gli stacktrace
- `Esc`: Chiudi il drawer JSON o annulla la ricerca

---

## 🚀 Come provarlo

```bash
# 1. Installa le dipendenze
make install

# 2. Avvia l'applicazione
make dev
```

### Comandi utili:

```bash
# Genera un file 'sample.log' di prova da 10.000 righe con JSON ed errori
make sample-log

# Simula uno streaming di log in tempo reale su sample.log
make stream-log

# Esegui i test automatici
make test

# Compila l'eseguibile di release
make build
```
