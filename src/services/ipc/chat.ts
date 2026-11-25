/**
 * NeuroClip Chat Replay Extraction & Heatmap IPC Client
 *
 * Implements Schema 5.3 contracts, runtime type guards, and Tauri IPC invoke wrappers.
 * Strict adherence to docs/CONVENTIONS.md (zero-any, strict null safety).
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { parseIpcError } from './error.ts';
import {
  type ChatEventsContainer,
  type ChatEventsContainerJson,
  type ChatHeatmapResponse,
  type ChatHeatmapResponseJson,
  normalizeChatEventsContainer,
  normalizeChatHeatmapResponse,
} from '../../types/chat.ts';

export * from '../../types/chat.ts';

// ---------------------------------------------------------------------------
// Event Stream Payloads
// ---------------------------------------------------------------------------

export interface ChatProgressPayload {
  readonly taskId: string;
  readonly stage: string;
  readonly percent: number;
  readonly messagesExtracted: number;
  readonly currentOffsetSec: number;
}

export interface ChatFallbackPayload {
  readonly taskId: string;
  readonly code: string;
  readonly message: string;
  readonly audioFallbackActive: boolean;
}

export function isChatProgressPayload(val: unknown): val is ChatProgressPayload {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  return (
    typeof obj['taskId'] === 'string' &&
    typeof obj['stage'] === 'string' &&
    typeof obj['percent'] === 'number' &&
    Number.isFinite(obj['percent']) &&
    typeof obj['messagesExtracted'] === 'number' &&
    Number.isFinite(obj['messagesExtracted']) &&
    typeof obj['currentOffsetSec'] === 'number' &&
    Number.isFinite(obj['currentOffsetSec'])
  );
}

export function isChatFallbackPayload(val: unknown): val is ChatFallbackPayload {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  return (
    typeof obj['taskId'] === 'string' &&
    typeof obj['code'] === 'string' &&
    typeof obj['message'] === 'string' &&
    typeof obj['audioFallbackActive'] === 'boolean'
  );
}

// ---------------------------------------------------------------------------
// IPC Invoke Wrappers
// ---------------------------------------------------------------------------

export interface ChatExtractionOptions {
  readonly cookiesPath?: string | null | undefined;
  readonly browserCookies?: string | null | undefined;
}

/**
 * Trigger online VOD chat download and normalization into Schema 5.3 JSON.
 *
 * @param vodId Unique identifier for the VOD.
 * @param url Full streaming URL (Twitch or YouTube).
 * @param options Optional cookie configuration passed through to YouTube replay extraction.
 * @returns Path to the normalized `chat_events.json`.
 */
export async function startChatExtraction(
  vodId: string,
  url: string,
  options?: ChatExtractionOptions,
): Promise<string> {
  const payload: {
    vodId: string;
    url: string;
    cookiesPath?: string;
    browserCookies?: string;
  } = {
    vodId,
    url,
  };

  if (typeof options?.cookiesPath === 'string' && options.cookiesPath.trim().length > 0) {
    payload.cookiesPath = options.cookiesPath.trim();
  }
  if (typeof options?.browserCookies === 'string' && options.browserCookies.trim().length > 0) {
    payload.browserCookies = options.browserCookies.trim();
  }

  try {
    return await invoke<string>('start_chat_extraction', payload);
  } catch (err: unknown) {
    throw parseIpcError(err);
  }
}

/**
 * Retrieve paginated normalized chat events for a VOD.
 *
 * @param vodId Unique identifier for the VOD.
 * @param limit Optional maximum number of events to return (default 500).
 * @param offset Optional starting index (default 0).
 * @returns ChatEventsContainer with domain accessors.
 */
export async function getChatEvents(
  vodId: string,
  limit?: number,
  offset?: number,
): Promise<ChatEventsContainer> {
  try {
    const raw = await invoke<ChatEventsContainerJson>('get_chat_events', {
      vodId,
      limit: limit ?? null,
      offset: offset ?? null,
    });
    return normalizeChatEventsContainer(raw);
  } catch (err: unknown) {
    throw parseIpcError(err);
  }
}

/**
 * Retrieve 10-second sliding window velocity heatmap analytics for a VOD.
 *
 * @param vodId Unique identifier for the VOD.
 * @param windowSec Optional window duration in seconds (default 10.0).
 * @returns ChatHeatmapResponse with domain accessors.
 */
export async function getChatVelocityHeatmap(
  vodId: string,
  windowSec?: number,
): Promise<ChatHeatmapResponse> {
  try {
    const raw = await invoke<ChatHeatmapResponseJson>('get_chat_velocity_heatmap', {
      vodId,
      windowSec: windowSec ?? null,
    });
    return normalizeChatHeatmapResponse(raw);
  } catch (err: unknown) {
    throw parseIpcError(err);
  }
}

/**
 * Cancel a running chat extraction task.
 *
 * @param taskId Unique identifier or VOD ID of the active chat extraction task.
 * @returns True if active task was found and cancelled, false otherwise.
 */
export async function cancelChatExtraction(taskId: string): Promise<boolean> {
  try {
    return await invoke<boolean>('cancel_chat_extraction', {
      taskId: taskId.trim(),
    });
  } catch (err: unknown) {
    throw parseIpcError(err, 'Không thể hủy tiến trình trích xuất chat');
  }
}

// ---------------------------------------------------------------------------
// Event Listeners
// ---------------------------------------------------------------------------

/**
 * Subscribe to chat extraction progress events.
 */
export async function listenChatProgress(
  callback: (payload: ChatProgressPayload) => void,
): Promise<UnlistenFn> {
  try {
    return await listen<unknown>('chat:progress', (event) => {
      if (isChatProgressPayload(event.payload)) {
        callback(event.payload);
      }
    });
  } catch {
    return () => {};
  }
}

/**
 * Subscribe to ERR-05 chat fallback notifications when chat replay is unavailable.
 */
export async function listenChatFallback(
  callback: (payload: ChatFallbackPayload) => void,
): Promise<UnlistenFn> {
  try {
    return await listen<unknown>('chat:fallback', (event) => {
      if (isChatFallbackPayload(event.payload)) {
        callback(event.payload);
      }
    });
  } catch {
    return () => {};
  }
}
