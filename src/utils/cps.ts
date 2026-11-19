/**
 * NeuroClip Reading Speed (CPS) Engine & Manual Time Borrowing Logic
 *
 * Implements authoritative mathematical models and categorization for:
 * - UTF-8 character-based CPS calculation with Vietnamese diacritic & emoji support
 * - 3-tier reading speed rating (Green < 17.0, Yellow 17.0..=20.0, Red > 20.0)
 * - Collision-safe silence gap borrowing (max 800ms, preserving >= 50ms buffer)
 *
 * Adheres strictly to docs/CONVENTIONS.md, docs/TASK_BOARD.md (TASK-M4-23), and ADR-005.
 */

import type { TranscriptSegment } from '../types/transcript.ts';

// ---------------------------------------------------------------------------
// Constants (SSOT per ADR-005 & TASK_BOARD.md lines 1079-1115)
// ---------------------------------------------------------------------------

export const CPS_GREEN_THRESHOLD = 17.0;
export const CPS_RED_THRESHOLD = 20.0;
export const DEFAULT_TARGET_CPS = 17.0;
export const DEFAULT_MAX_BORROW_MS = 800; // 0.8s max allowable extension
export const DEFAULT_MIN_GAP_MS = 50;     // 0.05s collision safety buffer
export const MIN_SEGMENT_DURATION_SEC = 0.05; // 50ms minimum duration clamp

// ---------------------------------------------------------------------------
// Types & Contracts
// ---------------------------------------------------------------------------

export type CpsRating = 'green' | 'yellow' | 'red';

export interface CpsEvaluation {
  readonly charCount: number;
  readonly durationSec: number;
  readonly cps: number;
  readonly rating: CpsRating;
  readonly isOverSpeed: boolean;
}

export interface TimeBorrowingResult {
  readonly segmentId: string;
  readonly initialEndSec: number;
  readonly newEndSec: number;
  readonly borrowedSec: number;
  readonly initialCps: number;
  readonly newCps: number;
  readonly initialRating: CpsRating;
  readonly newRating: CpsRating;
}

// ---------------------------------------------------------------------------
// Core Algorithms
// ---------------------------------------------------------------------------

/**
 * Counts Unicode characters (code points) using Array.from to accurately handle
 * Vietnamese accented characters, tone marks, and multi-byte emojis.
 */
export function getUnicodeCharCount(text: string): number {
  if (!text) return 0;
  return Array.from(text.trim()).length;
}

/**
 * Resolves effective subtitle text for CPS calculation.
 * Prefers Vietnamese subtitle (textVi / text_vi); falls back to English (textEn / text_en).
 */
export function getEffectiveText(segment: Partial<TranscriptSegment>): string {
  const vi = segment.textVi ?? segment.text_vi;
  if (typeof vi === 'string' && vi.trim() !== '') {
    return vi;
  }
  const en = segment.textEn ?? segment.text_en;
  if (typeof en === 'string' && en.trim() !== '') {
    return en;
  }
  return '';
}

/**
 * Calculates Characters Per Second (CPS) given text and duration.
 * Clamps duration >= 0.05s to avoid division by zero or NaN.
 */
export function calculateCps(text: string, durationSec: number): number {
  const chars = getUnicodeCharCount(text);
  const dur = Math.max(MIN_SEGMENT_DURATION_SEC, durationSec);
  const val = chars / dur;
  return Number.isFinite(val) && val >= 0 ? Number(val.toFixed(2)) : 0;
}

/**
 * Classifies CPS score into 3-tier readability rating:
 * - Green (< 17.0): Lý tưởng (Ideal)
 * - Yellow (17.0 <= CPS <= 20.0): Nhanh (Fast but acceptable)
 * - Red (> 20.0): Quá nhanh (Over-speed alert)
 */
export function rateCps(cps: number): CpsRating {
  if (cps < CPS_GREEN_THRESHOLD) {
    return 'green';
  }
  if (cps <= CPS_RED_THRESHOLD) {
    return 'yellow';
  }
  return 'red';
}

/**
 * Evaluates full CPS metrics and rating for a segment.
 */
export function evaluateSegmentCps(segment: Partial<TranscriptSegment>): CpsEvaluation {
  const text = getEffectiveText(segment);
  const start = segment.startSec ?? segment.start_sec ?? 0;
  const end = segment.endSec ?? segment.end_sec ?? 0;
  const dur = Math.max(MIN_SEGMENT_DURATION_SEC, end - start);
  const chars = getUnicodeCharCount(text);
  const cps = calculateCps(text, dur);
  const rating = rateCps(cps);

  return {
    charCount: chars,
    durationSec: dur,
    cps,
    rating,
    isOverSpeed: rating === 'red',
  };
}

/**
 * Computes allowable borrowed silence duration in seconds for an individual segment.
 *
 * Rules:
 * 1. Only applies if current CPS > 20.0 (red alert threshold).
 * 2. Attempts to extend end_sec so CPS reaches targetCps (default 17.0).
 * 3. Never borrows more than maxBorrowMs (default 800ms / 0.8s).
 * 4. Preserves at least minGapMs (default 50ms / 0.05s) collision safety gap before next segment startSec.
 * 5. Returns 0 if no extension is needed or if silence gap is insufficient (< 50ms).
 */
export function calculateBorrowDuration(
  segment: Partial<TranscriptSegment>,
  nextSegment?: Partial<TranscriptSegment> | null,
  maxClipDurationSec?: number,
  maxBorrowMs: number = DEFAULT_MAX_BORROW_MS,
  minGapMs: number = DEFAULT_MIN_GAP_MS,
  targetCps: number = DEFAULT_TARGET_CPS,
): number {
  const text = getEffectiveText(segment);
  const start = segment.startSec ?? segment.start_sec ?? 0;
  const end = segment.endSec ?? segment.end_sec ?? 0;
  const currentDur = Math.max(MIN_SEGMENT_DURATION_SEC, end - start);
  const currentCps = calculateCps(text, currentDur);

  // No borrow needed if not over-speed
  if (currentCps <= CPS_RED_THRESHOLD) {
    return 0;
  }

  const charCount = getUnicodeCharCount(text);
  if (charCount === 0 || targetCps <= 0) {
    return 0;
  }

  // Calculate needed extension to reach target CPS (17.0)
  const desiredDur = charCount / targetCps;
  const neededExtension = Math.max(0, desiredDur - currentDur);
  if (neededExtension <= 0) {
    return 0;
  }

  const maxBorrowSec = maxBorrowMs / 1000;
  const minGapSec = minGapMs / 1000;

  // Calculate maximum permissible extension based on silence gap
  let maxGapExtension = Number.POSITIVE_INFINITY;
  if (nextSegment) {
    const nextStart = nextSegment.startSec ?? nextSegment.start_sec ?? end;
    maxGapExtension = Math.max(0, nextStart - end - minGapSec);
  } else if (typeof maxClipDurationSec === 'number' && maxClipDurationSec > 0) {
    maxGapExtension = Math.max(0, maxClipDurationSec - end);
  }

  const allowed = Math.min(neededExtension, maxBorrowSec, maxGapExtension);

  return Number.isFinite(allowed) && allowed > 0 ? Math.round(allowed * 1000) / 1000 : 0;
}
