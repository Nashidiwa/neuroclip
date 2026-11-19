/**
 * NeuroClip Lean Subtitle Studio UI
 *
 * 2-Column editor layout integrating VideoCanvas, TimelineScrubber, and
 * bi-directional synchronized subtitle segment list with sub-30ms sync latency,
 * speaker color indicators, and click-to-seek navigation.
 *
 * Strict adherence to docs/CONVENTIONS.md (TypeScript strict mode, zero any, strict null safety).
 */

import React, { useEffect, useCallback } from 'react';
import { MessageSquare, Sparkles } from 'lucide-react';
import { ErrorBoundary } from '../common/ErrorBoundary.tsx';
import { VideoCanvas } from './VideoCanvas.tsx';
import { TimelineScrubber } from './TimelineScrubber.tsx';
import { VirtualizedSegmentList } from './VirtualizedSegmentList.tsx';
import { useStudioStore } from '../../stores/useStudioStore.ts';
import type { TranscriptSegment } from '../../types/transcript.ts';

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
  // Store bindings
  const storeSegments = useStudioStore((s) => s.segments);
  const activeSegmentIds = useStudioStore((s) => s.activeSegmentIds);
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

        {/* Virtualized Subtitle Segment List */}
        <VirtualizedSegmentList
          segments={effectiveSegments}
          onSegmentSelect={handleSegmentClick}
          className="flex-1"
        />
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
