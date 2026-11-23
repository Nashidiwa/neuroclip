/**
 * NeuroClip Subtitle Studio Timeline Scrubber
 *
 * Precision timeline scrubber with requestAnimationFrame throttling (ED-20.1),
 * sub-second millisecond timecode display (MM:SS.mmm), transport controls
 * (play/pause, volume, mute, speed, frame nudge), and 9:16 preset selectors.
 *
 * Strict adherence to docs/CONVENTIONS.md (TypeScript strict mode, zero any, strict null safety).
 */

import React, { useState, useEffect, useRef, useCallback } from 'react';
import {
  Play,
  Pause,
  RotateCcw,
  RotateCw,
  Volume2,
  VolumeX,
  FastForward,
  Rewind,
  Crop,
  Layers,
} from 'lucide-react';
import {
  useStudioStore,
  formatStudioTimecode,
  type Preset916,
  type AspectRatioMode,
} from '../../stores/useStudioStore.ts';

export interface TimelineScrubberProps {
  readonly currentTime?: number | undefined;
  readonly duration?: number | undefined;
  readonly isPlaying?: boolean | undefined;
  readonly isMuted?: boolean | undefined;
  readonly volume?: number | undefined;
  readonly playbackRate?: number | undefined;
  readonly preset916?: Preset916 | undefined;
  readonly aspectRatio?: AspectRatioMode | undefined;
  readonly onSeek?: ((time: number) => void) | undefined;
  readonly onTogglePlay?: (() => void) | undefined;
  readonly onToggleMute?: (() => void) | undefined;
  readonly onVolumeChange?: ((volume: number) => void) | undefined;
  readonly onPresetChange?: ((preset: Preset916) => void) | undefined;
  readonly onAspectRatioChange?: ((ratio: AspectRatioMode) => void) | undefined;
  readonly className?: string | undefined;
}

const SPEED_OPTIONS = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0] as const;

interface TimelineProgressTrackProps {
  readonly propCurrentTime: number | undefined;
  readonly effectiveDuration: number;
  readonly onSeek: ((time: number) => void) | undefined;
}

const TimelineProgressTrack: React.FC<TimelineProgressTrackProps> = ({
  propCurrentTime,
  effectiveDuration,
  onSeek,
}) => {
  const storeCurrentTime = useStudioStore((s) =>
    propCurrentTime !== undefined ? propCurrentTime : s.currentTime,
  );
  const seekToStore = useStudioStore((s) => s.seekTo);
  const setIsScrubbingStore = useStudioStore((s) => s.setIsScrubbing);

  const effectiveCurrentTime = propCurrentTime !== undefined ? propCurrentTime : storeCurrentTime;
  const [isLocalScrubbing, setIsLocalScrubbing] = useState<boolean>(false);
  const [scrubTime, setScrubTime] = useState<number>(0);
  const isActiveScrubRef = useRef<boolean>(false);
  const scrubTimeRef = useRef<number>(effectiveCurrentTime);
  const rafHandleRef = useRef<number | null>(null);

  const displayTime = isLocalScrubbing ? scrubTime : effectiveCurrentTime;
  const forceHours = effectiveDuration >= 3600;

  useEffect(() => {
    return () => {
      if (rafHandleRef.current !== null) {
        cancelAnimationFrame(rafHandleRef.current);
        rafHandleRef.current = null;
      }
    };
  }, []);

  const dispatchSeekThrottled = useCallback(
    (targetTime: number) => {
      if (rafHandleRef.current !== null) {
        cancelAnimationFrame(rafHandleRef.current);
      }

      rafHandleRef.current = requestAnimationFrame(() => {
        if (onSeek) {
          onSeek(targetTime);
        } else {
          seekToStore(targetTime);
        }
        rafHandleRef.current = null;
      });
    },
    [onSeek, seekToStore],
  );

  const dispatchSeekImmediate = useCallback(
    (targetTime: number) => {
      if (rafHandleRef.current !== null) {
        cancelAnimationFrame(rafHandleRef.current);
        rafHandleRef.current = null;
      }

      const clamped = Math.max(
        0,
        Math.min(effectiveDuration > 0 ? effectiveDuration : targetTime, targetTime),
      );
      if (onSeek) {
        onSeek(clamped);
      } else {
        seekToStore(clamped);
      }
    },
    [effectiveDuration, onSeek, seekToStore],
  );

  const handleSliderPointerDown = useCallback(() => {
    isActiveScrubRef.current = true;
    scrubTimeRef.current = effectiveCurrentTime;
    setScrubTime(effectiveCurrentTime);
    setIsLocalScrubbing(true);
    setIsScrubbingStore(true);
  }, [effectiveCurrentTime, setIsScrubbingStore]);

  const handleSliderChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const val = parseFloat(e.target.value);
      if (Number.isFinite(val)) {
        scrubTimeRef.current = val;
        setScrubTime(val);
        dispatchSeekThrottled(val);
      }
    },
    [dispatchSeekThrottled],
  );

  const handleSliderPointerUp = useCallback(() => {
    const targetTime = isActiveScrubRef.current ? scrubTimeRef.current : effectiveCurrentTime;
    isActiveScrubRef.current = false;
    setIsLocalScrubbing(false);
    setIsScrubbingStore(false);
    dispatchSeekImmediate(targetTime);
  }, [dispatchSeekImmediate, effectiveCurrentTime, setIsScrubbingStore]);

  return (
    <div className="flex flex-col space-y-1">
      <div className="relative flex items-center">
        <input
          type="range"
          data-testid="timeline-slider"
          min={0}
          max={effectiveDuration > 0 ? effectiveDuration : 100}
          step={0.01}
          value={displayTime}
          onPointerDown={handleSliderPointerDown}
          onChange={handleSliderChange}
          onPointerUp={handleSliderPointerUp}
          disabled={effectiveDuration <= 0}
          aria-label="Thanh tua thời gian video"
          className="focus:outline-hidden h-2 w-full cursor-pointer appearance-none rounded-lg bg-bg-tertiary accent-accent-neuro disabled:opacity-50"
        />
      </div>

      {/* Timecode and Progress display */}
      <div className="flex items-center justify-between font-mono text-xs text-slate-300">
        <span data-testid="timeline-timecode" className="font-semibold text-accent-neuro">
          {formatStudioTimecode(displayTime, forceHours)} /{' '}
          {formatStudioTimecode(effectiveDuration, forceHours)}
        </span>
        <span className="text-[11px] text-slate-400">
          {effectiveDuration > 0
            ? `${Math.min(100, Math.max(0, (displayTime / effectiveDuration) * 100)).toFixed(1)}%`
            : '0.0%'}
        </span>
      </div>
    </div>
  );
};

