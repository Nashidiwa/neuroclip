/**
 * NeuroClip Lean Subtitle Studio UI
 *
 * 2-Column editor layout integrating VideoCanvas, TimelineScrubber, and
 * bi-directional synchronized subtitle segment list with sub-30ms sync latency,
 * speaker color indicators, and click-to-seek navigation.
 *
 * Strict adherence to docs/CONVENTIONS.md (TypeScript strict mode, zero any, strict null safety).
 */

import React, { useEffect, useCallback, useState, useMemo } from 'react';
import { MessageSquare, Sparkles, Share2 } from 'lucide-react';
import { ErrorBoundary } from '../common/ErrorBoundary.tsx';
import { VideoCanvas } from './VideoCanvas.tsx';
import { TimelineScrubber } from './TimelineScrubber.tsx';
import { VirtualizedSegmentList } from './VirtualizedSegmentList.tsx';
import { CharacterHotkeyBar } from './CharacterHotkeyBar.tsx';
import { ExportModal } from '../export/ExportModal.tsx';
import { useStudioHotkeys } from '../../hooks/useStudioHotkeys.ts';
import { useStudioStore } from '../../stores/useStudioStore.ts';
import type {
  TranscriptSegment,
  SubtitleProject,
} from '../../types/transcript.ts';
import {
  normalizeSubtitleProject,
} from '../../types/transcript.ts';

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
  // Global character hotkeys (1-5, Numpad 1-5)
  useStudioHotkeys();

  // Store bindings
  const storeSegments = useStudioStore((s) => s.segments);
  const activeSegmentIds = useStudioStore((s) => s.activeSegmentIds);
  const currentTime = useStudioStore((s) => s.currentTime);
  const duration = useStudioStore((s) => s.duration);
  const setVideoSrcInStore = useStudioStore((s) => s.setVideoSrc);
  const setSegmentsInStore = useStudioStore((s) => s.setSegments);

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
  const [isExportModalOpen, setIsExportModalOpen] = useState<boolean>(false);

  const currentProject: SubtitleProject = useMemo(() => {
    return normalizeSubtitleProject({
      project_id: 'subproj_studio_export',
      project_name: 'NeuroClip_Highlight',
      vod_id: 'vod_current',
      video_file_path: propVideoSrc ?? null,
      target_layout: 'original_16_9',
      segments: effectiveSegments.map((seg) => ({
        segment_id: seg.segmentId,
        start_sec: seg.startSec,
        end_sec: seg.endSec,
        speaker_id: seg.speakerId,
        text_en: seg.textEn,
        text_vi: seg.textVi,
        confidence: seg.confidence,
        cps_score: seg.cpsScore,
        is_flagged_for_review: seg.isFlaggedForReview,
        words: seg.words.map((w) => ({
          word: w.word,
          start_sec: w.startSec,
          end_sec: w.endSec,
          probability: w.probability,
          confidence: w.confidence,
        })),
      })),
      speaker_styles: {
        neuro: { color_hex: '#FFB6C1', font_name: 'Arial', font_size: 54, outline_color_hex: '#000000' },
        vedal: { color_hex: '#98FB98', font_name: 'Arial', font_size: 54, outline_color_hex: '#000000' },
        evil: { color_hex: '#FF0055', font_name: 'Arial', font_size: 54, outline_color_hex: '#000000' },
        guest: { color_hex: '#FFD700', font_name: 'Arial', font_size: 54, outline_color_hex: '#000000' },
        chat: { color_hex: '#87CEEB', font_name: 'Arial', font_size: 48, outline_color_hex: '#000000' },
        unknown: { color_hex: '#FFFFFF', font_name: 'Arial', font_size: 52, outline_color_hex: '#000000' },
      },
      updated_at: new Date().toISOString(),
    });
  }, [effectiveSegments, propVideoSrc]);

  const handleSegmentClick = useCallback(
    (seg: TranscriptSegment) => {
      if (onSegmentSelect) {
        onSegmentSelect(seg);
      }
    },
    [onSegmentSelect],
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
          <div className="flex items-center space-x-3 text-xs text-slate-400">
            <span className="flex items-center space-x-1 font-mono text-[11px] text-accent-neuro">
              <Sparkles className="h-3 w-3" />
              <span>{activeSegmentIds.length} đang active</span>
            </span>
            <button
              type="button"
              data-testid="studio-export-button"
              onClick={() => setIsExportModalOpen(true)}
              className="flex items-center space-x-1 rounded bg-accent-neuro/20 hover:bg-accent-neuro/30 text-accent-neuro border border-accent-neuro/40 px-2.5 py-1 text-xs font-medium transition-colors"
            >
              <Share2 className="h-3.5 w-3.5" />
              <span>Xuất bản</span>
            </button>
          </div>
        </div>

        {/* Character Hotkeys (1-5) & Multi-Selection Toolbar */}
        <CharacterHotkeyBar />

        {/* Virtualized Subtitle Segment List */}
        <VirtualizedSegmentList
          segments={effectiveSegments}
          onSegmentSelect={handleSegmentClick}
          className="flex-1"
        />
      </div>

      <ExportModal
        isOpen={isExportModalOpen}
        onClose={() => setIsExportModalOpen(false)}
        project={currentProject}
        videoFilePath={propVideoSrc ?? undefined}
      />
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
