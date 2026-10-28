import { escapeRegex } from "$lib/utils/highlight";

export type StandardLogLevel = "ERROR" | "WARN" | "INFO" | "DEBUG" | "TRACE";

export interface LogLevelConfig {
  id: StandardLogLevel;
  label: string;
  colorName: "rose" | "amber" | "sky" | "slate" | "purple";
  badgeClass: string;
  rowTintClass: string;
  stripClass: string;
  textColor: string;
  keywords: string[];
}

export interface LogPreset {
  id: string;
  name: string;
  description: string;
  levels: Record<StandardLogLevel, string[]>;
}

export const LOG_PRESETS: LogPreset[] = [
  {
    id: "standard",
    name: "Standard (Default)",
    description: "Standard de facto (RFC 5424, Log4j, Python, Node.js)",
    levels: {
      ERROR: ["FATAL", "CRITICAL", "PANIC", "ERROR", "ERR"],
      WARN: ["WARN", "WARNING"],
      INFO: ["INFO", "NOTICE"],
      DEBUG: ["DEBUG"],
      TRACE: ["TRACE", "VERBOSE"],
    },
  },
  {
    id: "italian",
    name: "Italiano / Localizzato",
    description: "Terminologia italiana (ATTENZIONE, AVVISO, ERRORE, GUASTO...)",
    levels: {
      ERROR: ["FATAL", "CRITICAL", "PANIC", "ERROR", "ERR", "ERRORE", "GUASTO", "BLOCCANTE"],
      WARN: ["WARN", "WARNING", "ATTENZIONE", "AVVISO", "CAUTELA"],
      INFO: ["INFO", "NOTICE", "INFORMAZIONE", "SEGNALAZIONE", "NOTIFICA"],
      DEBUG: ["DEBUG", "DBG", "DIAGNOSTICA"],
      TRACE: ["TRACE", "VERBOSE", "TRACCIA"],
    },
  },
  {
    id: "syslog",
    name: "Syslog (RFC 5424)",
    description: "Severità standard syslog unix (EMERG, ALERT, CRIT, ERR...)",
    levels: {
      ERROR: ["EMERG", "ALERT", "CRIT", "ERR", "ERROR", "FATAL"],
      WARN: ["WARNING", "WARN"],
      INFO: ["NOTICE", "INFO"],
      DEBUG: ["DEBUG"],
      TRACE: ["TRACE"],
    },
  },
  {
    id: "short",
    name: "Abbreviato (1-3 Lettere)",
    description: "Prefissi compatti tipici di log embedded o microservizi ([E], [W], [I]...)",
    levels: {
      ERROR: ["FATAL", "ERR", "E"],
      WARN: ["WARN", "WRN", "W"],
      INFO: ["INFO", "INF", "I"],
      DEBUG: ["DEBUG", "DBG", "D"],
      TRACE: ["TRACE", "TRC", "T"],
    },
  },
];

const DEFAULT_LEVEL_CONFIGS: Record<StandardLogLevel, LogLevelConfig> = {
  ERROR: {
    id: "ERROR",
    label: "Error",
    colorName: "rose",
    badgeClass: "bg-rose-500/20 text-rose-200 ring-1 ring-inset ring-rose-400/40 shadow-[inset_0_1px_0_rgba(255,255,255,0.12)]",
    rowTintClass: "bg-rose-500/[0.055] hover:bg-rose-500/[0.11]",
    stripClass: "bg-rose-500 shadow-[0_0_8px_rgba(244,63,94,0.7)]",
    textColor: "text-rose-300",
    keywords: ["FATAL", "CRITICAL", "PANIC", "ERROR", "ERR"],
  },
  WARN: {
    id: "WARN",
    label: "Warning",
    colorName: "amber",
    badgeClass: "bg-amber-500/20 text-amber-200 ring-1 ring-inset ring-amber-400/40 shadow-[inset_0_1px_0_rgba(255,255,255,0.12)]",
    rowTintClass: "bg-amber-500/[0.04] hover:bg-amber-500/[0.09]",
    stripClass: "bg-amber-400 shadow-[0_0_8px_rgba(251,191,36,0.5)]",
    textColor: "text-amber-300",
    keywords: ["WARN", "WARNING", "ATTENZIONE", "AVVISO"],
  },
  INFO: {
    id: "INFO",
    label: "Info",
    colorName: "sky",
    badgeClass: "bg-sky-500/18 text-sky-200 ring-1 ring-inset ring-sky-400/35 shadow-[inset_0_1px_0_rgba(255,255,255,0.1)]",
    rowTintClass: "hover:bg-white/[0.04]",
    stripClass: "bg-sky-400/60",
    textColor: "text-sky-300",
    keywords: ["INFO", "NOTICE"],
  },
  DEBUG: {
    id: "DEBUG",
    label: "Debug",
    colorName: "slate",
    badgeClass: "bg-slate-400/15 text-slate-200 ring-1 ring-inset ring-slate-400/30",
    rowTintClass: "hover:bg-white/[0.04] text-[#CBD5E1]",
    stripClass: "bg-slate-500/40",
    textColor: "text-slate-300",
    keywords: ["DEBUG"],
  },
  TRACE: {
    id: "TRACE",
    label: "Trace",
    colorName: "purple",
    badgeClass: "bg-purple-500/20 text-purple-200 ring-1 ring-inset ring-purple-400/35",
    rowTintClass: "hover:bg-white/[0.04] text-[#B4BCD0]",
    stripClass: "bg-purple-400/45",
    textColor: "text-purple-300",
    keywords: ["TRACE", "VERBOSE"],
  },
};

