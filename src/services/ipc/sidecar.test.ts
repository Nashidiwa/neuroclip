import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  isSidecarProgressPayload,
  normalizeSidecarProgress,
  parseSidecarError,
  listenSidecarProgress,
  type SidecarProgressEvent,
} from './sidecar.ts';
import { ERROR_CODES } from './error.ts';

const mockListen = vi.fn();

vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: unknown[]) => mockListen(...args),
}));

describe('sidecar IPC client (TASK-M4-03)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('validates and normalizes canonical Rust snake_case payloads', () => {
    const raw = {
      task_id: '  ffmpeg_slice_1 ',
      percentage: 42.5,
      current_fps: 120.4,
      eta_sec: 15,
    };

    expect(isSidecarProgressPayload(raw)).toBe(true);
    expect(normalizeSidecarProgress(raw)).toEqual({
      taskId: 'ffmpeg_slice_1',
      percentage: 42.5,
      currentFps: 120.4,
      etaSec: 15,
      task_id: 'ffmpeg_slice_1',
      current_fps: 120.4,
      eta_sec: 15,
    });

    const withNulls = {
      task_id: 'task_nulls',
      percentage: 12,
      current_fps: null,
      eta_sec: null,
    };
    expect(isSidecarProgressPayload(withNulls)).toBe(true);
    expect(normalizeSidecarProgress(withNulls)).toEqual({
      taskId: 'task_nulls',
      percentage: 12,
      currentFps: null,
      etaSec: null,
      task_id: 'task_nulls',
      current_fps: null,
      eta_sec: null,
    });
  });

  it('validates and normalizes camelCase payloads and clamps percentage to [0, 100]', () => {
    const raw = {
      taskId: 'camel_task',
      percentage: 135,
      currentFps: 59.94,
      etaSec: 0,
    };
    expect(isSidecarProgressPayload(raw)).toBe(true);
    expect(normalizeSidecarProgress(raw)).toEqual({
      taskId: 'camel_task',
      percentage: 100,
      currentFps: 59.94,
      etaSec: 0,
      task_id: 'camel_task',
      current_fps: 59.94,
      eta_sec: 0,
    });

    const negative = { taskId: 'neg_task', percentage: -8 };
    expect(isSidecarProgressPayload(negative)).toBe(true);
    expect(normalizeSidecarProgress(negative).percentage).toBe(0);
  });

  it('enforces identical snake_case-first precedence for mixed-key payloads without crashing', () => {
    const validMixed: Record<string, unknown> = {
      task_id: 'ok_task',
      taskId: 123,
      percentage: 50,
      current_fps: 60,
      currentFps: 'bad',
      eta_sec: 10,
      etaSec: 'bad',
    };

    expect(isSidecarProgressPayload(validMixed)).toBe(true);
    expect(normalizeSidecarProgress(validMixed as never)).toEqual({
      taskId: 'ok_task',
      percentage: 50,
      currentFps: 60,
      etaSec: 10,
      task_id: 'ok_task',
      current_fps: 60,
      eta_sec: 10,
    });

    const invalidMixed = {
      task_id: 123,
      taskId: 'ok_task',
      percentage: 50,
    };
    expect(isSidecarProgressPayload(invalidMixed)).toBe(false);

    const invalidMixedFps = {
      task_id: 'ok_task',
      percentage: 50,
      current_fps: 'bad',
      currentFps: 60,
    };
    expect(isSidecarProgressPayload(invalidMixedFps)).toBe(false);
  });

  it('rejects malformed payloads cleanly', () => {
    expect(isSidecarProgressPayload(null)).toBe(false);
    expect(isSidecarProgressPayload('string')).toBe(false);
    expect(isSidecarProgressPayload({ task_id: '   ', percentage: 10 })).toBe(false);
    expect(isSidecarProgressPayload({ task_id: 't1', percentage: Number.NaN })).toBe(false);
    expect(isSidecarProgressPayload({ task_id: 't1', percentage: Number.POSITIVE_INFINITY })).toBe(
      false,
    );
    expect(isSidecarProgressPayload({ task_id: 't1', percentage: 10, current_fps: '60' })).toBe(
      false,
    );
    expect(isSidecarProgressPayload({ task_id: 't1', percentage: 10, eta_sec: Number.NaN })).toBe(
      false,
    );
  });

  it('parses sidecar IPC errors into canonical IpcErrorResponse', () => {
    const structured = {
      code: ERROR_CODES.SIDECAR_EXECUTION_FAILED,
      message: 'FFmpeg exited with code 1',
      details: 'stderr tail',
      recoverable: false,
    };
    expect(parseSidecarError(structured)).toEqual(structured);

    const parsedFallback = parseSidecarError('raw failure');
    expect(parsedFallback.code).toBe(ERROR_CODES.INTERNAL_ERROR);
    expect(parsedFallback.message).toBe('raw failure');
  });

  it('subscribes to sidecar:progress, forwards valid events, ignores invalid ones, and degrades gracefully', async () => {
    const unlistenSpy = vi.fn();
    let capturedHandler: ((event: { payload: unknown }) => void) | undefined;

    mockListen.mockImplementationOnce(
      async (_eventName: string, handler: (event: { payload: unknown }) => void) => {
        capturedHandler = handler;
        return unlistenSpy;
      },
    );

    const received: SidecarProgressEvent[] = [];
    const unlisten = await listenSidecarProgress((evt) => {
      received.push(evt);
    });

    expect(mockListen).toHaveBeenCalledWith('sidecar:progress', expect.any(Function));
    expect(capturedHandler).toBeDefined();

    capturedHandler?.({ payload: { invalid: true } });
    expect(received).toHaveLength(0);

    capturedHandler?.({
      payload: {
        task_id: 'live_1',
        percentage: 75,
        current_fps: 45,
        eta_sec: 8,
      },
    });
    expect(received).toHaveLength(1);
    expect(received[0]?.taskId).toBe('live_1');

    unlisten();
    expect(unlistenSpy).toHaveBeenCalledTimes(1);

    // Non-Tauri environment fallback when listen throws
    mockListen.mockRejectedValueOnce(new Error('No Tauri IPC'));
    const noopUnlisten = await listenSidecarProgress(() => {});
    expect(() => noopUnlisten()).not.toThrow();
  });

  describe('Managed Sidecars Auto-Updater (TASK-M4-29)', () => {
    it('validates sidecar:status event payloads with runtime type guard', async () => {
      const { isSidecarStatusEventPayload, SIDECAR_STATUS_EVENT } = await import('./updater.ts');
      expect(SIDECAR_STATUS_EVENT).toBe('sidecar:status');

      expect(
        isSidecarStatusEventPayload({
          binary: 'yt-dlp',
          status: 'ready',
          message: 'All good',
        }),
      ).toBe(true);

      expect(
        isSidecarStatusEventPayload({
          binary: 'ffmpeg',
          status: 'updating',
        }),
      ).toBe(true);

      expect(
        isSidecarStatusEventPayload({
          binary: 'yt-dlp',
          status: 'failed',
          message: 'Checksum mismatch',
        }),
      ).toBe(true);

      // Invalid payloads
      expect(isSidecarStatusEventPayload(null)).toBe(false);
      expect(isSidecarStatusEventPayload({})).toBe(false);
      expect(isSidecarStatusEventPayload({ binary: '', status: 'ready' })).toBe(false);
      expect(isSidecarStatusEventPayload({ binary: 'yt-dlp', status: 'unknown_status' })).toBe(
        false,
      );
      expect(isSidecarStatusEventPayload({ binary: 'yt-dlp', status: 'ready', message: 123 })).toBe(
        false,
      );
    });

    it('subscribes to sidecar:status and gracefully degrades in non-Tauri environments', async () => {
      const { listenSidecarStatus } = await import('./updater.ts');

      const unlistenSpy = vi.fn();
      let capturedHandler: ((event: { payload: unknown }) => void) | undefined;

      mockListen.mockImplementationOnce(
        async (eventName: string, handler: (event: { payload: unknown }) => void) => {
          expect(eventName).toBe('sidecar:status');
          capturedHandler = handler;
          return unlistenSpy;
        },
      );

      const received: unknown[] = [];
      const unlisten = await listenSidecarStatus((payload) => {
        received.push(payload);
      });

      expect(mockListen).toHaveBeenCalledWith('sidecar:status', expect.any(Function));

      // Malformed payload ignored
      capturedHandler?.({ payload: { bad: true } });
      expect(received).toHaveLength(0);

      // Valid payload accepted
      capturedHandler?.({
        payload: {
          binary: 'yt-dlp',
          status: 'ready',
          message: 'Update complete',
        },
      });
      expect(received).toHaveLength(1);
      expect(received[0]).toEqual({
        binary: 'yt-dlp',
        status: 'ready',
        message: 'Update complete',
      });

      unlisten();
      expect(unlistenSpy).toHaveBeenCalledTimes(1);

      // Throws gracefully
      mockListen.mockRejectedValueOnce(new Error('No IPC'));
      const fallbackUnlisten = await listenSidecarStatus(() => {});
      expect(() => fallbackUnlisten()).not.toThrow();
    });
  });
});
