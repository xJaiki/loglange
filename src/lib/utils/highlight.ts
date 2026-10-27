/**
 * Escape HTML special characters to prevent XSS.
 */
export function escapeHtml(str: string): string {
  return str
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

/**
 * Escapes regex special characters.
 */
export function escapeRegex(str: string): string {
  return str.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/**
 * Safely highlights matching occurrences within text.
 * Escapes text for HTML and wraps matches in a styled `<mark>` tag.
 */
export function highlightText(
  text: string,
  query: string,
  isRegex: boolean = false,
  caseSensitive: boolean = false
): string {
  if (!query || query.trim().length === 0 || !text) {
    return escapeHtml(text);
  }

  try {
    const flags = caseSensitive ? "g" : "gi";
    const pattern = isRegex ? query : escapeRegex(query);
    const regex = new RegExp(pattern, flags);

    // Split text by regex matches to safely escape non-matching text and highlight matches
    let result = "";
    let lastIndex = 0;
    let match: RegExpExecArray | null;

    // Reset lastIndex for safety
    regex.lastIndex = 0;

    // If global regex, iterate through matches
    while ((match = regex.exec(text)) !== null) {
      // Empty match guard to prevent infinite loops (e.g. `.*` or `^`)
      if (match[0].length === 0) {
        regex.lastIndex++;
        continue;
      }

      // Add escaped text before match
      result += escapeHtml(text.substring(lastIndex, match.index));
      // Add highlighted match
      result += `<mark class="bg-[#58A6FF]/35 text-[#79C0FF] font-medium rounded-xs px-0.5">${escapeHtml(match[0])}</mark>`;
      lastIndex = match.index + match[0].length;
    }

    result += escapeHtml(text.substring(lastIndex));
    return result;
  } catch {
    // If regex execution fails or regex is invalid, return escaped plain text
    return escapeHtml(text);
  }
}
