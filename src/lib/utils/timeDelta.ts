/**
 * Time Delta Utilities for Loglance
 * Parses timestamps from log lines and calculates/formats time differences.
 */

// Comprehensive regex matching common log timestamp patterns
const ISO_DATE_REGEX = /(\d{4}-\d{2}-\d{2})[T ](\d{2}:\d{2}:\d{2})(?:[.,](\d{1,6}))?(?:Z|([+-]\d{2}:?\d{2}))?/;
const TIME_ONLY_REGEX = /\b(\d{2}):(\d{2}):(\d{2})(?:[.,](\d{1,6}))?\b/;
const SYSLOG_DATE_REGEX = /\b(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s+(\d{1,2})\s+(\d{2}):(\d{2}):(\d{2})(?:[.,](\d{1,6}))?\b/i;

const MONTH_MAP: Record<string, number> = {
  jan: 0, feb: 1, mar: 2, apr: 3, may: 4, jun: 5,
  jul: 6, aug: 7, sep: 8, oct: 9, nov: 10, dec: 11
};

/**
 * Extracts and parses a timestamp from a log line into milliseconds (epoch or relative day millis).
 * Returns null if no valid timestamp is found.
 */
export function extractTimestampMs(raw: string): number | null {
  if (!raw || raw.length < 5) return null;
  const slice = raw.slice(0, 80);

  // 1. Try ISO Date / Full Date (e.g., 2026-10-28 12:26:18.123 or 2026-10-28T12:26:18.123Z)
  const isoMatch = ISO_DATE_REGEX.exec(slice);
  if (isoMatch) {
    const [, datePart, timePart, fracPart] = isoMatch;
    // Normalize to ISO standard with dot for fractions
    const msStr = fracPart ? (fracPart + "000").slice(0, 3) : "000";
    const isoString = `${datePart}T${timePart}.${msStr}Z`;
    const parsed = Date.parse(isoString);
    if (!Number.isNaN(parsed)) {
      return parsed;
    }
  }

  // 2. Try Syslog format (e.g., Oct 28 12:26:18.123)
  const syslogMatch = SYSLOG_DATE_REGEX.exec(slice);
  if (syslogMatch) {
    const [, monthStr, dayStr, hoursStr, minsStr, secsStr, fracPart] = syslogMatch;
    const month = MONTH_MAP[monthStr.toLowerCase()];
    if (month !== undefined) {
      const now = new Date();
      const currentYear = now.getFullYear();
      const ms = fracPart ? parseInt((fracPart + "000").slice(0, 3), 10) : 0;
      const d = new Date(Date.UTC(
        currentYear,
        month,
        parseInt(dayStr, 10),
        parseInt(hoursStr, 10),
        parseInt(minsStr, 10),
        parseInt(secsStr, 10),
        ms
      ));
      return d.getTime();
    }
  }

  // 3. Try Time-only (e.g., 12:26:18.123 or 12:26:18) -> Relative milliseconds within a 24h cycle
  const timeMatch = TIME_ONLY_REGEX.exec(slice);
  if (timeMatch) {
    const [, hoursStr, minsStr, secsStr, fracPart] = timeMatch;
    const hours = parseInt(hoursStr, 10);
    const mins = parseInt(minsStr, 10);
    const secs = parseInt(secsStr, 10);
    const ms = fracPart ? parseInt((fracPart + "000").slice(0, 3), 10) : 0;
    return (hours * 3600 + mins * 60 + secs) * 1000 + ms;
  }

  return null;
}

export interface DeltaInfo {
  deltaMs: number;
  formatted: string;
  formattedShort: string;
  lineCount: number;
  fromLine: number;
  toLine: number;
  fromTimeStr?: string;
  toTimeStr?: string;
}

/**
 * Computes time delta between two timestamp millisecond values and formats it cleanly.
 */
export function calculateDelta(
  t1: number,
  t2: number,
  line1: number,
  line2: number
): DeltaInfo {
  const deltaMs = Math.abs(t2 - t1);
  const fromLine = Math.min(line1, line2);
  const toLine = Math.max(line1, line2);
  const lineCount = toLine - fromLine;

  let formatted: string;
  let formattedShort: string;

  if (deltaMs === 0) {
    formatted = "0 ms (stesso istante)";
    formattedShort = "0 ms";
  } else if (deltaMs < 1) {
    formatted = "< 1 ms";
    formattedShort = "< 1 ms";
  } else if (deltaMs < 1000) {
    formatted = `+${deltaMs.toFixed(0)} ms`;
    formattedShort = `+${deltaMs.toFixed(0)}ms`;
  } else if (deltaMs < 60000) {
    const secs = deltaMs / 1000;
    formatted = `+${secs.toFixed(3)}s (${deltaMs.toLocaleString()} ms)`;
    formattedShort = `+${secs.toFixed(3)}s`;
  } else if (deltaMs < 3600000) {
    const mins = Math.floor(deltaMs / 60000);
    const remSecs = (deltaMs % 60000) / 1000;
    formatted = `+${mins}m ${remSecs.toFixed(2)}s (${deltaMs.toLocaleString()} ms)`;
    formattedShort = `+${mins}m ${remSecs.toFixed(1)}s`;
  } else {
    const hours = Math.floor(deltaMs / 3600000);
    const remMins = Math.floor((deltaMs % 3600000) / 60000);
    const remSecs = ((deltaMs % 60000) / 1000).toFixed(0);
    formatted = `+${hours}h ${remMins}m ${remSecs}s (${(deltaMs / 1000).toFixed(1)}s)`;
    formattedShort = `+${hours}h ${remMins}m`;
  }

  return {
    deltaMs,
    formatted,
    formattedShort,
    lineCount,
    fromLine,
    toLine,
  };
}