const STORAGE_KEY = "logviewer:logLevelsConfig";

class LogLevelsManager {
  levels = $state<Record<StandardLogLevel, LogLevelConfig>>(
    JSON.parse(JSON.stringify(DEFAULT_LEVEL_CONFIGS))
  );
  activePresetId = $state<string>("custom");
  version = $state<number>(0);

  // Cached precompiled regexes
  private _cachedLevelRegexes = new Map<StandardLogLevel, RegExp>();
  private _cachedAllKeywordsRegex: RegExp | null = null;

  constructor() {
    this.loadFromStorage();
    this.recompileRegexes();
  }

  private loadFromStorage() {
    if (typeof localStorage === "undefined") return;
    try {
      const saved = localStorage.getItem(STORAGE_KEY);
      if (saved) {
        const parsed = JSON.parse(saved);
        if (parsed.levels) {
          // Merge with defaults to ensure all fields are present
          for (const key of Object.keys(DEFAULT_LEVEL_CONFIGS) as StandardLogLevel[]) {
            if (parsed.levels[key]?.keywords) {
              this.levels[key].keywords = [...parsed.levels[key].keywords];
            }
          }
        }
        if (parsed.activePresetId) {
          this.activePresetId = parsed.activePresetId;
        }
      }
    } catch (e) {
      console.warn("Impossibile caricare configurazione livelli da localStorage:", e);
    }
  }

  private saveToStorage() {
    if (typeof localStorage === "undefined") return;
    try {
      const data = {
        activePresetId: this.activePresetId,
        levels: this.levels,
      };
      localStorage.setItem(STORAGE_KEY, JSON.stringify(data));
    } catch (e) {
      console.warn("Impossibile salvare configurazione livelli in localStorage:", e);
    }
  }

  private recompileRegexes() {
    this._cachedLevelRegexes.clear();

    const allPatterns: string[] = [];
    const order: StandardLogLevel[] = ["ERROR", "WARN", "INFO", "DEBUG", "TRACE"];

    for (const lvl of order) {
      const config = this.levels[lvl];
      if (config.keywords.length > 0) {
        // Escaping each keyword
        const escaped = config.keywords
          .filter((k) => k.trim().length > 0)
          .map((k) => escapeRegex(k.trim()))
          .sort((a, b) => b.length - a.length); // Longer matches first

        if (escaped.length > 0) {
          const patternStr = `\\b(${escaped.join("|")})\\b`;
          this._cachedLevelRegexes.set(lvl, new RegExp(patternStr, "i"));
          allPatterns.push(...escaped);
        }
      }
    }

    if (allPatterns.length > 0) {
      // Sort global patterns by length descending
      const uniqueSorted = Array.from(new Set(allPatterns)).sort((a, b) => b.length - a.length);
      this._cachedAllKeywordsRegex = new RegExp(`\\b(${uniqueSorted.join("|")})\\b`, "i");
    } else {
      this._cachedAllKeywordsRegex = null;
    }

    this.version++;
  }

  /**
   * Ritorna la regex compilata per catturare qualsiasi keyword di livello
   */
  getCombinedRegex(): RegExp {
    // If not yet compiled or empty, fallback to default
    return (
      this._cachedAllKeywordsRegex ||
      /\b(FATAL|CRITICAL|PANIC|ERROR|ERR|WARN|WARNING|ATTENZIONE|AVVISO|INFO|NOTICE|DEBUG|TRACE|VERBOSE)\b/i
    );
  }

