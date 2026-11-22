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
import {
  useStudioStore,
  type AspectRatioMode,
  type Preset916,
} from '../../stores/useStudioStore.ts';
import type { TargetLayout } from '../../types/export.ts';
import type { TranscriptSegment, SubtitleProject } from '../../types/transcript.ts';
import { normalizeSubtitleProject } from '../../types/transcript.ts';

const NON_FILESYSTEM_URI_PREFIXES = ['asset://', 'http://', 'https://', 'blob:', 'data:'] as const;

export function isNonFilesystemUri(value: string): boolean {
  const normalized = value.trim().toLowerCase();
  return NON_FILESYSTEM_URI_PREFIXES.some((prefix) => normalized.startsWith(prefix));
}

export function normalizeFilesystemPath(candidate: string | null | undefined): string | null {
  if (typeof candidate !== 'string') {
    return null;
  }
  const trimmed = candidate.trim();
  if (trimmed.length === 0 || isNonFilesystemUri(trimmed)) {
    return null;
  }
  return trimmed;
}

export function mapStudioLayoutToTargetLayout(
  aspectRatio: AspectRatioMode,
  preset916: Preset916,
): TargetLayout {
  if (aspectRatio === '16:9') {
    return 'original_16_9';
  }
  return preset916 === 'two-stack' ? 'preset_two_stack_9_16' : 'preset_center_crop_9_16';
}

export function resolveStudioVideoPath(options: {
  readonly propVideoPath: string | null | undefined;
  readonly propVideoSrc: string | null | undefined;
  readonly storeVideoPath: string | null;
  readonly storeVideoSrc: string | null;
}): string | null {
  const { propVideoPath, propVideoSrc, storeVideoPath, storeVideoSrc } = options;

  if (propVideoPath !== undefined) {
    return normalizeFilesystemPath(propVideoPath);
  }

  if (propVideoSrc !== undefined) {
    if (propVideoSrc === storeVideoSrc) {
      const matchingStorePath = normalizeFilesystemPath(storeVideoPath);
      if (matchingStorePath !== null) {
        return matchingStorePath;
      }
    }
    return normalizeFilesystemPath(propVideoSrc);
  }

  return normalizeFilesystemPath(storeVideoPath) ?? normalizeFilesystemPath(storeVideoSrc);
}

export interface SubtitleStudioProps {
  readonly videoSrc?: string | null | undefined;
  readonly videoPath?: string | null | undefined;
  readonly segments?: readonly TranscriptSegment[] | undefined;
  readonly onSegmentSelect?: ((segment: TranscriptSegment) => void) | undefined;
  readonly className?: string | undefined;
}

export const SubtitleStudioInner: React.FC<SubtitleStudioProps> = ({
  videoSrc: propVideoSrc,
  videoPath: propVideoPath,
  segments: propSegments,
  onSegmentSelect,
  className = '',
}) => {
  // Global character hotkeys (1-5, Numpad 1-5)
  useStudioHotkeys();

  // Store bindings
  const storeVideoSrc = useStudioStore((s) => s.videoSrc);
  const storeVideoPath = useStudioStore((s) => s.videoPath);
  const aspectRatio = useStudioStore((s) => s.aspectRatio);
  const preset916 = useStudioStore((s) => s.preset916);
  const storeSegments = useStudioStore((s) => s.segments);
  const activeSegmentIds = useStudioStore((s) => s.activeSegmentIds);
  const currentTime = useStudioStore((s) => s.currentTime);
  const duration = useStudioStore((s) => s.duration);
  const setVideoSrcInStore = useStudioStore((s) => s.setVideoSrc);
  const setSegmentsInStore = useStudioStore((s) => s.setSegments);

  // Synchronize incoming props to store if provided (preserving store.videoPath when unchanged)
  useEffect(() => {
    if (propVideoSrc !== undefined) {
      const currentState = useStudioStore.getState();
      if (propVideoPath !== undefined) {
        setVideoSrcInStore(propVideoSrc, propVideoPath);
      } else if (propVideoSrc !== currentState.videoSrc) {
        setVideoSrcInStore(propVideoSrc);
      }
    }
  }, [propVideoSrc, propVideoPath, setVideoSrcInStore]);

  useEffect(() => {
    if (propSegments !== undefined) {
      setSegmentsInStore(propSegments);
    }
  }, [propSegments, setSegmentsInStore]);

  const effectiveSegments = propSegments !== undefined ? propSegments : storeSegments;
  const effectiveVideoSrc =
    propVideoSrc !== undefined ? propVideoSrc : (storeVideoSrc ?? propVideoPath ?? null);
  const effectiveVideoPath = resolveStudioVideoPath({
    propVideoPath,
    propVideoSrc,
    storeVideoPath,
    storeVideoSrc,
  });
  const targetLayout = mapStudioLayoutToTargetLayout(aspectRatio, preset916);
  const [isExportModalOpen, setIsExportModalOpen] = useState<boolean>(false);

  const currentProject: SubtitleProject = useMemo(() => {
    return normalizeSubtitleProject({
      project_id: 'subproj_studio_export',
      project_name: 'NeuroClip_Highlight',
      vod_id: 'vod_current',
      video_file_path: effectiveVideoPath,
      target_layout: targetLayout,
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
        neuro: {
          color_hex: '#FFB6C1',
          font_name: 'Arial',
          font_size: 54,
          outline_color_hex: '#000000',
        },
        vedal: {
          color_hex: '#98FB98',
          font_name: 'Arial',
          font_size: 54,
          outline_color_hex: '#000000',
        },
        evil: {
          color_hex: '#FF0055',
          font_name: 'Arial',
          font_size: 54,
          outline_color_hex: '#000000',
        },
        guest: {
          color_hex: '#FFD700',
          font_name: 'Arial',
          font_size: 54,
          outline_color_hex: '#000000',
        },
        chat: {
          color_hex: '#87CEEB',
          font_name: 'Arial',
          font_size: 48,
          outline_color_hex: '#000000',
        },
        unknown: {
          color_hex: '#FFFFFF',
          font_name: 'Arial',
          font_size: 52,
          outline_color_hex: '#000000',
        },
      },
      updated_at: new Date().toISOString(),
    });
  }, [effectiveSegments, effectiveVideoPath, targetLayout]);

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
      data-target-layout={targetLayout}
      className={`flex h-full w-full select-none flex-col overflow-hidden bg-bg-primary text-white lg:flex-row ${className}`}
    >
      {/* Left Column: Video Preview Canvas & Timeline Scrubber */}
      <div className="flex flex-1 flex-col items-center justify-between overflow-y-auto border-b border-border-dark bg-bg-secondary/30 p-4 lg:border-b-0 lg:border-r">
        <div className="my-auto flex w-full flex-1 flex-col items-center justify-center">
          <VideoCanvas videoSrc={effectiveVideoSrc} />
        </div>

        <div className="w-full max-w-3xl pt-4">
          <TimelineScrubber currentTime={currentTime} duration={duration} />
        </div>
      </div>

      {/* Right Column: Subtitle Segment List Panel */}
      <div className="flex w-full shrink-0 flex-col overflow-hidden border-border-dark bg-bg-secondary/70 lg:w-[460px]">
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
              className="flex items-center space-x-1 rounded border border-accent-neuro/40 bg-accent-neuro/20 px-2.5 py-1 text-xs font-medium text-accent-neuro transition-colors hover:bg-accent-neuro/30"
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
        videoFilePath={effectiveVideoPath ?? undefined}
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
