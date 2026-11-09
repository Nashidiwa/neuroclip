import { describe, it, expect } from 'vitest';
import {
  isIpcError,
  parseIpcError,
  getFriendlyErrorMessage,
  getVietnameseMessage,
  isRecoverableError,
  IpcErrorResponse,
  ERROR_CODES,
  ERROR_MESSAGES_VI,
} from './error';

describe('TASK-M4-02: Unified IPC Error Service (IT-02.1)', () => {
  describe('isIpcError type guard', () => {
    it('should return true for valid IpcErrorResponse objects', () => {
      const errorWithNullDetails: IpcErrorResponse = {
        code: ERROR_CODES.INGEST_DISK_FULL,
        message: 'Dung lượng ổ đĩa không đủ',
        details: null,
        recoverable: true,
      };
      expect(isIpcError(errorWithNullDetails)).toBe(true);

      const errorWithStringDetails: IpcErrorResponse = {
        code: ERROR_CODES.STT_PAYLOAD_TOO_LARGE,
        message: 'Payload quá lớn',
        details: '28.5MB exceeds 25MB limit',
        recoverable: false,
      };
      expect(isIpcError(errorWithStringDetails)).toBe(true);

      const errorWithUndefinedDetails: IpcErrorResponse = {
        code: ERROR_CODES.INTERNAL_ERROR,
        message: 'Lỗi nội bộ',
        details: undefined,
        recoverable: false,
      };
      expect(isIpcError(errorWithUndefinedDetails)).toBe(true);
    });

    it('should return false for primitives and nullish values', () => {
      expect(isIpcError(null)).toBe(false);
      expect(isIpcError(undefined)).toBe(false);
      expect(isIpcError('')).toBe(false);
      expect(isIpcError('ERR_INTERNAL_ERROR')).toBe(false);
      expect(isIpcError(12345)).toBe(false);
      expect(isIpcError(true)).toBe(false);
      expect(isIpcError([])).toBe(false);
    });

    it('should return false for malformed or incomplete objects', () => {
      expect(isIpcError({})).toBe(false);
      expect(isIpcError({ code: 'ERR_INGEST_DISK_FULL' })).toBe(false);
      expect(isIpcError({ code: 'ERR_INGEST_DISK_FULL', message: 'Test' })).toBe(false);
      expect(
        isIpcError({
          code: 123,
          message: 'Test',
          recoverable: true,
        }),
      ).toBe(false);
      expect(
        isIpcError({
          code: 'ERR_TEST',
          message: 'Test',
          recoverable: 'yes',
        }),
      ).toBe(false);
      expect(
        isIpcError({
          code: 'ERR_TEST',
          message: 'Test',
          recoverable: true,
          details: 999, // details must be string, null, or undefined
        }),
      ).toBe(false);
    });
  });

  describe('parseIpcError utility', () => {
    it('should return valid IpcErrorResponse directly without modification', () => {
      const valid: IpcErrorResponse = {
        code: ERROR_CODES.INGEST_NETWORK_TIMEOUT,
        message: 'Mất kết nối mạng',
        details: 'Connection reset by peer',
        recoverable: true,
      };
      expect(parseIpcError(valid)).toEqual(valid);
    });

    it('should parse JSON stringified IpcErrorResponse from Tauri invoke rejections', () => {
      const payload: IpcErrorResponse = {
        code: ERROR_CODES.INGEST_BOT_BLOCKED,
        message: 'YouTube/Twitch đã kích hoạt cơ chế chặn bot',
        details: 'HTTP 403 Forbidden',
        recoverable: true,
      };
      const jsonStr = JSON.stringify(payload);
      const parsed = parseIpcError(jsonStr);

      expect(parsed).toEqual(payload);
    });

    it('should parse JavaScript Error instances', () => {
      const jsErr = new Error('Database connection failed');
      const parsed = parseIpcError(jsErr);

      expect(parsed.code).toBe(ERROR_CODES.INTERNAL_ERROR);
      expect(parsed.message).toBe('Database connection failed');
      expect(parsed.recoverable).toBe(false);
      expect(typeof parsed.details).toBe('string');
      expect(parsed.details).toContain('Error: Database connection failed');
    });

    it('should parse Error instances containing JSON stringified payload', () => {
      const innerPayload: IpcErrorResponse = {
        code: ERROR_CODES.TRANSLATE_SAFETY_FILTER,
        message: 'Bộ lọc an toàn kích hoạt',
        details: 'SAFETY',
        recoverable: true,
      };
      const wrappedErr = new Error(JSON.stringify(innerPayload));
      const parsed = parseIpcError(wrappedErr);

      expect(parsed).toEqual(innerPayload);
    });

    it('should parse arbitrary plain strings', () => {
      const parsed = parseIpcError('Unknown crash');
      expect(parsed.code).toBe(ERROR_CODES.INTERNAL_ERROR);
      expect(parsed.message).toBe('Unknown crash');
      expect(parsed.details).toBeNull();
      expect(parsed.recoverable).toBe(false);
    });

    it('should parse generic objects with partial fields', () => {
      const partial = {
        code: ERROR_CODES.EXPORT_CAPCUT_NOT_FOUND,
        message: 'Không tìm thấy CapCut',
        details: 'Path C:/CapCut not found',
        recoverable: true,
      };
      const parsed = parseIpcError(partial);

      expect(parsed.code).toBe(ERROR_CODES.EXPORT_CAPCUT_NOT_FOUND);
      expect(parsed.message).toBe('Không tìm thấy CapCut');
      expect(parsed.details).toBe('Path C:/CapCut not found');
      expect(parsed.recoverable).toBe(true);
    });

    it('should apply fallbackMessage when input is empty or unknown', () => {
      const parsed = parseIpcError(null, 'Tác vụ gặp sự cố');
      expect(parsed.code).toBe(ERROR_CODES.INTERNAL_ERROR);
      expect(parsed.message).toBe('Tác vụ gặp sự cố');
      expect(parsed.recoverable).toBe(false);
    });
  });

  describe('getFriendlyErrorMessage & getVietnameseMessage', () => {
    it('should return Vietnamese instructions for known error codes', () => {
      expect(getFriendlyErrorMessage(ERROR_CODES.INGEST_DISK_FULL)).toContain('ổ đĩa');
      expect(getFriendlyErrorMessage(ERROR_CODES.TRANSLATE_SAFETY_FILTER)).toContain('an toàn');
      expect(getFriendlyErrorMessage(ERROR_CODES.EXPORT_CAPCUT_NOT_FOUND)).toContain('CapCut');
      expect(getFriendlyErrorMessage(ERROR_CODES.STT_RATE_LIMIT)).toContain('Rate Limit');
    });

    it('should return identical results via getVietnameseMessage alias', () => {
      expect(getVietnameseMessage(ERROR_CODES.INGEST_DISK_FULL)).toBe(
        getFriendlyErrorMessage(ERROR_CODES.INGEST_DISK_FULL),
      );
    });

    it('should extract friendly message from IpcErrorResponse objects', () => {
      const errObj: IpcErrorResponse = {
        code: ERROR_CODES.INGEST_BOT_BLOCKED,
        message: 'Server blocked bot',
        details: null,
        recoverable: true,
      };
      expect(getFriendlyErrorMessage(errObj)).toContain('chặn bot');
    });

    it('should fallback to errorObj.message if code is not in lookup table', () => {
      const customErr: IpcErrorResponse = {
        code: 'ERR_CUSTOM_PLUGIN_ERROR',
        message: 'Lỗi từ plugin ngoài',
        details: null,
        recoverable: false,
      };
      expect(getFriendlyErrorMessage(customErr)).toBe('Lỗi từ plugin ngoài');
    });

    it('should fallback to default Vietnamese phrase for unrecognized inputs', () => {
      expect(getFriendlyErrorMessage(12345)).toBe('Đã xảy ra lỗi không xác định.');
    });
  });

  describe('isRecoverableError', () => {
    it('should return true for recoverable IpcErrorResponse', () => {
      const recoverableErr: IpcErrorResponse = {
        code: ERROR_CODES.INGEST_NETWORK_TIMEOUT,
        message: 'Timeout',
        details: null,
        recoverable: true,
      };
      expect(isRecoverableError(recoverableErr)).toBe(true);
    });

    it('should return false for unrecoverable IpcErrorResponse', () => {
      const unrecoverableErr: IpcErrorResponse = {
        code: ERROR_CODES.INTERNAL_ERROR,
        message: 'Fatal',
        details: null,
        recoverable: false,
      };
      expect(isRecoverableError(unrecoverableErr)).toBe(false);
    });

    it('should return false for arbitrary non-IpcErrorResponse values', () => {
      expect(isRecoverableError('some error')).toBe(false);
      expect(isRecoverableError(new Error('fail'))).toBe(false);
      expect(isRecoverableError(null)).toBe(false);
    });
  });

  describe('ERROR_CODES catalog invariants', () => {
    it('should have all error codes prefixed with ERR_', () => {
      const codes = Object.values(ERROR_CODES);
      for (const code of codes) {
        expect(code.startsWith('ERR_')).toBe(true);
      }
    });

    it('should have Vietnamese messages defined for canonical codes', () => {
      expect(ERROR_MESSAGES_VI[ERROR_CODES.INGEST_DISK_FULL]).toBeDefined();
      expect(ERROR_MESSAGES_VI[ERROR_CODES.INGEST_BOT_BLOCKED]).toBeDefined();
      expect(ERROR_MESSAGES_VI[ERROR_CODES.STT_VRAM_INSUFFICIENT]).toBeDefined();
      expect(ERROR_MESSAGES_VI[ERROR_CODES.TRANSLATE_SAFETY_FILTER]).toBeDefined();
      expect(ERROR_MESSAGES_VI[ERROR_CODES.EXPORT_CAPCUT_NOT_FOUND]).toBeDefined();
    });
  });
});
