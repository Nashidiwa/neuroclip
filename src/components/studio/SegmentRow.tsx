/**
 * NeuroClip Subtitle Studio - Virtualized Subtitle Segment Row
 *
 * Renders an individual subtitle segment with:
 * - Speaker badge with PO Option 1 character color styling
 * - Millisecond formatted In/Out timecodes (mm:ss.mmm)
 * - TimecodeNudge buttons for +/-100ms micro-adjustments
 * - Inline auto-resizing textareas for English and Vietnamese with <= 16ms response (zero input lag)
 * - CPS reading speed score badge
 * - React.memo with custom equality comparator per docs/CONVENTIONS.md §3.5
 */

import React, { useEffect, useRef, useCallback } from 'react';
import { Clock, User, CheckCircle2 } from 'lucide-react';
import type { TranscriptSegment, SpeakerRole } from '../../types/transcript.ts';
import {
  useStudioStore,
  formatStudioTimecode,
  getSpeakerColor,
} from '../../stores/useStudioStore.ts';
import { TimecodeNudge } from './TimecodeNudge.tsx';

export interface SegmentRowProps {
  readonly segment: TranscriptSegment;
  readonly isActive: boolean;
  readonly isSelected: boolean;
  readonly onSelect?: ((segment: TranscriptSegment) => void) | undefined;
  readonly onUpdateText?:
    | ((
        segmentId: string,
        updates: {
          text?: string;
          text_vi?: string | null;
          text_en?: string;
          textVi?: string | null;
          textEn?: string;
        },
      ) => void)
    | undefined;
  readonly onNudgeTime?:
    | ((segmentId: string, boundary: 'start' | 'end', deltaSec: number) => void)
    | undefined;
  readonly className?: string | undefined;
}

