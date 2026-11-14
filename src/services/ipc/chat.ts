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

// ---------------------------------------------------------------------------
// IPC Invoke Wrappers
// ---------------------------------------------------------------------------

/**
 * Trigger online VOD chat download and normalization into Schema 5.3 JSON.
 *
 * @param vodId Unique identifier for the VOD.
 * @param url Full streaming URL (Twitch or YouTube).
 * @returns Path to the normalized `chat_events.json`.
 */
export async function startChatExtraction(
  vodId: string,
  url: string
): Promise<string> {
  try {
    return await invoke<string>('start_chat_extraction', {
      vodId,
      url,
    });
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
  offset?: number
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
  windowSec?: number
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

// ---------------------------------------------------------------------------
// Event Listeners
// ---------------------------------------------------------------------------

/**
 * Subscribe to chat extraction progress events.
 */
export async function listenChatProgress(
  callback: (payload: ChatProgressPayload) => void
): Promise<UnlistenFn> {
  return listen<ChatProgressPayload>('chat:progress', (event) => {
    callback(event.payload);
  });
}

/**
 * Subscribe to ERR-05 chat fallback notifications when chat replay is unavailable.
 */
export async function listenChatFallback(
  callback: (payload: ChatFallbackPayload) => void
): Promise<UnlistenFn> {
  return listen<ChatFallbackPayload>('chat:fallback', (event) => {
    callback(event.payload);
  });
}
