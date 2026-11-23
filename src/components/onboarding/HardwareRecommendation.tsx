/**
 * HardwareRecommendation Component
 * Displays system hardware specifications (CPU, RAM, GPU/VRAM) and provides
 * intelligent workflow recommendations (offline_local, hybrid, cloud_byok).
 *
 * Conforms to docs/CONVENTIONS.md (Neuro VTuber Palette, zero-any, responsive design).
 */

import React, { useEffect, useRef } from 'react';
import {
  Cpu,
  HardDrive,
  Monitor,
  RefreshCw,
  Sparkles,
  ShieldCheck,
  AlertTriangle,
  Cloud,
  Zap,
  Server,
  CheckCircle2,
} from 'lucide-react';
import { useHardwareStore } from '../../stores/useHardwareStore.ts';
import type { RecommendedMode } from '../../services/config/hardware.ts';

export interface HardwareRecommendationProps {
  readonly onProceed?: ((selectedMode: RecommendedMode) => void) | undefined;
  readonly compact?: boolean | undefined;
  readonly className?: string | undefined;
}

interface ModeCardOption {
  readonly mode: RecommendedMode;
  readonly title: string;
  readonly subtitle: string;
  readonly description: string;
  readonly icon: React.FC<{ readonly className?: string | undefined }>;
  readonly accentClass: string;
  readonly borderClass: string;
  readonly bgClass: string;
}

const MODE_OPTIONS: readonly ModeCardOption[] = [
  {
    mode: 'offline_local',
    title: 'Offline Local',
    subtitle: 'Faster-Whisper Large Cục Bộ',
    description:
      'Chạy hoàn toàn trên máy tính cá nhân. Bảo mật tối đa, 0 chi phí API, yêu cầu GPU NVIDIA ≥ 8GB VRAM.',
    icon: Server,
    accentClass: 'text-accent-vedal',
    borderClass: 'border-accent-vedal/40',
    bgClass: 'bg-accent-vedal/10',
  },
  {
    mode: 'hybrid',
    title: 'Hybrid Khuyên Dùng',
    subtitle: 'Groq Cloud STT + Xử Lý Cục Bộ',
    description:
      'Bóc băng tốc độ cực cao với Groq Whisper Cloud (miễn phí), trích xuất và cắt ghép video mượt mà trên máy.',
    icon: Zap,
    accentClass: 'text-accent-neuro',
    borderClass: 'border-accent-neuro/40',
    bgClass: 'bg-accent-neuro/10',
  },
  {
    mode: 'cloud_byok',
    title: 'Cloud BYOK',
    subtitle: 'Groq API + Google Gemini Flash',
    description:
      'Phù hợp cho laptop văn phòng, máy không có card đồ họa rời (iGPU) hoặc RAM ≤ 8GB, giữ máy luôn mát mẻ.',
    icon: Cloud,
    accentClass: 'text-accent-chat',
    borderClass: 'border-accent-chat/40',
    bgClass: 'bg-accent-chat/10',
  },
];