const SegmentRowInner: React.FC<SegmentRowProps> = ({
  segment,
  isActive,
  isSelected,
  onSelect,
  onUpdateText,
  onNudgeTime,
  className = '',
}) => {
  const viTextareaRef = useRef<HTMLTextAreaElement | null>(null);
  const enTextareaRef = useRef<HTMLTextAreaElement | null>(null);

  const segId = segment.segmentId ?? segment.segment_id;
  const start = segment.startSec ?? segment.start_sec;
  const end = segment.endSec ?? segment.end_sec;
  const speaker: SpeakerRole = segment.speakerId ?? segment.speaker_id;
  const speakerColor = getSpeakerColor(speaker);
  const textVi = segment.textVi ?? segment.text_vi ?? '';
  const textEn = segment.textEn ?? segment.text_en ?? '';
  const cps = segment.cpsScore ?? segment.cps_score ?? 0;

  // Auto-resize textarea to fit content without scrollbars
  const adjustHeight = (el: HTMLTextAreaElement | null) => {
    if (!el) return;
    el.style.height = 'auto';
    el.style.height = `${el.scrollHeight}px`;
  };

  useEffect(() => {
    adjustHeight(viTextareaRef.current);
  }, [textVi]);

  useEffect(() => {
    adjustHeight(enTextareaRef.current);
  }, [textEn]);

  // Synchronous text update dispatch (<= 16ms, zero input lag)
  const handleViChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const val = e.target.value;
    if (onUpdateText) {
      onUpdateText(segId, { text_vi: val, textVi: val });
    } else {
      useStudioStore.getState().updateSegmentText(segId, { text_vi: val, textVi: val });
    }
    adjustHeight(e.target);
  };

  const handleEnChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const val = e.target.value;
    if (onUpdateText) {
      onUpdateText(segId, { text_en: val, textEn: val });
    } else {
      useStudioStore.getState().updateSegmentText(segId, { text_en: val, textEn: val });
    }
    adjustHeight(e.target);
  };

  const handleNudge = useCallback(
    (targetId: string, boundary: 'start' | 'end', deltaSec: number) => {
      if (onNudgeTime) {
        onNudgeTime(targetId, boundary, deltaSec);
      } else {
        useStudioStore.getState().nudgeSegmentTime(targetId, boundary, deltaSec);
      }
    },
    [onNudgeTime],
  );

  // Hotkey support for Shift+A / Shift+D / A / D on focused/selected row
  const handleKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    const isInsideInput =
      e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement;

    if (!isInsideInput) {
      if (e.key === 'A' || e.key === 'a') {
        e.preventDefault();
        handleNudge(segId, 'start', -0.1);
      } else if (e.key === 'D' || e.key === 'd') {
        e.preventDefault();
        handleNudge(segId, 'start', 0.1);
      }
    }
  };

  // Prevent textarea text typing from bubbling studio playback hotkeys
  const handleTextareaKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    // Only stop bubbling to prevent video playback shortcuts (Space, etc.)
    // Do NOT intercept Shift+A / Shift+D so users can type uppercase 'A' and 'D'
    e.stopPropagation();
  };

  const handleClick = () => {
    if (onSelect) {
      onSelect(segment);
    } else {
      useStudioStore.getState().selectSegment(segId);
      useStudioStore.getState().seekTo(start);
    }
  };

  // CPS reading speed color categorization
  const cpsBadgeClass =
    cps < 17
      ? 'text-emerald-400 bg-emerald-950/40 border-emerald-500/30'
      : cps <= 20
      ? 'text-amber-400 bg-amber-950/40 border-amber-500/30'
      : 'text-rose-400 bg-rose-950/40 border-rose-500/30';

  return (
    <div
      tabIndex={0}
      data-testid={`segment-row-${segId}`}
      data-segment-id={segId}
      data-active={isActive ? 'true' : 'false'}
      data-selected={isSelected ? 'true' : 'false'}
      onClick={handleClick}
      onKeyDown={handleKeyDown}
      className={`group relative flex flex-col space-y-2 rounded-lg border p-3 transition-colors outline-none cursor-pointer ${
        isActive
          ? 'border-accent-neuro bg-accent-neuro/15 shadow-md shadow-accent-neuro/10'
          : isSelected
          ? 'border-slate-400 bg-bg-tertiary/90 ring-1 ring-slate-400/50'
          : 'border-border-dark bg-bg-secondary/50 hover:border-slate-600 hover:bg-bg-tertiary/40'
      } ${className}`}
    >
      {/* Top Metadata Bar: Speaker Badge, Timecodes, Nudge Buttons, CPS Score */}
      <div className="flex flex-wrap items-center justify-between gap-1 text-xs select-none">
        <div className="flex flex-wrap items-center gap-2">
          {/* Speaker Badge (PO Option 1 Theme Colors) */}
          <span
            data-testid={`speaker-badge-${segId}`}
            style={{
              borderColor: speakerColor,
              color: speakerColor,
            }}
            className="inline-flex items-center space-x-1 rounded border px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider bg-bg-primary/50"
          >
            <User className="h-2.5 w-2.5 shrink-0" />
            <span>{speaker}</span>
          </span>

          {/* Formatted Timecode & Start/End Nudge Buttons */}
          <div className="flex items-center space-x-1 font-mono text-[11px] text-slate-300">
            <Clock className="h-3 w-3 text-slate-500 shrink-0" />
            <span data-testid={`timecode-start-${segId}`}>{formatStudioTimecode(start)}</span>
            <TimecodeNudge
              segmentId={segId}
              boundary="start"
              valueSec={start}
              onNudge={handleNudge}
            />
            <span className="text-slate-500">&rarr;</span>
            <span data-testid={`timecode-end-${segId}`}>{formatStudioTimecode(end)}</span>
            <TimecodeNudge
              segmentId={segId}
              boundary="end"
              valueSec={end}
              onNudge={handleNudge}
            />
          </div>
        </div>

        {/* CPS Indicator & Active Checkmark */}
        <div className="flex items-center space-x-1.5">
          <span
            className={`rounded-sm border px-1.5 py-0.2 font-mono text-[10px] ${cpsBadgeClass}`}
            title={`Tốc độ đọc: ${cps.toFixed(1)} ký tự/giây`}
          >
            {cps.toFixed(1)} CPS
          </span>
          {isActive && <CheckCircle2 className="h-3.5 w-3.5 text-accent-neuro shrink-0" />}
        </div>
      </div>

      {/* Inline Subtitle Textareas (Auto-resizing, <= 16ms Zero Input Lag) */}
      <div className="flex flex-col space-y-1.5 pt-0.5" onClick={(e) => e.stopPropagation()}>
        <div className="flex flex-col space-y-0.5">
          <label className="text-[10px] font-medium uppercase text-slate-400 tracking-wider">
            Vietsub
          </label>
          <textarea
            ref={viTextareaRef}
            rows={1}
            value={textVi}
            onChange={handleViChange}
            onKeyDown={handleTextareaKeyDown}
            placeholder="Nhập phụ đề tiếng Việt..."
            data-testid={`textarea-vi-${segId}`}
            className="w-full resize-none overflow-hidden rounded border border-border-dark/80 bg-bg-primary/50 px-2 py-1 text-xs sm:text-sm font-medium text-slate-100 placeholder-slate-500 focus:border-accent-neuro focus:outline-none focus:ring-1 focus:ring-accent-neuro/50 transition-colors"
          />
        </div>

        <div className="flex flex-col space-y-0.5">
          <label className="text-[10px] font-medium uppercase text-slate-500 tracking-wider">
            English Transcript
          </label>
          <textarea
            ref={enTextareaRef}
            rows={1}
            value={textEn}
            onChange={handleEnChange}
            onKeyDown={handleTextareaKeyDown}
            placeholder="English transcript..."
            data-testid={`textarea-en-${segId}`}
            className="w-full resize-none overflow-hidden rounded border border-border-dark/60 bg-bg-primary/30 px-2 py-0.5 text-[11px] italic text-slate-400 placeholder-slate-600 focus:border-slate-500 focus:outline-none transition-colors"
          />
        </div>
      </div>
    </div>
  );
};

