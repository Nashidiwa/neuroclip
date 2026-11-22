/**
 * NeuroClip Subtitle Studio Video Canvas
 *
 * HTML5 video player component supporting Tauri `asset://` URI scheme,
 * dual 9:16 vertical presets ('center-crop' and 'two-stack' 50/50 per ADR-005),
 * instant preset switching (<= 50ms without video reloading), and stacked
 * subtitle overlay with character-specific PO Option 1 colors.
 *
 * Strict adherence to docs/CONVENTIONS.md (TypeScript strict mode, zero any, strict null safety).
 */

import React, { useRef, useEffect, useMemo, useCallback } from 'react';
import { Film } from 'lucide-react';
import {
  useStudioStore,
  type Preset916,
  type AspectRatioMode,
  getSpeakerColor,
} from '../../stores/useStudioStore.ts';
import type { TranscriptSegment } from '../../types/transcript.ts';
import { toLocalAssetUrl } from '../../services/ipc/ingestion.ts';

export interface VideoCanvasProps {
  readonly videoSrc?: string | null | undefined;
  readonly preset916?: Preset916 | undefined;
  readonly aspectRatio?: AspectRatioMode | undefined;
  readonly activeSegments?: readonly TranscriptSegment[] | undefined;
  readonly currentTime?: number | undefined;
  readonly isPlaying?: boolean | undefined;
  readonly volume?: number | undefined;
  readonly isMuted?: boolean | undefined;
  readonly playbackRate?: number | undefined;
  readonly onTimeUpdate?: ((time: number) => void) | undefined;
  readonly onDurationChange?: ((duration: number) => void) | undefined;
  readonly onEnded?: (() => void) | undefined;
  readonly onPlay?: (() => void) | undefined;
  readonly onPause?: (() => void) | undefined;
  readonly className?: string | undefined;
}

