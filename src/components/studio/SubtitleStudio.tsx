/**
 * NeuroClip Lean Subtitle Studio UI
 *
 * 2-Column editor layout integrating VideoCanvas, TimelineScrubber, and
 * bi-directional synchronized subtitle segment list with sub-30ms sync latency,
 * speaker color indicators, and click-to-seek navigation.
 *
 * Strict adherence to docs/CONVENTIONS.md (TypeScript strict mode, zero any, strict null safety).
 */

import React, { useEffect, useRef, useCallback } from 'react';
import {
  MessageSquare,
  Sparkles,
  CheckCircle2,
  Clock,
  User,
} from 'lucide-react';
import { ErrorBoundary } from '../common/ErrorBoundary.tsx';
import { VideoCanvas } from './VideoCanvas.tsx';
import { TimelineScrubber } from './TimelineScrubber.tsx';
import {
  useStudioStore,
  formatStudioTimecode,
  getSpeakerColor,
} from '../../stores/useStudioStore.ts';
import type { TranscriptSegment, SpeakerRole } from '../../types/transcript.ts';

export interface SubtitleStudioProps {
  readonly videoSrc?: string | null | undefined;
  readonly segments?: readonly TranscriptSegment[] | undefined;
  readonly onSegmentSelect?: ((segment: TranscriptSegment) => void) | undefined;
  readonly className?: string | undefined;
}