export const TimelineScrubber: React.FC<TimelineScrubberProps> = ({
  currentTime: propCurrentTime,
  duration: propDuration,
  isPlaying: propIsPlaying,
  isMuted: propIsMuted,
  volume: propVolume,
  playbackRate: propPlaybackRate,
  preset916: propPreset916,
  aspectRatio: propAspectRatio,
  onSeek,
  onTogglePlay,
  onToggleMute,
  onVolumeChange,
  onPresetChange,
  onAspectRatioChange,
  className = '',
}) => {
  // Store subscriptions (fallback if props are not provided; currentTime lives in TimelineProgressTrack)
  const storeDuration = useStudioStore((s) =>
    propDuration !== undefined ? propDuration : s.duration,
  );
  const storeIsPlaying = useStudioStore((s) => s.isPlaying);
  const storeIsMuted = useStudioStore((s) => s.isMuted);
  const storeVolume = useStudioStore((s) => s.volume);
  const storePlaybackRate = useStudioStore((s) => s.playbackRate);
  const storePreset916 = useStudioStore((s) => s.preset916);
  const storeAspectRatio = useStudioStore((s) => s.aspectRatio);

  const seekToStore = useStudioStore((s) => s.seekTo);
  const togglePlayStore = useStudioStore((s) => s.togglePlay);
  const toggleMuteStore = useStudioStore((s) => s.toggleMute);
  const setVolumeStore = useStudioStore((s) => s.setVolume);
  const setPreset916Store = useStudioStore((s) => s.setPreset916);
  const setAspectRatioStore = useStudioStore((s) => s.setAspectRatio);
  const setPlaybackRateStore = useStudioStore((s) => s.setPlaybackRate);

  // Effective values
  const effectiveDuration = propDuration !== undefined ? propDuration : storeDuration;
  const effectiveIsPlaying = propIsPlaying !== undefined ? propIsPlaying : storeIsPlaying;
  const effectiveIsMuted = propIsMuted !== undefined ? propIsMuted : storeIsMuted;
  const effectiveVolume = propVolume !== undefined ? propVolume : storeVolume;
  const effectivePlaybackRate =
    propPlaybackRate !== undefined ? propPlaybackRate : storePlaybackRate;
  const effectivePreset = propPreset916 ?? storePreset916;
  const effectiveAspectRatio = propAspectRatio ?? storeAspectRatio;

  const handleTogglePlay = useCallback(() => {
    if (onTogglePlay) {
      onTogglePlay();
    } else {
      togglePlayStore();
    }
  }, [onTogglePlay, togglePlayStore]);

  const handleToggleMute = useCallback(() => {
    if (onToggleMute) {
      onToggleMute();
    } else {
      toggleMuteStore();
    }
  }, [onToggleMute, toggleMuteStore]);

  const handleVolumeChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const val = parseFloat(e.target.value);
      if (Number.isFinite(val)) {
        if (onVolumeChange) {
          onVolumeChange(val);
        } else {
          setVolumeStore(val);
        }
      }
    },
    [onVolumeChange, setVolumeStore],
  );

  const handlePresetSelect = useCallback(
    (preset: Preset916) => {
      if (onPresetChange) {
        onPresetChange(preset);
      } else {
        setPreset916Store(preset);
      }
    },
    [onPresetChange, setPreset916Store],
  );

  const handleAspectRatioToggle = useCallback(() => {
    const nextRatio: AspectRatioMode = effectiveAspectRatio === '9:16' ? '16:9' : '9:16';
    if (onAspectRatioChange) {
      onAspectRatioChange(nextRatio);
    } else {
      setAspectRatioStore(nextRatio);
    }
  }, [effectiveAspectRatio, onAspectRatioChange, setAspectRatioStore]);

  const handleNudge = useCallback(
    (deltaSec: number) => {
      const baseTime =
        propCurrentTime !== undefined ? propCurrentTime : useStudioStore.getState().currentTime;
      const nextTime = Math.max(0, Math.min(effectiveDuration, baseTime + deltaSec));
      const clamped = Math.max(
        0,
        Math.min(effectiveDuration > 0 ? effectiveDuration : nextTime, nextTime),
      );
      if (onSeek) {
        onSeek(clamped);
      } else {
        seekToStore(clamped);
      }
    },
    [effectiveDuration, onSeek, propCurrentTime, seekToStore],
  );

  const handleSpeedSelect = useCallback(
    (e: React.ChangeEvent<HTMLSelectElement>) => {
      const rate = parseFloat(e.target.value);
      if (Number.isFinite(rate)) {
        setPlaybackRateStore(rate);
      }
    },
    [setPlaybackRateStore],
  );

  return (
    <div
      data-testid="timeline-scrubber"
      className={`flex w-full select-none flex-col space-y-2 rounded-xl border border-border-dark bg-bg-secondary/70 p-3 shadow-acrylic ${className}`}
    >
      {/* Top Bar: Timeline Scrubber Slider Track */}
      <TimelineProgressTrack
        propCurrentTime={propCurrentTime}
        effectiveDuration={effectiveDuration}
        onSeek={onSeek}
      />

      {/* Bottom Bar: Transport Controls & Preset Selection */}
      <div className="flex flex-wrap items-center justify-between gap-2 border-t border-border-dark/60 pt-1">
        {/* Left: Playback & Nudge Controls */}
        <div className="flex items-center space-x-1">
          {/* Play/Pause Button */}
          <button
            type="button"
            data-testid="timeline-play-btn"
            onClick={handleTogglePlay}
            aria-label={effectiveIsPlaying ? 'Tạm dừng video' : 'Phát video'}
            className="focus:outline-hidden flex h-8 w-8 items-center justify-center rounded-lg bg-accent-neuro/20 text-accent-neuro transition-colors hover:bg-accent-neuro/30"
          >
            {effectiveIsPlaying ? (
              <Pause className="h-4 w-4" />
            ) : (
              <Play className="ml-0.5 h-4 w-4" />
            )}
          </button>

          {/* Jump -5s */}
          <button
            type="button"
            data-testid="timeline-nudge-back-5s"
            onClick={() => handleNudge(-5)}
            title="Lùi 5 giây"
            className="flex h-7 w-7 items-center justify-center rounded-md text-slate-400 transition-colors hover:bg-bg-tertiary hover:text-slate-200"
          >
            <Rewind className="h-3.5 w-3.5" />
          </button>

          {/* Nudge -0.1s */}
          <button
            type="button"
            data-testid="timeline-nudge-back-frame"
            onClick={() => handleNudge(-0.1)}
            title="Lùi 1 khung hình (100ms)"
            className="flex h-7 w-7 items-center justify-center rounded-md text-slate-400 transition-colors hover:bg-bg-tertiary hover:text-slate-200"
          >
            <RotateCcw className="h-3.5 w-3.5" />
          </button>

          {/* Nudge +0.1s */}
          <button
            type="button"
            data-testid="timeline-nudge-forward-frame"
            onClick={() => handleNudge(0.1)}
            title="Tiến 1 khung hình (100ms)"
            className="flex h-7 w-7 items-center justify-center rounded-md text-slate-400 transition-colors hover:bg-bg-tertiary hover:text-slate-200"
          >
            <RotateCw className="h-3.5 w-3.5" />
          </button>

          {/* Jump +5s */}
          <button
            type="button"
            data-testid="timeline-nudge-forward-5s"
            onClick={() => handleNudge(5)}
            title="Tiến 5 giây"
            className="flex h-7 w-7 items-center justify-center rounded-md text-slate-400 transition-colors hover:bg-bg-tertiary hover:text-slate-200"
          >
            <FastForward className="h-3.5 w-3.5" />
          </button>

          {/* Volume & Mute */}
          <div className="flex items-center space-x-1.5 pl-2">
            <button
              type="button"
              data-testid="timeline-mute-btn"
              onClick={handleToggleMute}
              aria-label={effectiveIsMuted ? 'Bật âm' : 'Tắt âm'}
              className="flex h-7 w-7 items-center justify-center rounded-md text-slate-400 transition-colors hover:bg-bg-tertiary hover:text-slate-200"
            >
              {effectiveIsMuted || effectiveVolume === 0 ? (
                <VolumeX className="h-4 w-4 text-accent-evil" />
              ) : (
                <Volume2 className="h-4 w-4" />
              )}
            </button>
            <input
              type="range"
              data-testid="timeline-volume-slider"
              min={0}
              max={1}
              step={0.05}
              value={effectiveIsMuted ? 0 : effectiveVolume}
              onChange={handleVolumeChange}
              aria-label="Âm lượng"
              className="h-1.5 w-16 cursor-pointer appearance-none rounded-lg bg-bg-tertiary accent-accent-neuro"
            />
          </div>

          {/* Playback speed selector */}
          <div className="pl-1">
            <select
              data-testid="timeline-speed-select"
              value={effectivePlaybackRate}
              onChange={handleSpeedSelect}
              aria-label="Tốc độ phát video"
              className="focus:outline-hidden h-6 rounded border border-border-dark bg-bg-tertiary px-1 font-mono text-[11px] text-slate-300"
            >
              {SPEED_OPTIONS.map((speed) => (
                <option key={speed} value={speed}>
                  {speed}x
                </option>
              ))}
            </select>
          </div>
        </div>

        {/* Right: Presets & Aspect Ratio Switchers */}
        <div className="flex items-center space-x-1.5">
          {/* Preset 1 (Center-Crop 9:16) */}
          <button
            type="button"
            data-testid="btn-preset-center-crop"
            onClick={() => handlePresetSelect('center-crop')}
            className={`flex items-center space-x-1 rounded-md px-2.5 py-1 text-xs font-medium transition-colors ${
              effectivePreset === 'center-crop'
                ? 'border border-accent-neuro/60 bg-accent-neuro/20 font-semibold text-accent-neuro'
                : 'text-slate-400 hover:bg-bg-tertiary hover:text-slate-200'
            }`}
          >
            <Crop className="h-3.5 w-3.5" />
            <span>Center-Crop 9:16</span>
          </button>

          {/* Preset 2 (Two-Stack 50/50) */}
          <button
            type="button"
            data-testid="btn-preset-two-stack"
            onClick={() => handlePresetSelect('two-stack')}
            className={`flex items-center space-x-1 rounded-md px-2.5 py-1 text-xs font-medium transition-colors ${
              effectivePreset === 'two-stack'
                ? 'border border-accent-neuro/60 bg-accent-neuro/20 font-semibold text-accent-neuro'
                : 'text-slate-400 hover:bg-bg-tertiary hover:text-slate-200'
            }`}
          >
            <Layers className="h-3.5 w-3.5" />
            <span>Two-Stack 50/50</span>
          </button>

          {/* Aspect Ratio Toggle (9:16 vs 16:9) */}
          <button
            type="button"
            data-testid="btn-aspect-ratio-toggle"
            onClick={handleAspectRatioToggle}
            title="Đổi tỷ lệ khung hình"
            className="flex items-center space-x-1 rounded-md border border-border-dark bg-bg-tertiary/70 px-2 py-1 text-xs text-slate-300 transition-colors hover:bg-bg-tertiary"
          >
            <span>{effectiveAspectRatio}</span>
          </button>
        </div>
      </div>
    </div>
  );
};
