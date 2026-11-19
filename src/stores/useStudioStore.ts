/**
 * NeuroClip Subtitle Studio State Store
 *
 * Reactive store managing video playback state, timeline synchronization,
 * 9:16 vertical presets ('center-crop' & 'two-stack'), and O(log N + K)
 * multi-segment dialogue lookup for overlapping speech and banter.
 *
 * Built on shared `./createStore.ts` (`useSyncExternalStore` with Zustand-compatible API).
 * Strict adherence to docs/CONVENTIONS.md (TypeScript strict mode, zero any, strict null safety).
 */

import { createStore, type StoreHook } from './createStore.ts';
import type { TranscriptSegment, SpeakerRole } from '../types/transcript.ts';

export type Preset916 = 'center-crop' | 'two-stack';
export type AspectRatioMode = '9:16' | '16:9';

// ---------------------------------------------------------------------------
// Single Source of Truth for Character Palette (PO Option 1)
// ---------------------------------------------------------------------------

export const SPEAKER_COLORS: Readonly<Record<SpeakerRole, string>> = Object.freeze({
  vedal: '#98FB98',
  neuro: '#FFB6C1',
  evil: '#FF0055',
  guest: '#FFD700',
  chat: '#87CEEB',
  unknown: '#FFFFFF',
});

/**
 * Maps a speaker role string to its official PO Option 1 hex color.
 * Defaults safely to '#FFFFFF' for unrecognized or missing roles.
 */
export function getSpeakerColor(role: string | null | undefined): string {
  if (!role) return SPEAKER_COLORS.unknown;
  const normalized = role.toLowerCase().trim() as SpeakerRole;
  return SPEAKER_COLORS[normalized] ?? SPEAKER_COLORS.unknown;
}

// ---------------------------------------------------------------------------
// Timecode Formatting Helper
// ---------------------------------------------------------------------------

/**
 * Formats a time offset in seconds into sub-second millisecond timecode:
 * `MM:SS.mmm` or `HH:MM:SS.mmm` (if duration >= 1 hour or forceHours = true).
 * Safely handles NaN, negative, and infinite values by clamping to '00:00.000'.
 */
export function formatStudioTimecode(seconds: number, forceHours = false): string {
  if (!Number.isFinite(seconds) || seconds < 0) {
    return forceHours ? '00:00:00.000' : '00:00.000';
  }

  const totalMilliseconds = Math.floor(seconds * 1000);
  const ms = totalMilliseconds % 1000;
  const totalSeconds = Math.floor(totalMilliseconds / 1000);
  const s = totalSeconds % 60;
  const totalMinutes = Math.floor(totalSeconds / 60);
  const m = totalMinutes % 60;
  const h = Math.floor(totalMinutes / 60);

  const msStr = ms.toString().padStart(3, '0');
  const sStr = s.toString().padStart(2, '0');
  const mStr = m.toString().padStart(2, '0');

  if (forceHours || h > 0) {
    const hStr = h.toString().padStart(2, '0');
    return `${hStr}:${mStr}:${sStr}.${msStr}`;
  }

  return `${mStr}:${sStr}.${msStr}`;
}

// ---------------------------------------------------------------------------
// Multi-Segment Binary Search Algorithm: O(log N + K)
// ---------------------------------------------------------------------------

/**
 * Optimized O(log N + K) binary search and bounded bidirectional neighborhood scan
 * for active subtitle segments at a given playback timecode.
 *
 * Fully supports overlapping banter and simultaneous speakers (e.g. Vedal & Neuro).
 *
 * @param segments Pre-sorted (or unsorted) array of segments.
 * @param timeSec Playback timecode in seconds.
 * @returns Array of all segments active at timeSec (startSec <= timeSec <= endSec),
 *          sorted chronologically by startSec then endSec.
 */