export const VideoCanvas: React.FC<VideoCanvasProps> = ({
  videoSrc: propVideoSrc,
  preset916: propPreset916,
  aspectRatio: propAspectRatio,
  activeSegments: propActiveSegments,
  currentTime: propCurrentTime,
  isPlaying: propIsPlaying,
  volume: propVolume,
  isMuted: propIsMuted,
  playbackRate: propPlaybackRate,
  onTimeUpdate,
  onDurationChange,
  onEnded,
  onPlay,
  onPause,
  className = '',
}) => {
  const primaryVideoRef = useRef<HTMLVideoElement | null>(null);
  const secondaryVideoRef = useRef<HTMLVideoElement | null>(null);

  // Store subscriptions (fallback when props are not provided)
  const storeVideoSrc = useStudioStore((s) => s.videoSrc);
  const storePreset916 = useStudioStore((s) => s.preset916);
  const storeAspectRatio = useStudioStore((s) => s.aspectRatio);
  const storeActiveSegments = useStudioStore((s) => s.activeSegments);
  const storeIsPlaying = useStudioStore((s) => s.isPlaying);
  const storeVolume = useStudioStore((s) => s.volume);
  const storeIsMuted = useStudioStore((s) => s.isMuted);
  const storePlaybackRate = useStudioStore((s) => s.playbackRate);
  const seekTargetTime = useStudioStore((s) => s.seekTargetTime);
  const clearSeekTarget = useStudioStore((s) => s.clearSeekTarget);
  const setCurrentTimeInStore = useStudioStore((s) => s.setCurrentTime);
  const setDurationInStore = useStudioStore((s) => s.setDuration);
  const setIsPlayingInStore = useStudioStore((s) => s.setIsPlaying);

  // Effective state resolution
  const rawVideoSrc = propVideoSrc !== undefined ? propVideoSrc : storeVideoSrc;
  const effectivePreset: Preset916 = propPreset916 ?? storePreset916;
  const effectiveAspectRatio: AspectRatioMode = propAspectRatio ?? storeAspectRatio;
  const effectiveActiveSegments = propActiveSegments ?? storeActiveSegments;
  const effectiveIsPlaying = propIsPlaying !== undefined ? propIsPlaying : storeIsPlaying;
  const effectiveVolume = propVolume !== undefined ? propVolume : storeVolume;
  const effectiveIsMuted = propIsMuted !== undefined ? propIsMuted : storeIsMuted;
  const effectivePlaybackRate =
    propPlaybackRate !== undefined ? propPlaybackRate : storePlaybackRate;

  // Synchronize controlled currentTime prop if provided
  useEffect(() => {
    if (propCurrentTime === undefined) return;
    const video = primaryVideoRef.current;
    if (video && Math.abs(video.currentTime - propCurrentTime) > 0.05) {
      video.currentTime = propCurrentTime;
    }
  }, [propCurrentTime]);

  // Convert local file paths to Tauri asset:// streaming URL if needed
  const resolvedVideoUrl = useMemo<string | null>(() => {
    if (!rawVideoSrc) return null;
    const trimmed = rawVideoSrc.trim();
    if (!trimmed) return null;

    if (
      trimmed.startsWith('asset://') ||
      trimmed.startsWith('http://') ||
      trimmed.startsWith('https://') ||
      trimmed.startsWith('blob:') ||
      trimmed.startsWith('data:')
    ) {
      return trimmed;
    }

    // Convert local Windows file path (e.g. C:\...) to asset:// protocol
    const assetUrl = toLocalAssetUrl(trimmed);
    return assetUrl || trimmed;
  }, [rawVideoSrc]);

  // Synchronize playback play/pause state
  useEffect(() => {
    const video = primaryVideoRef.current;
    if (!video) return;

    if (effectiveIsPlaying) {
      const playPromise = video.play();
      if (playPromise !== undefined) {
        playPromise.catch(() => {
          // Ignore autoplay restriction or abortion during rapid seek/tests
        });
      }
    } else {
      video.pause();
    }
  }, [effectiveIsPlaying]);

  // Synchronize volume and mute
  useEffect(() => {
    const video = primaryVideoRef.current;
    if (!video) return;
    video.volume = effectiveVolume;
    video.muted = effectiveIsMuted;
  }, [effectiveVolume, effectiveIsMuted]);

  // Synchronize playback speed
  useEffect(() => {
    const video = primaryVideoRef.current;
    if (!video) return;
    video.playbackRate = effectivePlaybackRate;
  }, [effectivePlaybackRate]);

  // Synchronize seek target time from store actions
  useEffect(() => {
    if (seekTargetTime === null) return;
    const video = primaryVideoRef.current;
    if (video && Math.abs(video.currentTime - seekTargetTime) > 0.05) {
      video.currentTime = seekTargetTime;
    }
    const secVideo = secondaryVideoRef.current;
    if (secVideo && Math.abs(secVideo.currentTime - seekTargetTime) > 0.05) {
      secVideo.currentTime = seekTargetTime;
    }
    clearSeekTarget();
  }, [seekTargetTime, clearSeekTarget]);

  // Secondary video sync loop for two-stack preset
  useEffect(() => {
    if (effectivePreset !== 'two-stack') return;
    const prim = primaryVideoRef.current;
    const sec = secondaryVideoRef.current;
    if (!prim || !sec) return;

    sec.muted = true;
    if (effectiveIsPlaying) {
      sec.play().catch(() => {});
    } else {
      sec.pause();
    }
  }, [effectivePreset, effectiveIsPlaying]);

  // Video event handlers
  const handleTimeUpdate = useCallback(() => {
    const video = primaryVideoRef.current;
    if (!video) return;
    const t = video.currentTime;

    if (onTimeUpdate) {
      onTimeUpdate(t);
    } else {
      setCurrentTimeInStore(t);
    }

    // Keep secondary video in sync if active
    const secVideo = secondaryVideoRef.current;
    if (secVideo && Math.abs(secVideo.currentTime - t) > 0.1) {
      secVideo.currentTime = t;
    }
  }, [onTimeUpdate, setCurrentTimeInStore]);

  const handleDurationChange = useCallback(() => {
    const video = primaryVideoRef.current;
    if (!video) return;
    const d = video.duration;
    if (Number.isFinite(d)) {
      if (onDurationChange) {
        onDurationChange(d);
      } else {
        setDurationInStore(d);
      }
    }
  }, [onDurationChange, setDurationInStore]);

  const handlePlay = useCallback(() => {
    if (onPlay) {
      onPlay();
    } else {
      setIsPlayingInStore(true);
    }
  }, [onPlay, setIsPlayingInStore]);

  const handlePause = useCallback(() => {
    if (onPause) {
      onPause();
    } else {
      setIsPlayingInStore(false);
    }
  }, [onPause, setIsPlayingInStore]);

  const handleEnded = useCallback(() => {
    if (onEnded) {
      onEnded();
    } else {
      setIsPlayingInStore(false);
    }
  }, [onEnded, setIsPlayingInStore]);

  // Preset & Aspect Ratio Container Styles
  const containerAspectClass =
    effectiveAspectRatio === '16:9'
      ? 'aspect-[16/9] w-full max-h-[70vh]'
      : 'aspect-[9/16] max-h-[70vh] w-auto';

  if (!resolvedVideoUrl) {
    return (
      <div
        data-testid="video-canvas-container"
        data-preset={effectivePreset}
        data-aspect-ratio={effectiveAspectRatio}
        className={`relative flex items-center justify-center rounded-xl border border-border-dark bg-bg-secondary/60 shadow-acrylic ${containerAspectClass} ${className}`}
      >
        <div
          data-testid="video-empty-state"
          className="flex flex-col items-center justify-center space-y-3 p-8 text-center"
        >
          <div className="flex h-14 w-14 items-center justify-center rounded-full border border-border-dark bg-bg-tertiary text-slate-400">
            <Film className="h-7 w-7 text-accent-neuro/70" />
          </div>
          <div className="space-y-1">
            <p className="text-sm font-semibold text-slate-200">Chưa có video được nạp</p>
            <p className="max-w-[240px] text-xs text-slate-400">
              Vui lòng chọn video slice hoặc nạp clip highlight để bắt đầu biên tập
            </p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div
      data-testid="video-canvas-container"
      data-preset={effectivePreset}
      data-aspect-ratio={effectiveAspectRatio}
      className={`relative mx-auto flex select-none flex-col overflow-hidden rounded-xl border border-border-dark bg-black shadow-acrylic ${containerAspectClass} ${className}`}
    >
      {/* Primary Video Pane (Top half in two-stack, full container in center-crop) */}
      <div
        data-testid="video-primary-pane"
        className={`relative w-full overflow-hidden transition-all duration-75 ${
          effectivePreset === 'two-stack' && effectiveAspectRatio === '9:16'
            ? 'h-1/2'
            : 'h-full flex-1'
        }`}
      >
        <video
          ref={primaryVideoRef}
          data-testid="studio-video-element"
          src={resolvedVideoUrl}
          playsInline
          onTimeUpdate={handleTimeUpdate}
          onDurationChange={handleDurationChange}
          onLoadedMetadata={handleDurationChange}
          onPlay={handlePlay}
          onPause={handlePause}
          onEnded={handleEnded}
          className={`absolute inset-0 h-full w-full ${
            effectivePreset === 'two-stack' && effectiveAspectRatio === '9:16'
              ? 'object-cover object-top'
              : 'object-cover object-center'
          }`}
        />
      </div>

      {/* Preset 2: Two-stack Top-Bottom 50/50 Divider and Secondary Pane */}
      {effectivePreset === 'two-stack' && effectiveAspectRatio === '9:16' && (
        <>
          {/* Preset 2 Divider with accent styling */}
          <div
            data-testid="preset2-divider"
            className="relative z-10 flex h-[3px] w-full shrink-0 items-center justify-center bg-accent-neuro shadow-sm"
          >
            <div className="h-1 w-8 rounded-full bg-white/80" />
          </div>

          {/* Secondary Video Pane (Avatar Bottom half) */}
          <div
            data-testid="video-secondary-pane"
            className="relative h-1/2 w-full overflow-hidden bg-black/60"
          >
            <video
              ref={secondaryVideoRef}
              data-testid="studio-secondary-video-element"
              src={resolvedVideoUrl}
              playsInline
              muted
              aria-hidden="true"
              tabIndex={-1}
              className="pointer-events-none absolute inset-0 h-full w-full object-cover object-bottom"
            />
          </div>
        </>
      )}

      {/* Stacked Subtitles Overlay */}
      {effectiveActiveSegments.length > 0 && (
        <div
          data-testid="subtitle-overlay"
          className="pointer-events-none absolute inset-x-3 bottom-6 z-20 flex flex-col items-center gap-1.5"
        >
          {effectiveActiveSegments.map((segment) => {
            const segId = segment.segmentId ?? segment.segment_id;
            const speaker = segment.speakerId ?? segment.speaker_id;
            const speakerColor = getSpeakerColor(speaker);
            const text = segment.textVi ?? segment.text_vi ?? segment.textEn ?? segment.text_en;

            return (
              <div
                key={segId}
                data-testid={`subtitle-line-${segId}`}
                className="backdrop-blur-xs max-w-[96%] rounded-md bg-black/40 px-2.5 py-1 text-center transition-opacity duration-100"
              >
                <span
                  style={{
                    color: speakerColor,
                    textShadow:
                      '2px 2px 0 #000, -2px -2px 0 #000, 2px -2px 0 #000, -2px 2px 0 #000, 0 2px 4px rgba(0,0,0,0.9)',
                  }}
                  className="font-sans text-sm font-bold leading-snug tracking-wide sm:text-base"
                >
                  <span className="mr-1.5 text-xs uppercase tracking-wider opacity-80">
                    [{speaker}]
                  </span>
                  {text}
                </span>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};