export const SubtitleStudioInner: React.FC<SubtitleStudioProps> = ({
  videoSrc: propVideoSrc,
  segments: propSegments,
  onSegmentSelect,
  className = '',
}) => {
  const segmentListRef = useRef<HTMLDivElement | null>(null);
  const activeSegmentRefs = useRef<Map<string, HTMLDivElement>>(new Map());

  // Store bindings
  const storeSegments = useStudioStore((s) => s.segments);
  const activeSegmentIds = useStudioStore((s) => s.activeSegmentIds);
  const selectedSegmentId = useStudioStore((s) => s.selectedSegmentId);
  const currentTime = useStudioStore((s) => s.currentTime);
  const duration = useStudioStore((s) => s.duration);
  const setVideoSrcInStore = useStudioStore((s) => s.setVideoSrc);
  const setSegmentsInStore = useStudioStore((s) => s.setSegments);
  const seekTo = useStudioStore((s) => s.seekTo);
  const selectSegmentInStore = useStudioStore((s) => s.selectSegment);

  // Synchronize incoming props to store if provided
  useEffect(() => {
    if (propVideoSrc !== undefined) {
      setVideoSrcInStore(propVideoSrc);
    }
  }, [propVideoSrc, setVideoSrcInStore]);

  useEffect(() => {
    if (propSegments !== undefined) {
      setSegmentsInStore(propSegments);
    }
  }, [propSegments, setSegmentsInStore]);

  const effectiveSegments = propSegments !== undefined ? propSegments : storeSegments;

  // Auto-scroll the primary active subtitle row into view
  useEffect(() => {
    if (activeSegmentIds.length === 0) return;
    const firstActiveId = activeSegmentIds[0];
    if (!firstActiveId) return;

    const targetEl = activeSegmentRefs.current.get(firstActiveId);
    if (targetEl && typeof targetEl.scrollIntoView === 'function') {
      targetEl.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
    }
  }, [activeSegmentIds]);

  const handleSegmentClick = useCallback(
    (seg: TranscriptSegment) => {
      const segId = seg.segmentId ?? seg.segment_id;
      const start = seg.startSec ?? seg.start_sec;

      selectSegmentInStore(segId);
      seekTo(start);

      if (onSegmentSelect) {
        onSegmentSelect(seg);
      }
    },
    [onSegmentSelect, seekTo, selectSegmentInStore],
  );

  return (
    <div
      data-testid="subtitle-studio-container"
      className={`flex h-full w-full flex-col lg:flex-row overflow-hidden bg-bg-primary text-white select-none ${className}`}
    >
      {/* Left Column: Video Preview Canvas & Timeline Scrubber */}
      <div className="flex flex-1 flex-col items-center justify-between border-b lg:border-b-0 lg:border-r border-border-dark bg-bg-secondary/30 p-4 overflow-y-auto">
        <div className="w-full flex flex-col items-center justify-center flex-1 my-auto">
          <VideoCanvas videoSrc={propVideoSrc} />
        </div>

        <div className="w-full max-w-3xl pt-4">
          <TimelineScrubber currentTime={currentTime} duration={duration} />
        </div>
      </div>

      {/* Right Column: Subtitle Segment List Panel */}
      <div className="w-full lg:w-[460px] flex flex-col bg-bg-secondary/70 border-border-dark overflow-hidden shrink-0">
        {/* Panel Header */}
        <div className="flex items-center justify-between border-b border-border-dark bg-bg-secondary px-4 py-3">
          <div className="flex items-center space-x-2">
            <MessageSquare className="h-4 w-4 text-accent-neuro" />
            <h2 className="text-sm font-semibold tracking-wide text-white">
              Phân đoạn Phụ đề ({effectiveSegments.length})
            </h2>
          </div>
          <div className="flex items-center space-x-1.5 text-xs text-slate-400">
            <span className="flex items-center space-x-1 font-mono text-[11px] text-accent-neuro">
              <Sparkles className="h-3 w-3" />
              <span>{activeSegmentIds.length} đang active</span>
            </span>
          </div>
        </div>

        {/* Subtitle Segment List */}
        <div
          ref={segmentListRef}
          data-testid="studio-segment-list"
          className="flex-1 divide-y divide-border-dark/60 overflow-y-auto p-2 space-y-1.5"
        >
          {effectiveSegments.length === 0 ? (
            <div
              data-testid="studio-empty-segments"
              className="flex h-full flex-col items-center justify-center space-y-2 p-8 text-center text-slate-400"
            >
              <MessageSquare className="h-8 w-8 text-slate-500/70" />
              <p className="text-sm font-medium">Chưa có phân đoạn phụ đề</p>
              <p className="text-xs text-slate-500 max-w-[220px]">
                Nạp tệp transcript_segments.json hoặc chạy Whisper STT để tạo phụ đề
              </p>
            </div>
          ) : (
            effectiveSegments.map((segment) => {
              const segId = segment.segmentId ?? segment.segment_id;
              const start = segment.startSec ?? segment.start_sec;
              const end = segment.endSec ?? segment.end_sec;
              const speaker: SpeakerRole = segment.speakerId ?? segment.speaker_id;
              const speakerColor = getSpeakerColor(speaker);
              const textVi = segment.textVi ?? segment.text_vi;
              const textEn = segment.textEn ?? segment.text_en;
              const cps = segment.cpsScore ?? segment.cps_score ?? 0;
              const isActive = activeSegmentIds.includes(segId);
              const isSelected = selectedSegmentId === segId;

              // CPS status color
              const cpsBadgeClass =
                cps < 17
                  ? 'text-emerald-400 bg-emerald-950/40 border-emerald-500/30'
                  : cps <= 20
                  ? 'text-amber-400 bg-amber-950/40 border-amber-500/30'
                  : 'text-rose-400 bg-rose-950/40 border-rose-500/30';

              return (
                <div
                  key={segId}
                  ref={(el) => {
                    if (el) {
                      activeSegmentRefs.current.set(segId, el);
                    } else {
                      activeSegmentRefs.current.delete(segId);
                    }
                  }}
                  data-testid={`segment-row-${segId}`}
                  data-segment-id={segId}
                  data-active={isActive ? 'true' : 'false'}
                  data-selected={isSelected ? 'true' : 'false'}
                  onClick={() => handleSegmentClick(segment)}
                  className={`group relative flex flex-col space-y-1.5 rounded-lg border p-2.5 transition-all cursor-pointer ${
                    isActive
                      ? 'border-accent-neuro bg-accent-neuro/15 shadow-md shadow-accent-neuro/10'
                      : isSelected
                      ? 'border-slate-500 bg-bg-tertiary/80'
                      : 'border-border-dark bg-bg-secondary/50 hover:border-slate-600 hover:bg-bg-tertiary/40'
                  }`}
                >
                  {/* Top metadata bar */}
                  <div className="flex items-center justify-between text-xs">
                    {/* Speaker badge with PO Option 1 colors */}
                    <div className="flex items-center space-x-1.5">
                      <span
                        style={{
                          borderColor: speakerColor,
                          color: speakerColor,
                        }}
                        className="flex items-center space-x-1 rounded-sm border px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wider"
                      >
                        <User className="h-2.5 w-2.5" />
                        <span>{speaker}</span>
                      </span>

                      {/* Timecode Range */}
                      <span className="font-mono text-[11px] text-slate-300 flex items-center space-x-1">
                        <Clock className="h-3 w-3 text-slate-500" />
                        <span>
                          {formatStudioTimecode(start)} &rarr; {formatStudioTimecode(end)}
                        </span>
                      </span>
                    </div>

                    {/* CPS Pill */}
                    <div className="flex items-center space-x-1">
                      <span
                        className={`rounded-sm border px-1 py-0.2 text-[10px] font-mono ${cpsBadgeClass}`}
                        title={`Tốc độ đọc: ${cps.toFixed(1)} ký tự/giây`}
                      >
                        {cps.toFixed(1)} CPS
                      </span>
                      {isActive && (
                        <CheckCircle2 className="h-3.5 w-3.5 text-accent-neuro shrink-0" />
                      )}
                    </div>
                  </div>

                  {/* Subtitle text */}
                  <div className="space-y-0.5 pt-0.5">
                    {textVi && (
                      <p className="text-xs sm:text-sm font-medium text-slate-100 leading-snug">
                        {textVi}
                      </p>
                    )}
                    {textEn && (
                      <p className="text-[11px] text-slate-400 italic leading-snug">
                        {textEn}
                      </p>
                    )}
                  </div>
                </div>
              );
            })
          )}
        </div>
      </div>
    </div>
  );
};

export const SubtitleStudio: React.FC<SubtitleStudioProps> = (props) => {
  return (
    <ErrorBoundary title="Subtitle Studio">
      <SubtitleStudioInner {...props} />
    </ErrorBoundary>
  );
};