export function findActiveSegments(
  segments: readonly TranscriptSegment[],
  timeSec: number,
): TranscriptSegment[] {
  if (segments.length === 0 || !Number.isFinite(timeSec) || timeSec < 0) {
    return [];
  }

  // 1. Binary search to find the latest starting segment whose startSec <= timeSec
  let low = 0;
  let high = segments.length - 1;
  let candidateIdx = -1;

  while (low <= high) {
    const mid = (low + high) >> 1;
    const seg = segments[mid];
    if (!seg) break;
    const segStart = seg.startSec ?? seg.start_sec;

    if (segStart <= timeSec) {
      candidateIdx = mid;
      low = mid + 1; // probe right to find the latest segment with start <= timeSec
    } else {
      high = mid - 1;
    }
  }

  const active: TranscriptSegment[] = [];
  const visited = new Set<string>();
  const startScan = candidateIdx >= 0 ? candidateIdx : 0;
  const MAX_LOOKBACK_SEC = 60.0; // Dialogue safety horizon in seconds

  // 2. Scan backwards from candidateIdx to collect earlier segments that still span timeSec
  for (let i = startScan; i >= 0; i--) {
    const seg = segments[i];
    if (!seg) continue;
    const segStart = seg.startSec ?? seg.start_sec;
    const segEnd = seg.endSec ?? seg.end_sec;

    if (timeSec - segStart > MAX_LOOKBACK_SEC) {
      break; // Safe boundary cutoff
    }

    if (segStart <= timeSec && segEnd >= timeSec) {
      const id = seg.segmentId ?? seg.segment_id;
      if (!visited.has(id)) {
        visited.add(id);
        active.push(seg);
      }
    }
  }

  // 3. Scan forwards from candidateIdx + 1 in case adjacent segments share the exact same startSec
  for (let i = startScan + 1; i < segments.length; i++) {
    const seg = segments[i];
    if (!seg) continue;
    const segStart = seg.startSec ?? seg.start_sec;
    const segEnd = seg.endSec ?? seg.end_sec;

    if (segStart > timeSec) {
      break; // Since segments are sorted by startSec, future segments cannot be active
    }

    if (segStart <= timeSec && segEnd >= timeSec) {
      const id = seg.segmentId ?? seg.segment_id;
      if (!visited.has(id)) {
        visited.add(id);
        active.push(seg);
      }
    }
  }

  // Preserve deterministic ascending chronological order
  return active.sort((a, b) => {
    const aStart = a.startSec ?? a.start_sec;
    const bStart = b.startSec ?? b.start_sec;
    if (aStart !== bStart) return aStart - bStart;
    const aEnd = a.endSec ?? a.end_sec;
    const bEnd = b.endSec ?? b.end_sec;
    return aEnd - bEnd;
  });
}

// ---------------------------------------------------------------------------
// Store Interface Contract
// ---------------------------------------------------------------------------

export interface StudioState {
  // Video & Playback State
  readonly videoSrc: string | null;
  readonly videoPath: string | null;
  readonly currentTime: number;
  readonly duration: number;
  readonly isPlaying: boolean;
  readonly playbackRate: number;
  readonly volume: number; // Clamped [0, 1]
  readonly isMuted: boolean;
  readonly isScrubbing: boolean;
  readonly seekTargetTime: number | null;

  // Presets & Aspect Ratio
  readonly preset916: Preset916;
  readonly aspectRatio: AspectRatioMode;

  // Subtitle Data & Synchronization
  readonly segments: readonly TranscriptSegment[];
  readonly activeSegmentIds: readonly string[];
  readonly activeSegments: readonly TranscriptSegment[];
  readonly selectedSegmentId: string | null;

  // Actions
  readonly setVideoSrc: (src: string | null, videoPath?: string | null) => void;
  readonly setCurrentTime: (time: number) => void;
  readonly setDuration: (duration: number) => void;
  readonly setIsPlaying: (isPlaying: boolean) => void;
  readonly togglePlay: () => void;
  readonly setPlaybackRate: (rate: number) => void;
  readonly setVolume: (volume: number) => void;
  readonly setIsMuted: (isMuted: boolean) => void;
  readonly toggleMute: () => void;
  readonly setPreset916: (preset: Preset916) => void;
  readonly setAspectRatio: (ratio: AspectRatioMode) => void;
  readonly setSegments: (segments: readonly TranscriptSegment[]) => void;
  readonly selectSegment: (segmentId: string | null) => void;
  readonly seekTo: (time: number) => void;
  readonly clearSeekTarget: () => void;
  readonly setIsScrubbing: (isScrubbing: boolean) => void;
  readonly getActiveSegments: () => readonly TranscriptSegment[];
  readonly reset: () => void;
}

