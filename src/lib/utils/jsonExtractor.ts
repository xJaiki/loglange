import type { ExtractedPayload } from "$lib/types";

/**
 * Checks if a parsed value is a meaningful JSON payload (not a PID or log bracket tag like [1015]).
 */
export function isValidJsonPayload(val: unknown): boolean {
  if (val === null || typeof val !== "object") {
    return false;
  }
  if (Array.isArray(val)) {
    // Reject simple PID / index / tag brackets like [2177] or ["main"]
    if (val.length === 0) return false;
    const hasObjectOrArray = val.some((item) => item !== null && typeof item === "object");
    if (hasObjectOrArray) return true;
    // An array of multiple elements (at least 2) that don't look like a single PID tag
    return val.length >= 2;
  }
  // An object must have at least one key
  return Object.keys(val).length > 0;
}

/**
 * Scans a string for balanced JSON object `{...}` or array `[...]` substrings.
 * Correctly accounts for quoted strings and escape characters.
 */
export function findJsonSubstring(text: string): string | null {
  const len = text.length;

  for (let start = 0; start < len; start++) {
    const ch = text[start];
    if (ch === "{" || ch === "[") {
      const openChar = ch;
      const closeChar = ch === "{" ? "}" : "]";
      let depth = 0;
      let inString = false;
      let isEscaped = false;

      for (let i = start; i < len; i++) {
        const c = text[i];

        if (inString) {
          if (isEscaped) {
            isEscaped = false;
          } else if (c === "\\") {
            isEscaped = true;
          } else if (c === '"') {
            inString = false;
          }
        } else {
          if (c === '"') {
            inString = true;
          } else if (c === openChar) {
            depth++;
          } else if (c === closeChar) {
            depth--;
            if (depth === 0) {
              const candidate = text.substring(start, i + 1);
              try {
                const parsed = JSON.parse(candidate);
                if (isValidJsonPayload(parsed)) {
                  return candidate;
                }
              } catch {
                // Not valid JSON at this boundary, continue searching
                break;
              }
            }
          }
        }
      }
    }
  }

  return null;
}

/**
 * Basic XML formatter that adds newlines and indentation.
 */
export function formatXml(xml: string): string {
  let formatted = "";
  let pad = 0;
  // Clean newlines/spaces between tags
  const cleanXml = xml.replace(/>\s*</g, "><").trim();

  // Tokenize tags and content
  const tokens = cleanXml.match(/<[^>]+>|[^<]+/g) || [];

  for (const token of tokens) {
    if (token.startsWith("</")) {
      // Closing tag
      pad = Math.max(0, pad - 1);
      formatted += "  ".repeat(pad) + token + "\n";
    } else if (token.startsWith("<") && !token.startsWith("<?") && !token.startsWith("<!") && !token.endsWith("/>")) {
      // Opening tag
      formatted += "  ".repeat(pad) + token + "\n";
      pad++;
    } else {
      // Self-closing tag, processing instruction, or text node
      formatted += "  ".repeat(pad) + token + "\n";
    }
  }

  return formatted.trim();
}

/**
 * Detects if a string contains an XML structure.
 */
export function findXmlSubstring(text: string): string | null {
  const trimmed = text.trim();
  // Check if starts with XML tag or contains an XML block
  const match = trimmed.match(/<([a-zA-Z0-9_\-:]+)(?:\s+[^>]*?)?>[\s\S]*?<\/\1>/) ||
                trimmed.match(/<\?xml[\s\S]*?\?>[\s\S]*/);
  if (match) {
    return match[0];
  }
  return null;
}

/**
 * Extracts JSON or XML payload from a log line.
 */
export function extractPayload(content: string): ExtractedPayload {
  if (!content || content.trim().length === 0) {
    return { type: "text", raw: content, formatted: content };
  }

  const trimmed = content.trim();

  // 1. Try full string as JSON first
  if ((trimmed.startsWith("{") && trimmed.endsWith("}")) || (trimmed.startsWith("[") && trimmed.endsWith("]"))) {
    try {
      const parsed = JSON.parse(trimmed);
      if (isValidJsonPayload(parsed)) {
        return {
          type: "json",
          raw: trimmed,
          parsed,
          formatted: JSON.stringify(parsed, null, 2),
        };
      }
    } catch {
      // Fall through to substring search
    }
  }

  // 2. Try embedded JSON substring
  const jsonSub = findJsonSubstring(content);
  if (jsonSub) {
    try {
      const parsed = JSON.parse(jsonSub);
      return {
        type: "json",
        raw: jsonSub,
        parsed,
        formatted: JSON.stringify(parsed, null, 2),
      };
    } catch {
      // Fall through
    }
  }

  // 3. Try XML detection
  const xmlSub = findXmlSubstring(content);
  if (xmlSub) {
    return {
      type: "xml",
      raw: xmlSub,
      formatted: formatXml(xmlSub),
    };
  }

  // 4. Default to raw text
  return {
    type: "text",
    raw: content,
    formatted: content,
  };
}
