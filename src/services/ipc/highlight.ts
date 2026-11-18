/**
 * NeuroClip Multimodal Highlight Detection & Clustering IPC Client
 *
 * Implements Schema 5.4 contracts, runtime type guards, and Tauri IPC invoke wrappers.
 * Strict adherence to docs/CONVENTIONS.md (zero-any, strict null safety).
 */

import { invoke } from '@tauri-apps/api/core';
import { parseIpcError } from './error.ts';
import {
  type HighlightCluster,
  type HighlightClusterJson,
  type HighlightClustersContainer,
  type HighlightClustersContainerJson,
  normalizeHighlightCluster,
  normalizeHighlightClustersContainer,
} from '../../types/highlight.ts';

export * from '../../types/highlight.ts';

/**
 * Trigger multimodal fusion and highlight clustering for a VOD.
 *
 * @param vodId Unique identifier for the VOD.
 * @param durationSeconds Total duration of the VOD stream in seconds.
 * @param options Optional override file paths for chat events and audio WAV.
 * @returns Ranked HighlightClustersContainer adhering to Schema 5.4.
 */
export async function detectHighlights(
  vodId: string,
  durationSeconds: number,
  options?: {
    chatEventsPath?: string | undefined;
    audioPath?: string | undefined;
  },
): Promise<HighlightClustersContainer> {
  try {
    const raw = await invoke<HighlightClustersContainerJson>('detect_highlights', {
      vodId,
      durationSeconds,
      chatEventsPath: options?.chatEventsPath ?? null,
      audioPath: options?.audioPath ?? null,
    });
    return normalizeHighlightClustersContainer(raw);
  } catch (err: unknown) {
    throw parseIpcError(err);
  }
}

/**
 * Retrieve saved highlight clusters from `highlight_clusters.json` for a VOD.
 *
 * @param vodId Unique identifier for the VOD.
 * @returns HighlightClustersContainer adhering to Schema 5.4.
 */
export async function getHighlightClusters(vodId: string): Promise<HighlightClustersContainer> {
  try {
    const raw = await invoke<HighlightClustersContainerJson>('get_highlight_clusters', {
      vodId,
    });
    return normalizeHighlightClustersContainer(raw);
  } catch (err: unknown) {
    throw parseIpcError(err);
  }
}

/**
 * Toggle or set bookmark status on a specific highlight cluster.
 *
 * @param vodId Unique identifier for the VOD.
 * @param clusterId Target cluster ID (e.g. "cluster_1").
 * @param bookmarked Optional explicit boolean bookmark state. If omitted, toggles state.
 * @returns Updated HighlightCluster.
 */
export async function toggleHighlightBookmark(
  vodId: string,
  clusterId: string,
  bookmarked?: boolean | undefined,
): Promise<HighlightCluster> {
  try {
    const raw = await invoke<HighlightClusterJson>('toggle_highlight_bookmark', {
      vodId,
      clusterId,
      bookmarked: bookmarked ?? null,
    });
    return normalizeHighlightCluster(raw);
  } catch (err: unknown) {
    throw parseIpcError(err);
  }
}

/**
 * Alias wrapper for toggleHighlightBookmark.
 */
export async function toggleClusterBookmark(
  vodId: string,
  clusterId: string,
  bookmarked?: boolean | undefined,
): Promise<HighlightCluster> {
  return toggleHighlightBookmark(vodId, clusterId, bookmarked);
}