// React.memo with custom comparison invariant per docs/CONVENTIONS.md §3.5
export const SegmentRow = React.memo(SegmentRowInner, (prev, next) => {
  const prevId = prev.segment.segmentId ?? prev.segment.segment_id;
  const nextId = next.segment.segmentId ?? next.segment.segment_id;
  const prevStart = prev.segment.startSec ?? prev.segment.start_sec;
  const nextStart = next.segment.startSec ?? next.segment.start_sec;
  const prevEnd = prev.segment.endSec ?? prev.segment.end_sec;
  const nextEnd = next.segment.endSec ?? next.segment.end_sec;
  const prevTextVi = prev.segment.textVi ?? prev.segment.text_vi ?? '';
  const nextTextVi = next.segment.textVi ?? next.segment.text_vi ?? '';
  const prevTextEn = prev.segment.textEn ?? prev.segment.text_en ?? '';
  const nextTextEn = next.segment.textEn ?? next.segment.text_en ?? '';
  const prevSpeaker = prev.segment.speakerId ?? prev.segment.speaker_id;
  const nextSpeaker = next.segment.speakerId ?? next.segment.speaker_id;
  const prevCps = prev.segment.cpsScore ?? prev.segment.cps_score;
  const nextCps = next.segment.cpsScore ?? next.segment.cps_score;

  return (
    prevId === nextId &&
    prevStart === nextStart &&
    prevEnd === nextEnd &&
    prevTextVi === nextTextVi &&
    prevTextEn === nextTextEn &&
    prevSpeaker === nextSpeaker &&
    prevCps === nextCps &&
    prev.isActive === next.isActive &&
    prev.isSelected === next.isSelected
  );
});
