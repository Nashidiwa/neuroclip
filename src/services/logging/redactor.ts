/**
 * Client-Side Secret Redaction & Sanitization Layer.
 * Defense-in-depth masking preventing API keys and tokens from reaching console or IPC bridge.
 * Compliance with docs/CONVENTIONS.md §6.3 and docs/TASK_BOARD.md AC-04.1.
 */

/**
 * Mathematical secret redaction formula per CONVENTIONS.md §6.3.
 *
 * For keys of length <= 8 characters: masks completely as `[REDACTED]`.
 * For keys of length > 8 characters: preserves 4-char prefix and 4-char suffix:
 * `${key.slice(0, 4)}[REDACTED]${key.slice(-4)}`
 */
export function redactSecret(key: string): string {
  if (key.length <= 8) {
    return '[REDACTED]';
  }
  return `${key.slice(0, 4)}[REDACTED]${key.slice(-4)}`;
}

/**
 * Known API Key and Auth Token patterns across supported AI and streaming providers.
 */
const SECRET_PATTERNS: readonly RegExp[] = [
  /\bgsk_[a-zA-Z0-9_]{16,}\b/g,
  /\bAIza[0-9A-Za-z_-]{20,}\b/g,
  /\bsk-(?:proj-)?[a-zA-Z0-9_-]{20,}\b/g,
  /\boauth:[a-zA-Z0-9]{16,}\b/g,
  /\bauth-token[=:]\s*[a-zA-Z0-9_-]{16,}\b/g,
  /\bBearer\s+[a-zA-Z0-9._-]{20,}\b/g,
];

/**
 * Scan and redact sensitive tokens within a freeform log string.
 */
export function sanitizeLogString(text: string): string {
  let result = text;
  for (const regex of SECRET_PATTERNS) {
    // Reset regex state in case of global flag caching
    regex.lastIndex = 0;
    result = result.replace(regex, (match) => {
      if (match.startsWith('auth-token')) {
        return 'auth-token=[REDACTED]';
      }
      if (match.startsWith('Bearer ')) {
        return 'Bearer [REDACTED]';
      }
      return redactSecret(match);
    });
  }
  return result;
}

/**
 * Recursively sanitize string values inside an arbitrary structured log context dictionary.
 */
export function sanitizeLogContext(
  context?: Readonly<Record<string, unknown>> | undefined,
): Readonly<Record<string, unknown>> | undefined {
  if (!context) {
    return undefined;
  }

  const sanitized: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(context)) {
    if (typeof value === 'string') {
      sanitized[key] = sanitizeLogString(value);
    } else if (Array.isArray(value)) {
      sanitized[key] = value.map((item) => {
        if (typeof item === 'string') return sanitizeLogString(item);
        if (typeof item === 'object' && item !== null) {
          return sanitizeLogContext(item as Readonly<Record<string, unknown>>);
        }
        return item;
      });
    } else if (typeof value === 'object' && value !== null) {
      sanitized[key] = sanitizeLogContext(value as Readonly<Record<string, unknown>>);
    } else {
      sanitized[key] = value;
    }
  }

  return sanitized;
}
