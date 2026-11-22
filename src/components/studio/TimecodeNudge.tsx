/**
 * NeuroClip Subtitle Studio - Timecode Nudge Component
 *
 * Provides +/-100ms micro-adjustment controls for subtitle segment timecodes.
 * Supports clickable buttons and keyboard shortcuts.
 *
 * Strict adherence to docs/CONVENTIONS.md (TypeScript strict mode, zero any).
 */

import React, { useCallback } from 'react';
import { Minus, Plus } from 'lucide-react';

export interface TimecodeNudgeProps {
  readonly segmentId: string;
  readonly boundary: 'start' | 'end';
  readonly valueSec: number;
  readonly onNudge: (segmentId: string, boundary: 'start' | 'end', deltaSec: number) => void;
  readonly disabled?: boolean | undefined;
  readonly className?: string | undefined;
}

export const TimecodeNudge: React.FC<TimecodeNudgeProps> = ({
  segmentId,
  boundary,
  valueSec,
  onNudge,
  disabled = false,
  className = '',
}) => {
  const handleDecrement = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      if (!disabled) {
        onNudge(segmentId, boundary, -0.1);
      }
    },
    [disabled, onNudge, segmentId, boundary],
  );

  const handleIncrement = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      if (!disabled) {
        onNudge(segmentId, boundary, 0.1);
      }
    },
    [disabled, onNudge, segmentId, boundary],
  );

  const isDecrementDisabled = disabled || (boundary === 'start' && valueSec <= 0);

  return (
    <div
      data-testid={`timecode-nudge-${boundary}-${segmentId}`}
      className={`inline-flex items-center space-x-0.5 ${className}`}
    >
      <button
        type="button"
        disabled={isDecrementDisabled}
        onClick={handleDecrement}
        data-testid={`nudge-${boundary}-minus`}
        aria-label={`Lùi mốc ${boundary} 100ms`}
        title={`Lùi 100ms (${boundary === 'start' ? 'Shift+A' : ''})`}
        className="rounded p-0.5 text-slate-400 transition-colors hover:bg-bg-tertiary hover:text-slate-200 active:scale-95 disabled:opacity-40"
      >
        <span data-testid={`nudge-${boundary}-minus-100ms`} className="sr-only">
          -100ms
        </span>
        <Minus className="h-2.5 w-2.5" />
      </button>

      <button
        type="button"
        disabled={disabled}
        onClick={handleIncrement}
        data-testid={`nudge-${boundary}-plus`}
        aria-label={`Tiến mốc ${boundary} 100ms`}
        title={`Tiến 100ms (${boundary === 'start' ? 'Shift+D' : ''})`}
        className="rounded p-0.5 text-slate-400 transition-colors hover:bg-bg-tertiary hover:text-slate-200 active:scale-95 disabled:opacity-40"
      >
        <span data-testid={`nudge-${boundary}-plus-100ms`} className="sr-only">
          +100ms
        </span>
        <Plus className="h-2.5 w-2.5" />
      </button>
    </div>
  );
};