  /**
   * Ritorna la query regex usata per filtrare i log per severità (es. in Rust backend)
   */
  getFilterRegex(level: StandardLogLevel): string {
    const config = this.levels[level];
    if (!config || config.keywords.length === 0) return "";
    const escaped = config.keywords
      .filter((k) => k.trim().length > 0)
      .map((k) => escapeRegex(k.trim()))
      .sort((a, b) => b.length - a.length);
    if (escaped.length === 0) return "";
    return `\\b(${escaped.join("|")})\\b`;
  }

  /**
   * Determina il livello di log da un frammento o intera riga
   */
  detectLevel(text: string): {
    level: StandardLogLevel | null;
    badgeClass: string;
    rowTintClass: string;
    stripClass: string;
    textColor: string;
  } {
    const order: StandardLogLevel[] = ["ERROR", "WARN", "INFO", "DEBUG", "TRACE"];
    for (const lvl of order) {
      const rx = this._cachedLevelRegexes.get(lvl);
      if (rx && rx.test(text)) {
        const config = this.levels[lvl];
        return {
          level: lvl,
          badgeClass: config.badgeClass,
          rowTintClass: config.rowTintClass,
          stripClass: config.stripClass,
          textColor: config.textColor,
        };
      }
    }

    return {
      level: null,
      badgeClass: "",
      rowTintClass: "hover:bg-white/[0.04]",
      stripClass: "bg-transparent",
      textColor: "text-[#F4F6FB]",
    };
  }

  /**
   * Aggiunge una keyword a un livello di log (es. aggiunge "ATTENZIONE" a WARN)
   */
  addKeyword(level: StandardLogLevel, rawKeyword: string): boolean {
    const keyword = rawKeyword.trim().toUpperCase();
    if (!keyword) return false;

    // Se esiste già in questo livello, non duplicare
    if (this.levels[level].keywords.map((k) => k.toUpperCase()).includes(keyword)) {
      return false;
    }

    // Rimuovi la parola da altri livelli se presente per evitare conflitti
    for (const key of Object.keys(this.levels) as StandardLogLevel[]) {
      if (key !== level) {
        this.levels[key].keywords = this.levels[key].keywords.filter(
          (k) => k.toUpperCase() !== keyword
        );
      }
    }

    this.levels[level].keywords.push(keyword);
    this.activePresetId = "custom";
    this.recompileRegexes();
    this.saveToStorage();
    return true;
  }

  /**
   * Rimuove una keyword da un livello
   */
  removeKeyword(level: StandardLogLevel, rawKeyword: string) {
    const keyword = rawKeyword.trim().toUpperCase();
    this.levels[level].keywords = this.levels[level].keywords.filter(
      (k) => k.toUpperCase() !== keyword
    );
    this.activePresetId = "custom";
    this.recompileRegexes();
    this.saveToStorage();
  }

  /**
   * Applica uno dei preset precostituiti
   */
  applyPreset(presetId: string) {
    const preset = LOG_PRESETS.find((p) => p.id === presetId);
    if (!preset) return;

    for (const key of Object.keys(this.levels) as StandardLogLevel[]) {
      if (preset.levels[key]) {
        this.levels[key].keywords = [...preset.levels[key]];
      }
    }
    this.activePresetId = preset.id;
    this.recompileRegexes();
    this.saveToStorage();
  }

  /**
   * Ripristina la configurazione predefinita
   */
  resetToDefaults() {
    for (const key of Object.keys(DEFAULT_LEVEL_CONFIGS) as StandardLogLevel[]) {
      this.levels[key].keywords = [...DEFAULT_LEVEL_CONFIGS[key].keywords];
    }
    this.activePresetId = "standard";
    this.recompileRegexes();
    this.saveToStorage();
  }

  /**
   * Esporta la configurazione corrente in formato JSON
   */
  exportConfig(): string {
    return JSON.stringify(
      {
        activePresetId: this.activePresetId,
        levels: this.levels,
      },
      null,
      2
    );
  }

  /**
   * Importa la configurazione da stringa JSON
   */
  importConfig(jsonStr: string): boolean {
    try {
      const parsed = JSON.parse(jsonStr);
      if (parsed.levels) {
        for (const key of Object.keys(DEFAULT_LEVEL_CONFIGS) as StandardLogLevel[]) {
          if (Array.isArray(parsed.levels[key]?.keywords)) {
            this.levels[key].keywords = [...parsed.levels[key].keywords];
          }
        }
        this.activePresetId = parsed.activePresetId || "custom";
        this.recompileRegexes();
        this.saveToStorage();
        return true;
      }
    } catch (e) {
      console.error("Errore importazione configurazione:", e);
    }
    return false;
  }
}

export const logLevelsManager = new LogLevelsManager();