export const HardwareRecommendation: React.FC<HardwareRecommendationProps> = ({
  onProceed,
  compact = false,
  className = '',
}) => {
  const profile = useHardwareStore((s) => s.profile);
  const selectedMode = useHardwareStore((s) => s.selectedMode);
  const isOverridden = useHardwareStore((s) => s.isOverridden);
  const isLoading = useHardwareStore((s) => s.isLoading);
  const error = useHardwareStore((s) => s.error);

  const fetchProfile = useHardwareStore((s) => s.fetchProfile);
  const detectHardware = useHardwareStore((s) => s.detectHardware);
  const selectModeOverride = useHardwareStore((s) => s.selectModeOverride);
  const applyToSettings = useHardwareStore((s) => s.applyToSettings);

  const hasInitiatedProbeRef = useRef(false);

  useEffect(() => {
    if (hasInitiatedProbeRef.current) {
      return;
    }
    if (profile === null && !isLoading && error === null) {
      hasInitiatedProbeRef.current = true;
      void fetchProfile().then((res) => {
        if (res === null) {
          void detectHardware();
        }
      });
    }
  }, [profile, isLoading, error, fetchProfile, detectHardware]);

  const handleSelectMode = (mode: RecommendedMode): void => {
    selectModeOverride(mode);
  };

  const handleProceed = (): void => {
    if (selectedMode !== null) {
      applyToSettings(selectedMode);
      if (onProceed) {
        onProceed(selectedMode);
      }
    }
  };

  const isLowEndForLocal =
    profile !== null &&
    (profile.gpuInfo.vramMb < 6000 ||
      !profile.gpuInfo.cudaSupported ||
      profile.ramInfo.totalMb < 12000);

  const showLocalWarning = selectedMode === 'offline_local' && isLowEndForLocal;

  return (
    <div
      data-testid="hardware-recommendation-card"
      className={`rounded-xl border border-border-dark bg-bg-secondary/80 p-5 shadow-acrylic backdrop-blur-md ${className}`}
    >
      {/* Header */}
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-border-dark/60 pb-4">
        <div className="space-y-1">
          <div className="flex items-center space-x-2">
            <Sparkles className="h-5 w-5 text-accent-neuro" />
            <h2 className="text-base font-bold text-white">
              Khảo Sát Năng Lực Phần Cứng &amp; Đề Xuất Cấu Hình
            </h2>
          </div>
          <p className="text-xs text-slate-300">
            Hệ thống tự động thăm dò thông số CPU, RAM, VRAM để lựa chọn chế độ bóc băng và dựng
            clip tối ưu.
          </p>
        </div>

        <button
          type="button"
          data-testid="btn-reprobe-hardware"
          disabled={isLoading}
          onClick={() => void detectHardware()}
          className="inline-flex items-center space-x-1.5 rounded-lg border border-border-dark bg-bg-tertiary px-3 py-1.5 text-xs font-medium text-slate-200 transition-colors hover:border-accent-neuro/60 hover:text-white disabled:opacity-50"
        >
          <RefreshCw
            className={`h-3.5 w-3.5 ${isLoading ? 'animate-spin text-accent-neuro' : ''}`}
          />
          <span>{isLoading ? 'Đang quét...' : 'Quét lại cấu hình'}</span>
        </button>
      </div>

      {/* Error Callout */}
      {error && (
        <div
          data-testid="hardware-probe-error"
          className="mt-4 flex items-center space-x-2 rounded-lg border border-accent-evil/40 bg-accent-evil/10 p-3 text-xs text-accent-evil"
        >
          <AlertTriangle className="h-4 w-4 shrink-0" />
          <span>{error}</span>
        </div>
      )}

      {/* Hardware Specs Grid */}
      {profile ? (
        <div
          data-testid="hardware-specs-grid"
          className="mt-4 grid grid-cols-1 gap-3 sm:grid-cols-3"
        >
          {/* CPU Card */}
          <div className="rounded-lg border border-border-dark bg-bg-primary/50 p-3">
            <div className="flex items-center justify-between text-slate-400">
              <span className="text-[11px] font-medium uppercase">Bộ Xử Lý (CPU)</span>
              <Cpu className="h-4 w-4 text-accent-vedal" />
            </div>
            <div
              className="mt-1 truncate text-xs font-semibold text-white"
              title={profile.cpuInfo.model}
            >
              {profile.cpuInfo.model || 'Unknown CPU'}
            </div>
            <div className="mt-1 font-mono text-[11px] text-slate-400">
              {profile.cpuInfo.cores} Nhân / {profile.cpuInfo.threads} Luồng
            </div>
          </div>

          {/* RAM Card */}
          <div className="rounded-lg border border-border-dark bg-bg-primary/50 p-3">
            <div className="flex items-center justify-between text-slate-400">
              <span className="text-[11px] font-medium uppercase">Bộ Nhớ RAM</span>
              <HardDrive className="h-4 w-4 text-accent-chat" />
            </div>
            <div className="mt-1 text-xs font-semibold text-white">
              {(profile.ramInfo.totalMb / 1024).toFixed(1)} GB Tổng
            </div>
            <div className="mt-1 font-mono text-[11px] text-slate-400">
              Khả dụng: {(profile.ramInfo.availableMb / 1024).toFixed(1)} GB
            </div>
          </div>

          {/* GPU Card */}
          <div className="rounded-lg border border-border-dark bg-bg-primary/50 p-3">
            <div className="flex items-center justify-between text-slate-400">
              <span className="text-[11px] font-medium uppercase">Đồ Họa &amp; VRAM</span>
              <Monitor className="h-4 w-4 text-accent-neuro" />
            </div>
            <div
              className="mt-1 truncate text-xs font-semibold text-white"
              title={profile.gpuInfo.model}
            >
              {profile.gpuInfo.model || 'Unknown GPU'}
            </div>
            <div className="mt-1 flex items-center justify-between text-[11px]">
              <span className="font-mono text-slate-400">
                {(profile.gpuInfo.vramMb / 1024).toFixed(1)} GB VRAM
              </span>
              <span
                data-testid="cuda-support-badge"
                className={`rounded px-1.5 py-0.5 text-[10px] font-semibold ${
                  profile.gpuInfo.cudaSupported
                    ? 'border border-accent-vedal/40 bg-accent-vedal/20 text-accent-vedal'
                    : 'border border-slate-600 bg-slate-800 text-slate-400'
                }`}
              >
                {profile.gpuInfo.cudaSupported ? 'CUDA Sẵn sàng' : 'Không có CUDA'}
              </span>
            </div>
          </div>
        </div>
      ) : (
        <div
          data-testid="hardware-loading-skeleton"
          className="mt-4 flex items-center justify-center p-6 text-xs text-slate-400"
        >
          <RefreshCw className="mr-2 h-4 w-4 animate-spin text-accent-neuro" />
          <span>Đang truy vấn thông số phần cứng hệ thống...</span>
        </div>
      )}

      {/* Heuristic Recommendation Callout */}
      {profile && (
        <div
          data-testid="recommendation-callout"
          className="mt-4 rounded-lg border border-accent-neuro/30 bg-accent-neuro/10 p-3.5"
        >
          <div className="flex items-start space-x-2.5">
            <ShieldCheck className="mt-0.5 h-4 w-4 shrink-0 text-accent-neuro" />
            <div className="space-y-1">
              <div className="flex items-center space-x-2">
                <span className="text-xs font-semibold text-white">Khuyến nghị từ NeuroClip:</span>
                <span
                  data-testid="recommended-mode-badge"
                  className="rounded-full border border-accent-neuro/50 bg-accent-neuro/20 px-2 py-0.5 text-[11px] font-bold uppercase text-accent-neuro"
                >
                  {profile.recommendedMode}
                </span>
                {isOverridden && (
                  <span
                    data-testid="mode-overridden-badge"
                    className="rounded border border-accent-guest/40 bg-accent-guest/20 px-1.5 py-0.5 text-[10px] text-accent-guest"
                  >
                    Đã ghi đè thủ công
                  </span>
                )}
              </div>
              <p
                data-testid="recommendation-reason-text"
                className="text-xs leading-relaxed text-slate-200"
              >
                {profile.recommendationReason ||
                  'Cấu hình tối ưu được xác định dựa trên thông số phần cứng.'}
              </p>
            </div>
          </div>
        </div>
      )}

      {/* Warning for Weak Hardware Selecting Offline Local */}
      {showLocalWarning && (
        <div
          data-testid="low-end-local-warning"
          className="mt-3 flex items-start space-x-2 rounded-lg border border-accent-evil/40 bg-accent-evil/10 p-3 text-xs text-slate-200"
        >
          <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-accent-evil" />
          <p>
            <strong className="text-accent-evil">Lưu ý hiệu năng:</strong> Máy tính của bạn không có
            đủ VRAM hoặc CUDA để chạy Faster-Whisper Large mượt mà. Lựa chọn Offline Local có thể
            gây chậm hoặc lỗi quá tải bộ nhớ. Bạn nên cân nhắc chế độ <strong>Hybrid</strong> hoặc{' '}
            <strong>Cloud BYOK</strong>.
          </p>
        </div>
      )}

      {/* Interactive Mode Selection Cards */}
      <div className="mt-5 space-y-2">
        <label className="text-xs font-semibold text-slate-300">
          Chọn Chế Độ Vận Hành {compact ? '' : '(Nhấp để thay đổi):'}
        </label>

        <div className="grid grid-cols-1 gap-2.5 md:grid-cols-3">
          {MODE_OPTIONS.map((opt) => {
            const isSelected = selectedMode === opt.mode;
            const isSystemRecommended = profile?.recommendedMode === opt.mode;
            const IconComponent = opt.icon;

            return (
              <button
                key={opt.mode}
                type="button"
                data-testid={`mode-option-${opt.mode}`}
                onClick={() => handleSelectMode(opt.mode)}
                className={`relative flex flex-col justify-between rounded-lg border p-3.5 text-left transition-all ${
                  isSelected
                    ? `${opt.borderClass} ${opt.bgClass} shadow-md`
                    : 'border-border-dark bg-bg-primary/40 hover:border-slate-600 hover:bg-bg-tertiary/40'
                }`}
              >
                <div>
                  <div className="flex items-center justify-between">
                    <div className="flex items-center space-x-1.5">
                      <IconComponent className={`h-4 w-4 ${opt.accentClass}`} />
                      <span className="text-xs font-bold text-white">{opt.title}</span>
                    </div>
                    {isSelected && <CheckCircle2 className={`h-4 w-4 ${opt.accentClass}`} />}
                  </div>

                  <p className="mt-1 text-[11px] font-medium text-slate-300">{opt.subtitle}</p>
                  <p className="mt-1.5 text-[11px] leading-relaxed text-slate-400">
                    {opt.description}
                  </p>
                </div>

                {isSystemRecommended && (
                  <div className="mt-3">
                    <span className="inline-block rounded bg-accent-neuro/20 px-1.5 py-0.5 text-[10px] font-semibold text-accent-neuro">
                      ★ Tối ưu cho máy bạn
                    </span>
                  </div>
                )}
              </button>
            );
          })}
        </div>
      </div>

      {/* Action Footer */}
      {onProceed && (
        <div className="mt-6 flex justify-end border-t border-border-dark/60 pt-4">
          <button
            type="button"
            data-testid="btn-confirm-hardware-mode"
            disabled={selectedMode === null || isLoading}
            onClick={handleProceed}
            className="rounded-lg bg-accent-neuro px-4 py-2 text-xs font-semibold text-bg-primary transition-opacity hover:opacity-90 disabled:opacity-50"
          >
            Xác Nhận &amp; Tiếp Tục
          </button>
        </div>
      )}
    </div>
  );
};
