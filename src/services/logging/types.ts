/**
 * Wire & Domain Contracts for NeuroClip Structured Logging Subsystem.
 * Strict compliance with zero-any and exactOptionalPropertyTypes: true.
 */

export type LogLevelWire = 'TRACE' | 'DEBUG' | 'INFO' | 'WARN' | 'ERROR';
export type LogLevel = 'trace' | 'debug' | 'info' | 'warn' | 'error';

/**
 * Raw JSON wire transfer format received from Tauri IPC command `get_recent_logs`.
 */
export interface LogEntryJson {
  readonly timestamp: string;
  readonly level: LogLevelWire | string;
  readonly target: string;
  readonly message: string;
  readonly fields?: Readonly<Record<string, unknown>> | undefined;
  readonly line?: number | null | undefined;
  readonly file?: string | null | undefined;
}

/**
 * Normalized UI domain model for reactive log store and presentation.
 */
export interface LogEntry {
  readonly id: string;
  readonly timestamp: string;
  readonly level: LogLevel;
  readonly target: string;
  readonly message: string;
  readonly fields?: Readonly<Record<string, unknown>> | undefined;
  readonly line?: number | undefined;
  readonly file?: string | undefined;
}

/**
 * Payload sent from frontend logging bridge to Tauri IPC command `log_frontend_event`.
 */
export interface FrontendLogPayloadJson {
  readonly level: string;
  readonly target?: string | undefined;
  readonly message: string;
  readonly context?: Readonly<Record<string, unknown>> | undefined;
}

/**
 * Query filter applied to log store ring buffer.
 */
export interface LogFilter {
  readonly level?: LogLevel | 'all' | undefined;
  readonly search?: string | undefined;
  readonly target?: string | undefined;
  readonly startTime?: string | undefined;
  readonly endTime?: string | undefined;
}

/**
 * Check whether a value is a valid lowercase LogLevel.
 */
export function isLogLevel(val: unknown): val is LogLevel {
  return (
    typeof val === 'string' &&
    (val === 'trace' || val === 'debug' || val === 'info' || val === 'warn' || val === 'error')
  );
}

/**
 * Normalize an arbitrary uppercase or lowercase wire level string into LogLevel.
 */
export function toLogLevel(wire: string): LogLevel {
  const lower = wire.toLowerCase();
  if (isLogLevel(lower)) {
    return lower;
  }
  return 'info';
}

/**
 * Type guard validating raw LogEntryJson objects.
 */
export function isLogEntryJson(val: unknown): val is LogEntryJson {
  if (typeof val !== 'object' || val === null) return false;
  const obj = val as Record<string, unknown>;
  return (
    typeof obj['timestamp'] === 'string' &&
    typeof obj['level'] === 'string' &&
    typeof obj['target'] === 'string' &&
    typeof obj['message'] === 'string'
  );
}

let nextIdSequence = 1;

/**
 * Convert a raw LogEntryJson into a domain LogEntry model.
 */
export function toLogEntry(json: LogEntryJson, idOrIndex?: string | number): LogEntry {
  const id =
    idOrIndex !== undefined
      ? `log-${idOrIndex}`
      : `log-${Date.now()}-${nextIdSequence++}`;

  const entry: {
    readonly id: string;
    readonly timestamp: string;
    readonly level: LogLevel;
    readonly target: string;
    readonly message: string;
    readonly fields?: Readonly<Record<string, unknown>> | undefined;
    readonly line?: number | undefined;
    readonly file?: string | undefined;
  } = {
    id,
    timestamp: json.timestamp || new Date().toISOString(),
    level: toLogLevel(json.level),
    target: json.target || 'app',
    message: json.message ?? '',
    ...(json.fields !== undefined ? { fields: json.fields } : {}),
    ...(json.line !== null && json.line !== undefined ? { line: json.line } : {}),
    ...(json.file !== null && json.file !== undefined ? { file: json.file } : {}),
  };

  return entry;
}
