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
import { MessageSquare, Sparkles, Share2, Mic, Languages, RefreshCw } from 'lucide-react';
import { ErrorBoundary } from '../common/ErrorBoundary.tsx';
import { ErrorBanner } from '../common/ErrorBanner.tsx';
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
import { useTranscriptStore } from '../../stores/useTranscriptStore.ts';
import { useSettingsStore } from '../../stores/settingsStore.ts';
import type { TargetLayout } from '../../types/export.ts';
import type { TranscriptSegment, SubtitleProject } from '../../types/transcript.ts';
import { normalizeSubtitleProject } from '../../types/transcript.ts';

export type StudioSttEngine = 'groq' | 'faster_whisper' | 'whisper_cpp';

function sanitizeClipIdentifier(raw: string): string {
  const cleaned = raw
    .trim()
    .replace(/[^A-Za-z0-9_-]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .slice(0, 64);
  return cleaned.length > 0 ? cleaned : '';
}

export function deriveStudioClipId(clipId: string | null, videoPath: string | null): string {
  if (typeof clipId === 'string') {
    const clean = sanitizeClipIdentifier(clipId);
    if (clean.length > 0) {
      return clean;
    }
  }

  if (typeof videoPath === 'string' && videoPath.trim().length > 0) {
    const normalized = videoPath.trim().replace(/\\/g, '/');
    const filename = normalized.split('/').pop() ?? '';
    const stem = filename.replace(/\.[^/.]+$/, '');
    const cleanStem = sanitizeClipIdentifier(stem);
    if (cleanStem.length > 0) {
      return cleanStem;
    }
  }

  return 'clip_studio';
}

function formatSttStageLabel(stage: string): string {
  switch (stage) {
    case 'preparing':
      return 'Đang chuẩn bị âm thanh (FFmpeg)...';
    case 'vad':
      return 'Đang tách đoạn giọng nói (Silero VAD)...';
    case 'transcribing':
      return 'Đang bóc băng giọng nói...';
    case 'completed':
      return 'Hoàn tất bóc băng';
    default:
      return 'Đang bóc băng Whisper STT...';
  }
}

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

const ActiveSegmentCounter: React.FC = () => {
  const activeCount = useStudioStore((s) => s.activeSegmentIds.length);
  return (
    <span className="flex items-center space-x-1 font-mono text-[11px] text-accent-neuro">
      <Sparkles className="h-3 w-3" />
      <span>{activeCount} đang active</span>
    </span>
  );
};

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
  const duration = useStudioStore((s) => s.duration);
  const setVideoSrcInStore = useStudioStore((s) => s.setVideoSrc);
  const setSegmentsInStore = useStudioStore((s) => s.setSegments);

  // Transcript / STT & Translation store bindings
  const transcriptClipId = useTranscriptStore((s) => s.clipId);
  const isTranscribing = useTranscriptStore((s) => s.isTranscribing);
  const isTranslating = useTranscriptStore((s) => s.isTranslating);
  const sttProgress = useTranscriptStore((s) => s.progress);
  const translationProgress = useTranscriptStore((s) => s.translationProgress);
  const streamedChunks = useTranscriptStore((s) => s.streamedChunks);
  const transcriptError = useTranscriptStore((s) => s.error);
  const transcribeClip = useTranscriptStore((s) => s.transcribeClip);
  const transcribeFasterWhisper = useTranscriptStore((s) => s.transcribeFasterWhisper);
  const transcribeWhisperCpp = useTranscriptStore((s) => s.transcribe);
  const translateVietsub = useTranscriptStore((s) => s.translate);
  const setContainerSegments = useTranscriptStore((s) => s.setContainerSegments);
  const clearTranscriptError = useTranscriptStore((s) => s.clearError);

  // Configured defaults
  const activeSttProvider = useSettingsStore((s) => s.config.activeSttProvider);
  const activeTranslationModel = useSettingsStore((s) => s.config.activeTranslationModel);
  const [sttEngine, setSttEngine] = useState<StudioSttEngine>(() =>
    activeSttProvider === 'local_whisper' ? 'faster_whisper' : 'groq',
  );
  const [isMusicMode, setIsMusicMode] = useState<boolean>(false);

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

  useEffect(() => {
    if (propSegments === undefined && isTranscribing && streamedChunks.length > 0) {
      setSegmentsInStore(streamedChunks);
    }
  }, [propSegments, isTranscribing, streamedChunks, setSegmentsInStore]);

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

  const handleRunStt = async (): Promise<void> => {
    if (!effectiveVideoPath || isTranscribing || isTranslating) {
      return;
    }
    const targetClipId = deriveStudioClipId(transcriptClipId, effectiveVideoPath);
    try {
      const resultContainer =
        sttEngine === 'groq'
          ? await transcribeClip(targetClipId, effectiveVideoPath, { isMusicMode })
          : sttEngine === 'faster_whisper'
            ? await transcribeFasterWhisper(targetClipId, effectiveVideoPath)
            : await transcribeWhisperCpp(targetClipId, effectiveVideoPath);

      setSegmentsInStore(resultContainer.segments);
    } catch {
      // Error state captured in useTranscriptStore
    }
  };

  const handleRunTranslate = async (): Promise<void> => {
    if (effectiveSegments.length === 0 || isTranscribing || isTranslating) {
      return;
    }
    const targetClipId = deriveStudioClipId(transcriptClipId, effectiveVideoPath);
    setContainerSegments(targetClipId, effectiveSegments);
    try {
      const modelTrimmed = activeTranslationModel.trim();
      const updated = await translateVietsub(
        modelTrimmed.length > 0 ? { model: modelTrimmed } : undefined,
      );
      setSegmentsInStore(updated.segments);
    } catch {
      // Error state captured in useTranscriptStore
    }
  };

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
          <TimelineScrubber duration={duration} />
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
            <ActiveSegmentCounter />
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

        {/* AI Subtitle Pipeline Toolbar (Whisper STT & Gemini Vietsub) */}
        <div
          data-testid="studio-ai-pipeline-bar"
          className="space-y-2 border-b border-border-dark bg-bg-primary/60 px-4 py-2.5"
        >
          <div className="flex flex-wrap items-center justify-between gap-2">
            <select
              aria-label="Bộ máy Whisper STT"
              data-testid="studio-stt-engine-select"
              value={sttEngine}
              onChange={(e) => setSttEngine(e.target.value as StudioSttEngine)}
              disabled={isTranscribing || isTranslating}
              className="rounded border border-border-dark bg-bg-secondary px-2 py-1 text-xs text-slate-200 outline-none focus:border-accent-neuro disabled:opacity-50"
            >
              <option value="groq">Groq Whisper (Cloud)</option>
              <option value="faster_whisper">Faster-Whisper (Local INT8)</option>
              <option value="whisper_cpp">Whisper.cpp (Local)</option>
            </select>

            {sttEngine === 'groq' && (
              <label className="flex cursor-pointer items-center space-x-1.5 text-[11px] text-slate-300">
                <input
                  type="checkbox"
                  data-testid="studio-music-mode-toggle"
                  checked={isMusicMode}
                  onChange={(e) => setIsMusicMode(e.target.checked)}
                  disabled={isTranscribing || isTranslating}
                  className="rounded border-border-dark bg-bg-secondary accent-accent-neuro"
                />
                <span>Nhạc / Karaoke</span>
              </label>
            )}
          </div>

          <div className="flex items-center gap-2">
            <button
              type="button"
              data-testid="studio-transcribe-button"
              disabled={!effectiveVideoPath || isTranscribing || isTranslating}
              onClick={() => {
                void handleRunStt();
              }}
              title={
                effectiveVideoPath
                  ? 'Bóc băng âm thanh clip bằng Whisper STT'
                  : 'Cần cắt phân đoạn video hoặc nạp tệp clip cục bộ trước khi chạy Whisper STT'
              }
              className="flex flex-1 items-center justify-center space-x-1.5 rounded border border-accent-vedal/40 bg-accent-vedal/20 px-2.5 py-1.5 text-xs font-medium text-accent-vedal transition-colors hover:bg-accent-vedal/30 disabled:cursor-not-allowed disabled:opacity-50"
            >
              <Mic className="h-3.5 w-3.5" />
              <span>{isTranscribing ? 'Đang bóc băng...' : 'Bóc băng (Whisper STT)'}</span>
            </button>

            <button
              type="button"
              data-testid="studio-translate-button"
              disabled={effectiveSegments.length === 0 || isTranscribing || isTranslating}
              onClick={() => {
                void handleRunTranslate();
              }}
              title="Dịch phụ đề sang tiếng Việt bằng Gemini (Theatrical Framing)"
              className="flex flex-1 items-center justify-center space-x-1.5 rounded border border-accent-chat/40 bg-accent-chat/20 px-2.5 py-1.5 text-xs font-medium text-accent-chat transition-colors hover:bg-accent-chat/30 disabled:cursor-not-allowed disabled:opacity-50"
            >
              <Languages className="h-3.5 w-3.5" />
              <span>{isTranslating ? 'Đang dịch Vietsub...' : 'Dịch Vietsub (Gemini)'}</span>
            </button>
          </div>

          {!effectiveVideoPath && (
            <p className="text-[11px] text-slate-400">
              Cắt một phân đoạn ở tab Highlight để nạp video clip cho Whisper STT.
            </p>
          )}

          {isTranscribing && sttProgress && (
            <div data-testid="studio-stt-progress" className="space-y-1">
              <div className="flex items-center justify-between text-[11px] text-slate-300">
                <span className="flex items-center space-x-1.5">
                  <RefreshCw className="h-3 w-3 animate-spin text-accent-vedal" />
                  <span>{formatSttStageLabel(sttProgress.stage)}</span>
                </span>
                <span className="font-mono text-accent-vedal">
                  {Math.round(sttProgress.percent)}%
                  {sttProgress.totalChunks > 1
                    ? ` (${sttProgress.currentChunk}/${sttProgress.totalChunks})`
                    : ''}
                </span>
              </div>
              <div className="h-1 w-full overflow-hidden rounded-full bg-bg-secondary">
                <div
                  className="h-full bg-accent-vedal transition-all duration-300"
                  style={{ width: `${Math.min(100, Math.max(0, sttProgress.percent))}%` }}
                />
              </div>
            </div>
          )}

          {isTranslating && (
            <div
              data-testid="studio-translation-progress"
              className="flex items-center justify-between text-[11px] text-accent-chat"
            >
              <span className="flex items-center space-x-1.5">
                <RefreshCw className="h-3 w-3 animate-spin" />
                <span>Đang dịch Vietsub ngữ cảnh (Gemini)...</span>
              </span>
              {translationProgress && (
                <span className="font-mono">
                  {translationProgress.translatedCount}/{translationProgress.totalCount}
                </span>
              )}
            </div>
          )}

          {transcriptError && (
            <ErrorBanner
              error={transcriptError}
              onDismiss={clearTranscriptError}
              title="Lỗi xử lý phụ đề AI"
            />
          )}
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
