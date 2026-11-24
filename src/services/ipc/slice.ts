/**
 * NeuroClip Lossless Precision Range Slicer IPC Client
 *
 * Implements Tauri invoke wrappers for precision video segment slicing and cancellation.
 * Strict adherence to docs/CONVENTIONS.md (zero-any, strict null safety).
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { ERROR_CODES, parseIpcError } from './error.ts';
import {
  type SliceResult,
  type SliceResultJson,
  type SliceVideoSegmentRequest,
  isSliceResultJson,
  normalizeSliceResult,
} from '../../types/range_slicer.ts';

export * from '../../types/range_slicer.ts';

/**
 * Options for slicing video segment when passing parameters individually.
 */
export interface RangeSliceOptions {
  readonly clusterId?: string | null | undefined;
  readonly outputFilename?: string | null | undefined;
  readonly customInputSource?: string | null | undefined;
}

/**
 * Slice a precision video segment from online stream (m3u8) or local video file.
 *
 * Uses stream copy (`-c copy`) without re-encoding, normalizes presentation timestamps
 * to eliminate initial playback stutter, and checks keyframe alignment (ED-13.1 <= 50ms).
 *
 * @param requestOrVodId Request object or unique VOD identifier string.
 * @param startSec Start timestamp in seconds (if passing individual parameters).
 * @param endSec End timestamp in seconds (if passing individual parameters).
 * @param options Additional options (clusterId, outputFilename, customInputSource).
 * @returns Sliced video segment result metadata.
 */
export async function sliceVideoSegment(
  requestOrVodId: SliceVideoSegmentRequest | string,
  startSec?: number,
  endSec?: number,
  options?: RangeSliceOptions,
): Promise<SliceResult> {
  try {
    let vodId: string;
    let start: number;
    let end: number;
    let clusterId: string | null = null;
    let outputFilename: string | null = null;
    let customInputSource: string | null = null;

    if (typeof requestOrVodId === 'string') {
      vodId = requestOrVodId.trim();
      start = startSec ?? 0;
      end = endSec ?? 0;
      clusterId = options?.clusterId ?? null;
      outputFilename = options?.outputFilename ?? null;
      customInputSource = options?.customInputSource ?? null;
    } else {
      vodId = (requestOrVodId.vodId ?? requestOrVodId.vod_id ?? '').trim();
      start = requestOrVodId.startSec ?? requestOrVodId.start_sec ?? 0;
      end = requestOrVodId.endSec ?? requestOrVodId.end_sec ?? 0;
      clusterId = requestOrVodId.clusterId ?? requestOrVodId.cluster_id ?? null;
      outputFilename = requestOrVodId.outputFilename ?? requestOrVodId.output_filename ?? null;
      customInputSource =
        requestOrVodId.customInputSource ?? requestOrVodId.custom_input_source ?? null;
    }

    const raw = await invoke<SliceResultJson>('slice_video_segment', {
      vodId,
      startSec: start,
      endSec: end,
      clusterId,
      outputFilename,
      customInputSource,
    });

    if (!isSliceResultJson(raw)) {
      throw {
        code: ERROR_CODES.INGEST_SLICE_FAILED,
        message: 'Dữ liệu phản hồi cắt phân đoạn không đúng định dạng',
        details: typeof raw === 'object' && raw !== null ? JSON.stringify(raw) : String(raw),
        recoverable: true,
      };
    }

    return normalizeSliceResult(raw);
  } catch (err: unknown) {
    throw parseIpcError(err, 'Lỗi cắt phân đoạn video');
  }
}

/**
 * Cancel a running video slicing task.
 *
 * @param taskId Task identifier registered during slicing.
 * @returns True if the active task was found and cancellation requested.
 */
export async function cancelSliceVideoSegment(taskId: string): Promise<boolean> {
  try {
    const res = await invoke<boolean>('cancel_slice_video_segment', {
      taskId: taskId.trim(),
    });
    return Boolean(res);
  } catch (err: unknown) {
    throw parseIpcError(err, 'Không thể hủy tiến trình cắt video');
  }
}

// ---------------------------------------------------------------------------
// Event Stream Payloads & Listeners
// ---------------------------------------------------------------------------

export interface SliceProgressPayload {
  readonly taskId: string;
  readonly stage: string;
  readonly percent: number;
}

export function isSliceProgressPayload(val: unknown): val is SliceProgressPayload {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  return (
    typeof obj['taskId'] === 'string' &&
    typeof obj['stage'] === 'string' &&
    typeof obj['percent'] === 'number' &&
    Number.isFinite(obj['percent'])
  );
}

/**
 * Subscribe to video slicing progress events.
 *
 * @param callback Handler receiving SliceProgressPayload updates.
 * @returns UnlistenFn to unsubscribe from the 'slice:progress' event.
 */
export async function listenSliceProgress(
  callback: (payload: SliceProgressPayload) => void,
): Promise<UnlistenFn> {
  try {
    return await listen<unknown>('slice:progress', (event) => {
      if (isSliceProgressPayload(event.payload)) {
        callback(event.payload);
      }
    });
  } catch (err: unknown) {
    throw parseIpcError(err, 'Lỗi lắng nghe tiến trình cắt video');
  }
}
