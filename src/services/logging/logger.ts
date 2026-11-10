/**
 * Frontend Structured Logging Service & IPC Bridge.
 *
 * Implements:
 * - Leveled logging: trace, debug, info, warn, error
 * - Client-side secret redaction on messages and structured context
 * - In-memory FIFO queue with batching (25 items or 500ms delay)
 * - Immediate flush on error
 * - Safe fallback to colored console in Vitest / browser environments
 * - Zero `any` and strict compliance with `exactOptionalPropertyTypes: true`
 */

import { invoke } from '@tauri-apps/api/core';
import { type FrontendLogPayloadJson, type LogLevel } from './types.ts';
import { sanitizeLogContext, sanitizeLogString } from './redactor.ts';

const BATCH_SIZE_THRESHOLD = 25;
const FLUSH_INTERVAL_MS = 500;

/**
 * Check whether the native Tauri IPC runtime is available in the current environment.
 */
export function isTauriEnvironment(): boolean {
  return (
    typeof window !== 'undefined' &&
    '__TAURI_INTERNALS__' in window &&
    typeof (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ === 'object'
  );
}

/**
 * Console badge color palette corresponding to Neuro-sama VTuber theme.
 */
const CONSOLE_COLORS: Readonly<Record<LogLevel, string>> = {
  trace: '#717590',
  debug: '#87CEEB',
  info: '#98FB98',
  warn: '#FFD700',
  error: '#FF0055',
};

export interface ScopedLogger {
  trace: (message: string, context?: Readonly<Record<string, unknown>>) => void;
  debug: (message: string, context?: Readonly<Record<string, unknown>>) => void;
  info: (message: string, context?: Readonly<Record<string, unknown>>) => void;
  warn: (message: string, context?: Readonly<Record<string, unknown>>) => void;
  error: (message: string, context?: Readonly<Record<string, unknown>>) => void;
  flush: () => Promise<void>;
}

export class LoggerService {
  private queue: FrontendLogPayloadJson[] = [];
  private flushTimer: ReturnType<typeof setTimeout> | null = null;
  private isFlushing = false;
  private isDestroyed = false;

  constructor() {
    if (typeof window !== 'undefined') {
      window.addEventListener('beforeunload', () => {
        void this.flush();
      });
    }
  }

  /**
   * Log an event with the given level, target, message, and structured context.
   */
  public log(
    level: LogLevel,
    target: string,
    message: string,
    context?: Readonly<Record<string, unknown>>,
  ): void {
    if (this.isDestroyed) return;

    // 1. Sanitize sensitive information at origin (Defense-in-Depth)
    const sanitizedMessage = sanitizeLogString(message);
    const sanitizedContext = sanitizeLogContext(context);

    // 2. Console presentation for developers in non-production / test runs
    this.outputConsole(level, target, sanitizedMessage, sanitizedContext);

    // 3. Construct IPC payload
    const payload: {
      readonly level: string;
      readonly target?: string | undefined;
      readonly message: string;
      readonly context?: Readonly<Record<string, unknown>> | undefined;
    } = {
      level,
      target,
      message: sanitizedMessage,
      ...(sanitizedContext !== undefined ? { context: sanitizedContext } : {}),
    };

    this.queue.push(payload);

    // 4. Flush triggers: immediate flush on error or when queue exceeds threshold
    if (level === 'error' || this.queue.length >= BATCH_SIZE_THRESHOLD) {
      void this.flush();
    } else {
      this.scheduleFlush();
    }
  }

  public trace(message: string, context?: Readonly<Record<string, unknown>>): void {
    this.log('trace', 'frontend', message, context);
  }

  public debug(message: string, context?: Readonly<Record<string, unknown>>): void {
    this.log('debug', 'frontend', message, context);
  }

  public info(message: string, context?: Readonly<Record<string, unknown>>): void {
    this.log('info', 'frontend', message, context);
  }

  public warn(message: string, context?: Readonly<Record<string, unknown>>): void {
    this.log('warn', 'frontend', message, context);
  }

  public error(message: string, context?: Readonly<Record<string, unknown>>): void {
    this.log('error', 'frontend', message, context);
  }

  /**
   * Create a scoped logger instance with a dedicated module/target prefix.
   */
  public createLogger(target: string): ScopedLogger {
    return {
      trace: (msg, ctx) => this.log('trace', target, msg, ctx),
      debug: (msg, ctx) => this.log('debug', target, msg, ctx),
      info: (msg, ctx) => this.log('info', target, msg, ctx),
      warn: (msg, ctx) => this.log('warn', target, msg, ctx),
      error: (msg, ctx) => this.log('error', target, msg, ctx),
      flush: () => this.flush(),
    };
  }

  /**
   * Asynchronously flush queued logs to the backend via Tauri IPC.
   */
  public async flush(): Promise<void> {
    if (this.queue.length === 0 || this.isFlushing) {
      return;
    }

    if (this.flushTimer !== null) {
      clearTimeout(this.flushTimer);
      this.flushTimer = null;
    }

    this.isFlushing = true;

    try {
      while (this.queue.length > 0 && !this.isDestroyed) {
        const batch = [...this.queue];
        this.queue = [];

        if (!isTauriEnvironment()) {
          // In browser or mock unit tests without Tauri runtime
          return;
        }

        // Dispatch batch items to IPC
        for (const item of batch) {
          try {
            await invoke('log_frontend_event', {
              level: item.level,
              target: item.target ?? null,
              message: item.message,
              context: item.context ?? null,
            });
          } catch {
            // Silent swallow to prevent unhandled promise rejections during logging failures
          }
        }
      }
    } finally {
      this.isFlushing = false;
      if (this.queue.length > 0 && !this.isDestroyed) {
        if (
          this.queue.length >= BATCH_SIZE_THRESHOLD ||
          this.queue.some((item) => item.level === 'error')
        ) {
          void this.flush();
        } else {
          this.scheduleFlush();
        }
      }
    }
  }

  /**
   * Teardown timer and clear active queue.
   */
  public destroy(): void {
    this.isDestroyed = true;
    if (this.flushTimer !== null) {
      clearTimeout(this.flushTimer);
      this.flushTimer = null;
    }
    this.queue = [];
  }

  /**
   * Internal queue length accessor for unit test assertions.
   */
  public getQueueLength(): number {
    return this.queue.length;
  }

  private scheduleFlush(): void {
    if (this.flushTimer === null) {
      this.flushTimer = setTimeout(() => {
        void this.flush();
      }, FLUSH_INTERVAL_MS);
    }
  }

  private outputConsole(
    level: LogLevel,
    target: string,
    message: string,
    context?: Readonly<Record<string, unknown>>,
  ): void {
    const color = CONSOLE_COLORS[level];
    const prefix = `%c[${level.toUpperCase()}] [${target}]`;
    const style = `color: ${color}; font-weight: bold;`;

    switch (level) {
      case 'trace':
      case 'debug':
        if (context) {
          console.debug(prefix, style, message, context);
        } else {
          console.debug(prefix, style, message);
        }
        break;
      case 'info':
        if (context) {
          console.info(prefix, style, message, context);
        } else {
          console.info(prefix, style, message);
        }
        break;
      case 'warn':
        if (context) {
          console.warn(prefix, style, message, context);
        } else {
          console.warn(prefix, style, message);
        }
        break;
      case 'error':
        if (context) {
          console.error(prefix, style, message, context);
        } else {
          console.error(prefix, style, message);
        }
        break;
    }
  }
}

/**
 * Global singleton logger instance.
 */
export const logger = new LoggerService();
export const createLogger = (target: string): ScopedLogger => logger.createLogger(target);
