/**
 * NeuroClip Subtitle Studio - Visual Reading Speed (CPS) Warning Lamp
 * & Manual Time Borrowing Assistant (`TASK-M4-23`, `ADR-005`, `PRD.md` §6.1).
 *
 * Provides:
 * - 3-tier visual reading speed badge (Green < 17, Yellow 17..20, Pulsating Red > 20)
 * - 1-touch "Nới thời gian (Borrow Silence)" assistant button when CPS > 20
 * - Collision safety preservation (>= 50ms buffer) and max 800ms extension
 * - Zero event propagation leakage to prevent row selection/video seek on button click
 * - Strict adherence to docs/CONVENTIONS.md (TypeScript strict mode, zero any)
 */

import React, { useCallback } from 'react';
import { Clock, AlertTriangle } from 'lucide-react';
import type { TranscriptSegment } from '../../types/transcript.ts';
import {
  evaluateSegmentCps,
  calculateBorrowDuration,
  DEFAULT_MAX_BORROW_MS,
  DEFAULT_MIN_GAP_MS,
} from '../../utils/cps.ts';

export interface CPSIndicatorProps {
  readonly segment: TranscriptSegment;
  readonly nextSegment?: TranscriptSegment | undefined;
  readonly onBorrowSilence?: ((segmentId: string, deltaSec: number) => void) | undefined;
  readonly className?: string | undefined;
}

export const CPSIndicator: React.FC<CPSIndicatorProps> = ({
  segment,
  nextSegment,
  onBorrowSilence,
  className = '',
}) => {
  const segId = segment.segmentId ?? segment.segment_id;
  const { cps, rating, isOverSpeed } = evaluateSegmentCps(segment);

  const allowableBorrowSec = isOverSpeed
    ? calculateBorrowDuration(
        segment,
        nextSegment,
        undefined,
        DEFAULT_MAX_BORROW_MS,
        DEFAULT_MIN_GAP_MS,
      )
    : 0;

  const handleBorrowClick = useCallback(
    (e: React.MouseEvent<HTMLButtonElement>) => {
      e.stopPropagation();
      if (allowableBorrowSec > 0 && onBorrowSilence) {
        onBorrowSilence(segId, allowableBorrowSec);
      }
    },
    [segId, allowableBorrowSec, onBorrowSilence],
  );

  let badgeStyles = 'border-emerald-500/30 bg-emerald-950/40 text-emerald-400';
  let badgeLabel = 'Tốt';

  if (rating === 'yellow') {
    badgeStyles = 'border-amber-500/30 bg-amber-950/40 text-amber-400';
    badgeLabel = 'Nhanh';
  } else if (rating === 'red') {
    badgeStyles =
      'border-rose-500/40 bg-rose-950/50 text-rose-300 animate-pulse font-semibold shadow-sm shadow-rose-950';
    badgeLabel = 'Quá nhanh!';
  }

  return (
    <div
      data-testid={`cps-indicator-${segId}`}
      data-rating={rating}
      className={`inline-flex items-center space-x-1.5 select-none ${className}`}
    >
      {/* Visual Badge Indicator */}
      <span
        data-testid={`cps-badge-${segId}`}
        title={`Tốc độ đọc: ${cps.toFixed(1)} ký tự/giây (${badgeLabel})`}
        className={`inline-flex items-center space-x-1 rounded-sm border px-1.5 py-0.5 font-mono text-[10px] transition-colors ${badgeStyles}`}
      >
        {isOverSpeed && <AlertTriangle className="h-2.5 w-2.5 shrink-0 text-rose-400" />}
        <span>{cps.toFixed(1)} CPS</span>
        <span className="hidden sm:inline font-sans text-[9px] opacity-80">({badgeLabel})</span>
      </span>

      {/* 1-Touch Borrow Silence Button (Displayed ONLY when CPS > 20) */}
      {isOverSpeed && (
        <button
          type="button"
          data-testid={`borrow-silence-btn-${segId}`}
          onClick={handleBorrowClick}
          disabled={allowableBorrowSec <= 0}
          title={
            allowableBorrowSec > 0
              ? `Nới thời gian (+${(allowableBorrowSec * 1000).toFixed(0)}ms vào khoảng lặng)`
              : 'Không có khoảng lặng khả dụng để mượn thời gian'
          }
          className={`inline-flex items-center space-x-1 rounded border px-1.5 py-0.5 text-[10px] font-medium transition-all ${
            allowableBorrowSec > 0
              ? 'border-accent-neuro/60 bg-accent-neuro/20 text-accent-neuro hover:bg-accent-neuro/30 hover:border-accent-neuro cursor-pointer active:scale-95'
              : 'border-slate-700 bg-slate-800/40 text-slate-500 cursor-not-allowed opacity-60'
          }`}
        >
          <Clock className="h-2.5 w-2.5 shrink-0" />
          <span>Nới thời gian</span>
          {allowableBorrowSec > 0 && (
            <span className="font-mono text-[9px] opacity-90">
              (+{(allowableBorrowSec * 1000).toFixed(0)}ms)
            </span>
          )}
        </button>
      )}
    </div>
  );
};