export type { StoreHook };

const INITIAL_STATE = {
  videoSrc: null,
  videoPath: null,
  currentTime: 0,
  duration: 0,
  isPlaying: false,
  playbackRate: 1.0,
  volume: 1.0,
  isMuted: false,
  isScrubbing: false,
  seekTargetTime: null,
  preset916: 'center-crop' as Preset916,
  aspectRatio: '9:16' as AspectRatioMode,
  segments: [] as readonly TranscriptSegment[],
  activeSegmentIds: [] as readonly string[],
  activeSegments: [] as readonly TranscriptSegment[],
  selectedSegmentId: null as string | null,
};

export const useStudioStore = createStore<StudioState>((set, get) => ({
  ...INITIAL_STATE,

  setVideoSrc: (src, videoPath = null) => {
    set({
      videoSrc: src,
      videoPath: videoPath ?? (typeof src === 'string' && !src.startsWith('http') && !src.startsWith('asset://') && !src.startsWith('blob:') ? src : null),
    });
  },

  setCurrentTime: (time) => {
    const validTime = Number.isFinite(time) && time >= 0 ? time : 0;
    const currentSegments = get().segments;
    const active = findActiveSegments(currentSegments, validTime);
    const activeIds = active.map((s) => s.segmentId ?? s.segment_id);

    set({
      currentTime: validTime,
      activeSegments: active,
      activeSegmentIds: activeIds,
    });
  },

  setDuration: (duration) => {
    const validDuration = Number.isFinite(duration) && duration >= 0 ? duration : 0;
    set({ duration: validDuration });
  },

  setIsPlaying: (isPlaying) => {
    set({ isPlaying });
  },

  togglePlay: () => {
    set((state) => ({ isPlaying: !state.isPlaying }));
  },

  setPlaybackRate: (rate) => {
    const validRate = Number.isFinite(rate) && rate > 0 ? rate : 1.0;
    set({ playbackRate: validRate });
  },

  setVolume: (volume) => {
    const clamped = Math.max(0, Math.min(1, Number.isFinite(volume) ? volume : 1.0));
    set({
      volume: clamped,
      isMuted: clamped === 0,
    });
  },

  setIsMuted: (isMuted) => {
    set({ isMuted });
  },

  toggleMute: () => {
    set((state) => ({ isMuted: !state.isMuted }));
  },

  setPreset916: (preset916) => {
    set({ preset916 });
  },

  setAspectRatio: (aspectRatio) => {
    set({ aspectRatio });
  },

  setSegments: (segments) => {
    // Ensure segments are sorted by startSec to uphold binary search invariant
    const sorted = [...segments].sort((a, b) => {
      const aStart = a.startSec ?? a.start_sec;
      const bStart = b.startSec ?? b.start_sec;
      return aStart - bStart;
    });

    const active = findActiveSegments(sorted, get().currentTime);
    const activeIds = active.map((s) => s.segmentId ?? s.segment_id);

    set({
      segments: sorted,
      activeSegments: active,
      activeSegmentIds: activeIds,
    });
  },

  selectSegment: (segmentId) => {
    set({ selectedSegmentId: segmentId });
  },

  seekTo: (time) => {
    const duration = get().duration;
    let clamped = Number.isFinite(time) ? Math.max(0, time) : 0;
    if (duration > 0) {
      clamped = Math.min(clamped, duration);
    }

    const currentSegments = get().segments;
    const active = findActiveSegments(currentSegments, clamped);
    const activeIds = active.map((s) => s.segmentId ?? s.segment_id);

    set({
      currentTime: clamped,
      seekTargetTime: clamped,
      activeSegments: active,
      activeSegmentIds: activeIds,
    });
  },

  clearSeekTarget: () => {
    set({ seekTargetTime: null });
  },

  setIsScrubbing: (isScrubbing) => {
    set({ isScrubbing });
  },

  getActiveSegments: () => {
    return get().activeSegments;
  },

  reset: () => {
    set({ ...INITIAL_STATE });
  },
}));
