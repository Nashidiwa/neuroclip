import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  redactSecret,
  sanitizeLogString,
  sanitizeLogContext,
} from '../services/logging/redactor.ts';
import { LoggerService } from '../services/logging/logger.ts';

describe('Logger Redaction & Frontend Bridge Subsystem', () => {
  describe('UT-04.1 & UT-04.2: Canonical Secret Redaction Formula', () => {
    it('UT-04.1 (Happy Path): should redact key > 8 chars preserving 4-prefix and 4-suffix', () => {
      const raw = 'gsk_1234567890abcdef12345678';
      const redacted = redactSecret(raw);
      expect(redacted).toBe('gsk_[REDACTED]5678');
      expect(redacted.startsWith('gsk_')).toBe(true);
      expect(redacted.endsWith('5678')).toBe(true);
      expect(redacted.includes('1234567890abcdef')).toBe(false);
    });

    it('UT-04.2 (Edge Case): should mask keys of length <= 8 completely as [REDACTED]', () => {
      expect(redactSecret('')).toBe('[REDACTED]');
      expect(redactSecret('a')).toBe('[REDACTED]');
      expect(redactSecret('1234567')).toBe('[REDACTED]');
      expect(redactSecret('12345678')).toBe('[REDACTED]');
    });

    it('should correctly handle boundary length of 9 characters', () => {
      expect(redactSecret('123456789')).toBe('1234[REDACTED]6789');
    });
  });

  describe('UT-04.3: Multi-Provider Secret Scanning & Sanitization', () => {
    it('should redact Groq API keys (gsk_*)', () => {
      const msg = 'Loaded Groq engine with gsk_abcdef1234567890fedcba config';
      const sanitized = sanitizeLogString(msg);
      expect(sanitized).not.toContain('gsk_abcdef1234567890fedcba');
      expect(sanitized).toContain('gsk_[REDACTED]dcba');
    });

    it('should redact Google Gemini API keys (AIza*)', () => {
      const msg = 'Gemini auth: AIzaSyD987654321abcdef1234567890 verified';
      const sanitized = sanitizeLogString(msg);
      expect(sanitized).not.toContain('AIzaSyD987654321abcdef1234567890');
      expect(sanitized).toContain('AIza[REDACTED]7890');
    });

    it('should redact OpenAI API keys (sk-*) and project keys (sk-proj-*)', () => {
      const msg = 'OpenAI model with sk-proj-abcdefghijklmnopqrstuvwxyz123456 active';
      const sanitized = sanitizeLogString(msg);
      expect(sanitized).not.toContain('sk-proj-abcdefghijklmnopqrstuvwxyz123456');
      expect(sanitized).toContain('sk-p[REDACTED]3456');
    });

    it('should redact Twitch Auth Tokens (auth-token=*)', () => {
      const msg = 'Twitch cookie auth-token=1234567890abcdef1234 detected';
      const sanitized = sanitizeLogString(msg);
      expect(sanitized).not.toContain('1234567890abcdef1234');
      expect(sanitized).toContain('auth-token=[REDACTED]');
    });

    it('should redact Twitch OAuth tokens (oauth:*)', () => {
      const msg = 'Connecting with oauth:1234567890abcdef1234 token';
      const sanitized = sanitizeLogString(msg);
      expect(sanitized).not.toContain('1234567890abcdef1234');
      expect(sanitized).toContain('oaut[REDACTED]1234');
    });

    it('should redact Bearer authorization tokens', () => {
      const msg = 'Header: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6';
      const sanitized = sanitizeLogString(msg);
      expect(sanitized).not.toContain('eyJhbGciOiJIUzI1NiIsInR5cCI6');
      expect(sanitized).toBe('Header: Bearer [REDACTED]');
    });

    it('should sanitize multiple sensitive keys embedded within a single string', () => {
      const msg =
        'Keys: groq=gsk_1234567890abcdef12345678 gemini=AIzaSyD987654321abcdef1234567890';
      const sanitized = sanitizeLogString(msg);
      expect(sanitized).toContain('gsk_[REDACTED]5678');
      expect(sanitized).toContain('AIza[REDACTED]7890');
      expect(sanitized).not.toContain('gsk_1234567890abcdef12345678');
      expect(sanitized).not.toContain('AIzaSyD987654321abcdef1234567890');
    });

    it('should leave strings without secrets untouched', () => {
      const msg = 'Normal diagnostic log: pipeline completed in 245ms';
      expect(sanitizeLogString(msg)).toBe(msg);
    });
  });

  describe('Structured Context Sanitization (sanitizeLogContext)', () => {
    it('should recursively sanitize nested context dictionaries and arrays', () => {
      const context: Readonly<Record<string, unknown>> = {
        apiKey: 'gsk_1234567890abcdef12345678',
        vodId: '12345',
        meta: {
          subToken: 'AIzaSyD987654321abcdef1234567890',
          tokens: ['sk-proj-abcdefghijklmnopqrstuvwxyz123456', 'plain-text'],
        },
        count: 42,
        active: true,
      };

      const sanitized = sanitizeLogContext(context);
      expect(sanitized).toBeDefined();
      if (!sanitized) return;

      expect(sanitized['apiKey']).toBe('gsk_[REDACTED]5678');
      expect(sanitized['vodId']).toBe('12345');
      expect(sanitized['count']).toBe(42);
      expect(sanitized['active']).toBe(true);

      const meta = sanitized['meta'] as Record<string, unknown>;
      expect(meta['subToken']).toBe('AIza[REDACTED]7890');

      const tokens = meta['tokens'] as unknown[];
      expect(tokens[0]).toBe('sk-p[REDACTED]3456');
      expect(tokens[1]).toBe('plain-text');
    });

    it('should return undefined when passed undefined context', () => {
      expect(sanitizeLogContext(undefined)).toBeUndefined();
    });
  });

  describe('LoggerService FIFO Buffering & Fallback', () => {
    let service: LoggerService;

    beforeEach(() => {
      vi.useFakeTimers();
      service = new LoggerService();
    });

    afterEach(() => {
      service.destroy();
      vi.useRealTimers();
    });

    it('should accumulate logs in memory queue below batch threshold', () => {
      for (let i = 0; i < 24; i++) {
        service.info(`Message ${i}`);
      }
      expect(service.getQueueLength()).toBe(24);
    });

    it('should trigger flush when batch threshold (25 items) is reached', async () => {
      for (let i = 0; i < 24; i++) {
        service.info(`Message ${i}`);
      }
      expect(service.getQueueLength()).toBe(24);

      // 25th item triggers immediate flush
      service.info('Message 25');
      await vi.runAllTimersAsync();
      expect(service.getQueueLength()).toBe(0);
    });

    it('should trigger immediate flush on error level events regardless of queue size', async () => {
      service.info('Preliminary info message');
      expect(service.getQueueLength()).toBe(1);

      service.error('Critical storage failure');
      await vi.runAllTimersAsync();
      expect(service.getQueueLength()).toBe(0);
    });

    it('should flush pending queue on debounce timer expiration (500ms)', async () => {
      service.info('Pending debounced message');
      expect(service.getQueueLength()).toBe(1);

      await vi.advanceTimersByTimeAsync(400);
      expect(service.getQueueLength()).toBe(1);

      await vi.advanceTimersByTimeAsync(100);
      expect(service.getQueueLength()).toBe(0);
    });

    it('should support scoped loggers with dedicated target module', () => {
      const ingestionLogger = service.createLogger('neuroclip::ingestion');
      ingestionLogger.info('VOD download started');
      expect(service.getQueueLength()).toBe(1);
    });
  });
});
