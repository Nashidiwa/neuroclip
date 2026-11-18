/**
 * NeuroClip Subprocess Sidecar IPC Client
 *
 * Provides typed interfaces, runtime type guards, and event listeners
 * for `sidecar:progress` events emitted by `SubprocessSupervisor` (`TASK-M4-03`).
 * Strict adherence to docs/CONVENTIONS.md (zero-any, exactOptionalPropertyTypes).
 */

import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { parseIpcError, type IpcErrorResponse } from './error.ts';

/**
 * Raw wire payload emitted on the `sidecar:progress` Tauri event channel.
 * Accepts canonical snake_case fields from Rust as well as camelCase aliases.
 */
export interface SidecarProgressPayloadJson {
  readonly task_id?: string | undefined;
  readonly taskId?: string | undefined;
  readonly percentage: number;
  readonly current_fps?: number | null | undefined;
  readonly currentFps?: number | null | undefined;
  readonly eta_sec?: number | null | undefined;
  readonly etaSec?: number | null | undefined;
}

/**
 * Normalized Frontend Domain Model for Sidecar subprocess progress telemetry.
 */
export interface SidecarProgressEvent {
  readonly taskId: string;
  readonly percentage: number;
  readonly currentFps: number | null;
  readonly etaSec: number | null;

  // Snake_case aliases matching Rust `SidecarProgressPayload`
  readonly task_id: string;
  readonly current_fps: number | null;
  readonly eta_sec: number | null;
}

function isFiniteNumberOrNullish(val: unknown): boolean {
  if (val === undefined || val === null) return true;
  return typeof val === 'number' && Number.isFinite(val);
}

/**
 * Runtime type guard validating raw `sidecar:progress` payloads.
 */
export function isSidecarProgressPayload(val: unknown): val is SidecarProgressPayloadJson {
  if (typeof val !== 'object' || val === null) return false;
  const obj = val as Record<string, unknown>;

  const rawTaskId = obj['task_id'] !== undefined ? obj['task_id'] : obj['taskId'];
  const hasTaskId = typeof rawTaskId === 'string' && rawTaskId.trim().length > 0;
  const hasPercentage = typeof obj['percentage'] === 'number' && Number.isFinite(obj['percentage']);
  const rawFps = obj['current_fps'] !== undefined ? obj['current_fps'] : obj['currentFps'];
  const rawEta = obj['eta_sec'] !== undefined ? obj['eta_sec'] : obj['etaSec'];
  const validFps = isFiniteNumberOrNullish(rawFps);
  const validEta = isFiniteNumberOrNullish(rawEta);

  return hasTaskId && hasPercentage && validFps && validEta;
}

/**
 * Normalize wire `sidecar:progress` payload into dual camelCase/snake_case domain model.
 */
export function normalizeSidecarProgress(raw: SidecarProgressPayloadJson): SidecarProgressEvent {
  const rawTaskId = raw.task_id !== undefined ? raw.task_id : raw.taskId;
  const taskId = typeof rawTaskId === 'string' ? rawTaskId.trim() : '';
  const rawFps = raw.current_fps !== undefined ? raw.current_fps : raw.currentFps;
  const rawEta = raw.eta_sec !== undefined ? raw.eta_sec : raw.etaSec;
  const currentFps = typeof rawFps === 'number' && Number.isFinite(rawFps) ? rawFps : null;
  const etaSec = typeof rawEta === 'number' && Number.isFinite(rawEta) ? rawEta : null;
  const clampedPercent = Math.min(100, Math.max(0, raw.percentage));

  return {
    taskId,
    percentage: clampedPercent,
    currentFps,
    etaSec,
    task_id: taskId,
    current_fps: currentFps,
    eta_sec: etaSec,
  };
}

/**
 * Normalize an unknown sidecar IPC error into the canonical `IpcErrorResponse` envelope.
 */
export function parseSidecarError(err: unknown): IpcErrorResponse {
  return parseIpcError(err, 'Lỗi tiến trình phụ trợ Sidecar');
}

/**
 * Subscribe to real-time `sidecar:progress` events emitted by the Rust `SubprocessSupervisor`.
 *
 * Gracefully degrades to a no-op unlisten stub in non-Tauri / Vitest environments.
 */
export async function listenSidecarProgress(
  callback: (event: SidecarProgressEvent) => void,
): Promise<UnlistenFn> {
  try {
    return await listen<unknown>('sidecar:progress', (event) => {
      if (isSidecarProgressPayload(event.payload)) {
        callback(normalizeSidecarProgress(event.payload));
      }
    });
  } catch {
    return () => {};
  }
}
