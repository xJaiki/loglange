import { escapeHtml, escapeRegex } from "./highlight.js";
import { findJsonSubstring, findXmlSubstring } from "./jsonExtractor.js";
import type { LogLevel } from "$lib/types";

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

// Regex patterns for tokens
const LEVEL_REGEX = /\b(FATAL|CRITICAL|PANIC|ERROR|ERR|WARN|WARNING|INFO|NOTICE|DEBUG|TRACE|VERBOSE)\b/i;
const TIMESTAMP_REGEX = /(?:\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:[.,]\d+)?(?:Z|[+-]\d{2}:?\d{2})?|\b\d{2}:\d{2}:\d{2}(?:[.,]\d+)?\b|(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s+\d{1,2}\s+\d{2}:\d{2}:\d{2})/;
const IPV4_REGEX = /\b(?:\d{1,3}\.){3}\d{1,3}(?::\d{1,5})?\b/;
const UUID_REGEX = /\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b/;
const HTTP_METHOD_REGEX = /\b(GET|POST|PUT|DELETE|PATCH|HEAD|OPTIONS)\b/;
const STACKTRACE_PREFIX_REGEX = /^\s*(?:at\s+|Caused by:|Traceback|File\s+"|\.\.\.\s+\d+\s+more|\d+:\s+0x)/;

export function detectLogLevel(text: string): {
  level: LogLevel | null;
  badgeClass: string;
  rowTintClass: string;
  textColor: string;
} {
  if (/\b(FATAL|CRITICAL|PANIC|ERROR|ERR)\b/i.test(text)) {
    return {
      level: "ERROR",
      badgeClass: "bg-rose-500/20 text-rose-200 ring-1 ring-inset ring-rose-400/40 shadow-[inset_0_1px_0_rgba(255,255,255,0.12)]",
      rowTintClass: "bg-rose-500/[0.055] hover:bg-rose-500/[0.11] border-l-2 border-rose-500/85",
      textColor: "text-rose-300",
    };
  }
  if (/\b(WARN|WARNING)\b/i.test(text)) {
    return {
      level: "WARN",
      badgeClass: "bg-amber-500/20 text-amber-200 ring-1 ring-inset ring-amber-400/40 shadow-[inset_0_1px_0_rgba(255,255,255,0.12)]",
      rowTintClass: "bg-amber-500/[0.04] hover:bg-amber-500/[0.09] border-l-2 border-amber-400/80",
      textColor: "text-amber-300",
    };
  }
  if (/\b(INFO|NOTICE)\b/i.test(text)) {
    return {
      level: "INFO",
      badgeClass: "bg-sky-500/18 text-sky-200 ring-1 ring-inset ring-sky-400/35 shadow-[inset_0_1px_0_rgba(255,255,255,0.1)]",
      rowTintClass: "hover:bg-white/[0.04] border-l-2 border-transparent",
      textColor: "text-sky-300",
    };
  }
  if (/\b(DEBUG)\b/i.test(text)) {
    return {
      level: "DEBUG",
      badgeClass: "bg-slate-400/15 text-slate-200 ring-1 ring-inset ring-slate-400/30",
      rowTintClass: "hover:bg-white/[0.04] border-l-2 border-transparent text-[#CBD5E1]",
      textColor: "text-slate-300",
    };
  }
  if (/\b(TRACE|VERBOSE)\b/i.test(text)) {
    return {
      level: "TRACE",
      badgeClass: "bg-purple-500/20 text-purple-200 ring-1 ring-inset ring-purple-400/35",
      rowTintClass: "hover:bg-white/[0.04] border-l-2 border-transparent text-[#B4BCD0]",
      textColor: "text-purple-300",
    };
  }
  return {
    level: null,
    badgeClass: "",
    rowTintClass: "hover:bg-white/[0.04] border-l-2 border-transparent",
    textColor: "text-[#F4F6FB]",
  };
}

/**
 * Tokenizes a single log line into rich HTML with column extraction, syntax coloring, and search highlighting.
 */
export function tokenizeLogLine(
  content: string,
  searchQuery: string = "",
  isRegex: boolean = false,
  caseSensitive: boolean = false
): ParsedLineInfo {
  const isStacktrace = STACKTRACE_PREFIX_REGEX.test(content);
  const { level, badgeClass, rowTintClass } = detectLogLevel(content);

  let workingContent = content;
  let extractedTimestamp: string | null = null;
  let fullTimestamp: string | null = null;

  if (!isStacktrace) {
    // 1. Extract leading timestamp if found within the first 35 chars
    const headSlice = workingContent.slice(0, 40);
    const tsMatch = TIMESTAMP_REGEX.exec(headSlice);
    if (tsMatch && tsMatch.index <= 10) {
      fullTimestamp = tsMatch[0];
      const timeMatch = /(\d{2}:\d{2}:\d{2}(?:[.,]\d{1,3})?)/.exec(fullTimestamp);
      extractedTimestamp = timeMatch ? timeMatch[1].slice(0, 12) : fullTimestamp.slice(0, 12);

      // Remove leading bracket/spaces around the timestamp
      const afterTsIdx = tsMatch.index + fullTimestamp.length;
      const prefixBeforeTs = workingContent.slice(0, tsMatch.index).trim();
      if (prefixBeforeTs === "" || prefixBeforeTs === "[") {
        let rest = workingContent.slice(afterTsIdx);
        if (prefixBeforeTs === "[" && rest.startsWith("]")) {
          rest = rest.slice(1);
        }
        workingContent = rest.replace(/^[\s\-:|]+/, "");
      }
    }

    // 2. Extract leading log level if found within the first 25 chars of remaining workingContent
    const levelSlice = workingContent.slice(0, 25);
    const lvlMatch = LEVEL_REGEX.exec(levelSlice);
    if (lvlMatch && lvlMatch.index <= 6) {
      const beforeLvl = workingContent.slice(0, lvlMatch.index).trim();
      const afterLvlIdx = lvlMatch.index + lvlMatch[0].length;
      if (beforeLvl === "" || beforeLvl === "[" || beforeLvl === "(") {
        let rest = workingContent.slice(afterLvlIdx);
        if ((beforeLvl === "[" && rest.startsWith("]")) || (beforeLvl === "(" && rest.startsWith(")"))) {
          rest = rest.slice(1);
        }
        workingContent = rest.replace(/^[\s\-:|]+/, "");
      }
    }
  }

  // Meaningful check for embedded JSON / XML structures (avoid log tags/PIDs like [1015] or <thread>)
  let hasJson = false;
  if (content.includes("{") && content.includes("}")) {
    if (/\{[\s\S]*"[^"]+"\s*:[\s\S]*\}/.test(content) || findJsonSubstring(content) !== null) {
      hasJson = true;
    }
  } else if (content.includes("[{") || content.includes("[ {")) {
    hasJson = findJsonSubstring(content) !== null;
  }

  let hasXml = false;
  if (content.includes("<") && content.includes(">") && content.includes("</")) {
    hasXml = findXmlSubstring(content) !== null;
  }

  // If search query is present, we wrap matching terms
  let searchRe: RegExp | null = null;
  if (searchQuery && searchQuery.trim().length > 0) {
    try {
      const flags = caseSensitive ? "g" : "gi";
      const pattern = isRegex ? searchQuery : escapeRegex(searchQuery);
      searchRe = new RegExp(pattern, flags);
    } catch {
      searchRe = null;
    }
  }

  // Combined tokenizer regex matching semantic tokens
  // Order matters: Timestamps, Levels, UUIDs, IPs, HTTP Methods
  const combinedRegex = new RegExp(
    `(${TIMESTAMP_REGEX.source})|(${LEVEL_REGEX.source})|(${UUID_REGEX.source})|(${IPV4_REGEX.source})|(${HTTP_METHOD_REGEX.source})`,
    "gi"
  );

  let resultHtml = "";
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = combinedRegex.exec(workingContent)) !== null) {
    if (match[0].length === 0) {
      combinedRegex.lastIndex++;
      continue;
    }

    // Text between matches
    const plainBefore = workingContent.substring(lastIndex, match.index);
    resultHtml += renderWithSearchHighlight(plainBefore, searchRe);

    const tokenText = match[0];

    // Determine token category
    if (match[1]) {
      // Timestamp
      resultHtml += `<span class="text-[#B4BCD0] font-mono select-text">${renderWithSearchHighlight(tokenText, searchRe)}</span>`;
    } else if (match[2]) {
      // Level inside message body
      const lvl = detectLogLevel(tokenText);
      resultHtml += `<span class="px-1.5 py-0.2 rounded-[4px] font-semibold text-[11px] tracking-wide inline-block select-text ${lvl.badgeClass}">${renderWithSearchHighlight(tokenText.toUpperCase(), searchRe)}</span>`;
    } else if (match[3]) {
      // UUID
      resultHtml += `<span class="text-indigo-300 font-mono select-text underline decoration-indigo-400/40 decoration-dotted underline-offset-2">${renderWithSearchHighlight(tokenText, searchRe)}</span>`;
    } else if (match[4]) {
      // IPv4
      resultHtml += `<span class="text-emerald-300 font-mono select-text">${renderWithSearchHighlight(tokenText, searchRe)}</span>`;
    } else if (match[5]) {
      // HTTP Method
      resultHtml += `<span class="text-sky-300 font-semibold font-mono select-text">${renderWithSearchHighlight(tokenText, searchRe)}</span>`;
    } else {
      resultHtml += renderWithSearchHighlight(tokenText, searchRe);
    }

    lastIndex = match.index + tokenText.length;
  }

  // Remaining tail
  const tail = workingContent.substring(lastIndex);
  resultHtml += renderWithSearchHighlight(tail, searchRe);

  return {
    level,
    levelBadgeClass: badgeClass,
    rowTintClass,
    extractedTimestamp,
    fullTimestamp,
    hasJson,
    hasXml,
    isStacktrace,
    html: resultHtml,
  };
}

/**
 * Escapes HTML and applies search `<mark>` highlighting.
 */
function renderWithSearchHighlight(text: string, searchRe: RegExp | null): string {
  if (!text) return "";
  if (!searchRe) {
    return escapeHtml(text);
  }

  searchRe.lastIndex = 0;
  let out = "";
  let lastIdx = 0;
  let m: RegExpExecArray | null;

  while ((m = searchRe.exec(text)) !== null) {
    if (m[0].length === 0) {
      searchRe.lastIndex++;
      continue;
    }
    out += escapeHtml(text.substring(lastIdx, m.index));
    out += `<mark class="bg-amber-400/25 text-amber-200 ring-1 ring-amber-400/50 rounded-[3px] px-0.5 font-medium">${escapeHtml(m[0])}</mark>`;
    lastIdx = m.index + m[0].length;
  }

  out += escapeHtml(text.substring(lastIdx));
  return out;
}
