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
import {
  isSpeakerRole,
  type TranscriptSegment,
  type SpeakerRole,
} from '../types/transcript.ts';
import { calculateCps } from '../utils/text_wrap.ts';
import { calculateBorrowDuration } from '../utils/cps.ts';

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
  readonly selectedSegmentIds: readonly string[];

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
  readonly setSelectedSegmentId: (segmentId: string | null) => void;
  readonly setSelectedSegmentIds: (ids: readonly string[]) => void;
  readonly toggleSegmentSelection: (segmentId: string, isMulti?: boolean) => void;
  readonly selectSegmentRange: (targetSegmentId: string) => void;
  readonly selectAllSegments: () => void;
  readonly clearSegmentSelection: () => void;
  readonly updateSegmentText: (
    segmentId: string,
    updates: {
      text?: string;
      text_vi?: string | null;
      text_en?: string;
      textVi?: string | null;
      textEn?: string;
    },
  ) => void;
  readonly nudgeSegmentTime: (
    segmentId: string,
    boundary: 'start' | 'end',
    deltaSec: number,
  ) => void;
  readonly borrowSilence: (segmentId: string) => boolean;
  readonly updateSegmentSpeaker: (segmentId: string, speakerId: SpeakerRole | string) => void;
  readonly updateSegmentsSpeaker: (
    segmentIds: readonly string[],
    speakerId: SpeakerRole | string,
  ) => void;
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
  selectedSegmentIds: [] as readonly string[],
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
    set({
      selectedSegmentId: segmentId,
      selectedSegmentIds: segmentId !== null ? [segmentId] : [],
    });
  },

  setSelectedSegmentId: (segmentId) => {
    set({
      selectedSegmentId: segmentId,
      selectedSegmentIds: segmentId !== null ? [segmentId] : [],
    });
  },

  setSelectedSegmentIds: (ids) => {
    set((state) => ({
      selectedSegmentIds: ids,
      selectedSegmentId:
        ids.length > 0
          ? ids.includes(state.selectedSegmentId ?? '')
            ? state.selectedSegmentId
            : (ids[ids.length - 1] ?? null)
          : null,
    }));
  },

  toggleSegmentSelection: (segmentId, isMulti = false) => {
    set((state) => {
      if (!isMulti) {
        return {
          selectedSegmentId: segmentId,
          selectedSegmentIds: [segmentId],
        };
      }

      const exists = state.selectedSegmentIds.includes(segmentId);
      const nextIds = exists
        ? state.selectedSegmentIds.filter((id) => id !== segmentId)
        : [...state.selectedSegmentIds, segmentId];

      const nextSelectedId =
        state.selectedSegmentId === segmentId
          ? (nextIds[nextIds.length - 1] ?? null)
          : (state.selectedSegmentId ?? nextIds[0] ?? null);

      return {
        selectedSegmentIds: nextIds,
        selectedSegmentId: nextSelectedId,
      };
    });
  },

  selectSegmentRange: (targetSegmentId) => {
    set((state) => {
      const segments = state.segments;
      if (segments.length === 0) return {};

      const targetIdx = segments.findIndex(
        (s) => (s.segmentId ?? s.segment_id) === targetSegmentId,
      );
      if (targetIdx < 0) return {};

      const anchorId = state.selectedSegmentId ?? state.selectedSegmentIds[0];
      const anchorIdx = anchorId
        ? segments.findIndex((s) => (s.segmentId ?? s.segment_id) === anchorId)
        : 0;

      const validAnchorIdx = anchorIdx >= 0 ? anchorIdx : 0;
      const startIdx = Math.min(validAnchorIdx, targetIdx);
      const endIdx = Math.max(validAnchorIdx, targetIdx);

      const rangeIds = segments
        .slice(startIdx, endIdx + 1)
        .map((s) => s.segmentId ?? s.segment_id);

      return {
        selectedSegmentIds: rangeIds,
        selectedSegmentId: state.selectedSegmentId ?? anchorId ?? targetSegmentId,
      };
    });
  },

  selectAllSegments: () => {
    set((state) => ({
      selectedSegmentIds: state.segments.map((s) => s.segmentId ?? s.segment_id),
      selectedSegmentId: state.segments[0]
        ? (state.segments[0].segmentId ?? state.segments[0].segment_id)
        : null,
    }));
  },

  clearSegmentSelection: () => {
    set({
      selectedSegmentIds: [],
      selectedSegmentId: null,
    });
  },

  updateSegmentText: (segmentId, updates) => {
    const textVi =
      updates.textVi !== undefined
        ? updates.textVi
        : updates.text_vi !== undefined
        ? updates.text_vi
        : undefined;

    const textEn =
      updates.textEn !== undefined
        ? updates.textEn
        : updates.text_en !== undefined
        ? updates.text_en
        : updates.text !== undefined
        ? updates.text
        : undefined;

    set((state) => {
      const nextSegments = state.segments.map((seg) => {
        const id = seg.segmentId ?? seg.segment_id;
        if (id !== segmentId) return seg;

        const updatedVi = textVi !== undefined ? (textVi ?? '') : (seg.textVi ?? seg.text_vi ?? '');
        const updatedEn = textEn !== undefined ? (textEn ?? '') : (seg.textEn ?? seg.text_en ?? '');

        const start = seg.startSec ?? seg.start_sec;
        const end = seg.endSec ?? seg.end_sec;
        const durationSec = Math.max(0.05, end - start);
        const effectiveText = updatedVi && updatedVi.trim() !== '' ? updatedVi : updatedEn;
        const newCps = calculateCps(effectiveText, durationSec);

        return {
          ...seg,
          textVi: updatedVi,
          text_vi: updatedVi,
          textEn: updatedEn,
          text_en: updatedEn,
          cpsScore: newCps,
          cps_score: newCps,
        };
      });

      const active = findActiveSegments(nextSegments, state.currentTime);
      const activeIds = active.map((s) => s.segmentId ?? s.segment_id);

      return {
        segments: nextSegments,
        activeSegments: active,
        activeSegmentIds: activeIds,
      };
    });
  },

  nudgeSegmentTime: (segmentId, boundary, deltaSec) => {
    if (!Number.isFinite(deltaSec) || deltaSec === 0) return;

    set((state) => {
      let needsResort = false;

      const nextSegments = state.segments.map((seg) => {
        const id = seg.segmentId ?? seg.segment_id;
        if (id !== segmentId) return seg;

        let start = seg.startSec ?? seg.start_sec;
        let end = seg.endSec ?? seg.end_sec;

        if (boundary === 'start') {
          const raw = start + deltaSec;
          start = Math.max(0, Math.round(raw * 1000) / 1000);
          if (start > end - 0.05) {
            start = Math.max(0, Math.round((end - 0.05) * 1000) / 1000);
          }
          needsResort = true;
        } else {
          const raw = end + deltaSec;
          const minEnd = Math.round((start + 0.05) * 1000) / 1000;
          end = Math.max(minEnd, Math.round(raw * 1000) / 1000);
          if (state.duration > 0 && end > state.duration) {
            end = state.duration;
          }
        }

        const durationSec = Math.max(0.05, end - start);
        const currentVi = seg.textVi ?? seg.text_vi ?? '';
        const currentEn = seg.textEn ?? seg.text_en ?? '';
        const effectiveText = currentVi && currentVi.trim() !== '' ? currentVi : currentEn;
        const newCps = calculateCps(effectiveText, durationSec);

        return {
          ...seg,
          startSec: start,
          start_sec: start,
          endSec: end,
          end_sec: end,
          cpsScore: newCps,
          cps_score: newCps,
        };
      });

      const finalSegments = needsResort
        ? [...nextSegments].sort((a, b) => {
            const aStart = a.startSec ?? a.start_sec;
            const bStart = b.startSec ?? b.start_sec;
            return aStart - bStart;
          })
        : nextSegments;

      const active = findActiveSegments(finalSegments, state.currentTime);
      const activeIds = active.map((s) => s.segmentId ?? s.segment_id);

      return {
        segments: finalSegments,
        activeSegments: active,
        activeSegmentIds: activeIds,
      };
    });
  },

  borrowSilence: (segmentId) => {
    const state = get();
    const segIdx = state.segments.findIndex(
      (s) => (s.segmentId ?? s.segment_id) === segmentId,
    );
    if (segIdx < 0) return false;
    const currentSeg = state.segments[segIdx];
    if (!currentSeg) return false;
    const nextSeg = state.segments[segIdx + 1];

    const deltaSec = calculateBorrowDuration(currentSeg, nextSeg, state.duration);
    if (deltaSec <= 0) return false;

    get().nudgeSegmentTime(segmentId, 'end', deltaSec);
    return true;
  },

  updateSegmentsSpeaker: (segmentIds, speakerId) => {
    if (segmentIds.length === 0) return;
    const validRole: SpeakerRole = isSpeakerRole(speakerId) ? speakerId : 'unknown';
    const targetIdSet = new Set(segmentIds);

    set((state) => {
      let modified = false;
      const nextSegments = state.segments.map((seg) => {
        const id = seg.segmentId ?? seg.segment_id;
        if (!targetIdSet.has(id)) return seg;
        if ((seg.speakerId ?? seg.speaker_id) === validRole) return seg;
        modified = true;
        return {
          ...seg,
          speakerId: validRole,
          speaker_id: validRole,
        };
      });

      if (!modified) return {};

      const active = findActiveSegments(nextSegments, state.currentTime);
      const activeIds = active.map((s) => s.segmentId ?? s.segment_id);

      return {
        segments: nextSegments,
        activeSegments: active,
        activeSegmentIds: activeIds,
      };
    });
  },

  updateSegmentSpeaker: (segmentId, speakerId) => {
    get().updateSegmentsSpeaker([segmentId], speakerId);
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
