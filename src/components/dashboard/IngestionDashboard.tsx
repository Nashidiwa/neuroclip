/**
 * NeuroClip Audio-First Ingestion Dashboard Component
 *
 * Provides clipper UI to input Twitch/YouTube VOD URLs, preview metadata,
 * configure anti-bot cookies/User-Agent, monitor 1Hz download progress,
 * and review Schema 5.2 compliant `vod_manifest.json`.
 *
 * Strict adherence to docs/CONVENTIONS.md (zero-any, strict mode).
 */

import React, { useState } from 'react';
import {
  DownloadCloud,
  CheckCircle2,
  StopCircle,
  Search,
  Shield,
  FileAudio,
  Radio,
  Clock,
  User,
  ChevronDown,
  ChevronUp,
} from 'lucide-react';
import { useIngestionStore } from '@/stores/useIngestionStore';
import { ErrorBanner } from '@/components/common/ErrorBanner';
import type { BrowserCookieType, VodManifest } from '@/services/ipc/ingestion';

export interface IngestionDashboardProps {
  onIngestSuccess?: (manifest: VodManifest) => void;
  className?: string;
}

function formatDuration(seconds: number): string {
  if (!seconds || seconds <= 0) return '00:00';
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = Math.floor(seconds % 60);
  if (h > 0) {
    return `${h.toString().padStart(2, '0')}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
  }
  return `${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
}

function formatSpeed(speedKbps: number): string {
  if (speedKbps >= 1024) {
    return `${(speedKbps / 1024).toFixed(2)} MB/s`;
  }
  return `${speedKbps.toFixed(1)} KB/s`;
}

export const IngestionDashboard: React.FC<IngestionDashboardProps> = ({
  onIngestSuccess,
  className = '',
}) => {
  const {
    url,
    audioOnly,
    cookiesPath,
    browserCookies,
    isProbing,
    isIngesting,
    metadata,
    manifest,
    progress,
    error,
    setUrl,
    setAudioOnly,
    setCookiesPath,
    setBrowserCookies,
    probe,
    startIngestion,
    cancelIngestion,
    clearError,
  } = useIngestionStore();

  const [isCookieConfigOpen, setIsCookieConfigOpen] = useState<boolean>(false);

  const handleUrlChange = (e: React.ChangeEvent<HTMLInputElement>): void => {
    setUrl(e.target.value);
  };

  const handleProbe = async (): Promise<void> => {
    if (!url.trim()) return;
    try {
      await probe();
    } catch {
      // Error is tracked in store
    }
  };

  const handleStartIngest = async (): Promise<void> => {
    if (!url.trim() || isIngesting) return;
    try {
      const res = await startIngestion();
      if (onIngestSuccess) {
        onIngestSuccess(res);
      }
    } catch {
      // Error is tracked in store
    }
  };

  const handleCancel = async (): Promise<void> => {
    await cancelIngestion();
  };

  const isLiveStreamBlocked = metadata?.isLive === true;

  return (
    <div
      data-testid="ingestion-dashboard"
      className={`space-y-5 rounded-xl border border-border-dark bg-bg-secondary/80 p-5 text-slate-100 shadow-xl backdrop-blur-md ${className}`}
    >
      {/* Header */}
      <div className="flex items-center justify-between border-b border-border-dark pb-3">
        <div className="flex items-center space-x-2.5">
          <div className="rounded-lg bg-accent-neuro/10 p-2 text-accent-neuro">
            <DownloadCloud className="h-5 w-5" />
          </div>
          <div>
            <h2 className="flex items-center gap-2 text-sm font-semibold tracking-wide text-slate-100">
              Thu Nạp Luồng VOD Trực Tuyến
              <span className="rounded-full border border-accent-vedal/20 bg-accent-vedal/10 px-2 py-0.5 text-[10px] text-accent-vedal">
                Audio-First
              </span>
            </h2>
            <p className="text-xs text-slate-400">
              Trích xuất luồng âm thanh nhẹ từ Twitch / YouTube (tiết kiệm &ge; 98.5% ổ đĩa &amp;
              băng thông)
            </p>
          </div>
        </div>
      </div>

      {/* Error Banner */}
      {error && (
        <ErrorBanner
          error={error}
          onDismiss={clearError}
          onRetry={error.recoverable ? handleStartIngest : undefined}
        />
      )}

      {/* URL Input & Probe Bar */}
      <div className="space-y-1.5">
        <label htmlFor="vod-url-input" className="block text-xs font-medium text-slate-300">
          URL VOD (Twitch / YouTube):
        </label>
        <div className="flex items-center gap-2">
          <input
            id="vod-url-input"
            data-testid="vod-url-input"
            type="url"
            value={url}
            onChange={handleUrlChange}
            placeholder="https://www.twitch.tv/videos/... hoặc https://www.youtube.com/watch?v=..."
            disabled={isIngesting}
            className="flex-1 rounded-lg border border-border-dark bg-bg-tertiary px-3 py-2 text-xs text-slate-100 transition-colors placeholder:text-slate-500 focus:border-accent-neuro/60 focus:outline-none disabled:opacity-50"
          />
          <button
            type="button"
            data-testid="probe-vod-button"
            onClick={() => void handleProbe()}
            disabled={!url.trim() || isProbing || isIngesting}
            className="flex items-center gap-1.5 rounded-lg border border-border-dark bg-bg-tertiary px-3 py-2 text-xs font-medium text-slate-200 transition-colors hover:bg-slate-700/60 hover:text-accent-neuro disabled:cursor-not-allowed disabled:opacity-40"
          >
            <Search className={`h-3.5 w-3.5 ${isProbing ? 'animate-spin' : ''}`} />
            {isProbing ? 'Đang kiểm tra...' : 'Kiểm tra'}
          </button>
        </div>
      </div>

      {/* Metadata Preview Card */}
      {metadata && (
        <div
          data-testid="vod-metadata-card"
          className="space-y-2 rounded-lg border border-border-dark bg-bg-tertiary/60 p-3 text-xs"
        >
          <div className="flex items-start justify-between gap-3">
            <div className="flex-1">
              <span className="line-clamp-1 font-semibold text-slate-200">{metadata.title}</span>
              <div className="mt-1 flex items-center gap-4 text-slate-400">
                {metadata.uploader && (
                  <span className="flex items-center gap-1">
                    <User className="h-3 w-3 text-accent-vedal" />
                    {metadata.uploader}
                  </span>
                )}
                {metadata.duration !== null && (
                  <span className="flex items-center gap-1">
                    <Clock className="h-3 w-3 text-accent-chat" />
                    {formatDuration(metadata.duration)}
                  </span>
                )}
              </div>
            </div>

            {metadata.isLive ? (
              <span
                data-testid="live-stream-badge"
                className="flex shrink-0 items-center gap-1 rounded border border-accent-evil/30 bg-accent-evil/15 px-2 py-0.5 text-[10px] font-semibold text-accent-evil"
              >
                <Radio className="h-3 w-3 animate-pulse" />
                ĐANG PHÁT TRỰC TIẾP
              </span>
            ) : (
              <span className="shrink-0 rounded border border-accent-vedal/20 bg-accent-vedal/10 px-2 py-0.5 text-[10px] font-medium text-accent-vedal">
                VOD Sẵn Sàng
              </span>
            )}
          </div>

          {isLiveStreamBlocked && (
            <div className="rounded border border-accent-evil/20 bg-accent-evil/10 p-2 text-[11px] text-accent-evil">
              ⚠️ <strong>Cảnh báo:</strong> Livestream đang phát trực tiếp không được hỗ trợ để
              tránh tràn bộ nhớ ổ đĩa. Vui lòng nạp VOD sau khi buổi phát trực tiếp kết thúc.
            </div>
          )}
        </div>
      )}

      {/* Audio-First Invariant Switch */}
      <div className="flex items-start gap-3 rounded-lg border border-border-dark bg-bg-tertiary/40 p-3">
        <input
          id="audio-first-checkbox"
          data-testid="audio-first-checkbox"
          type="checkbox"
          checked={audioOnly}
          onChange={(e) => setAudioOnly(e.target.checked)}
          disabled={isIngesting}
          className="mt-0.5 cursor-pointer rounded border-border-dark bg-bg-secondary text-accent-neuro focus:ring-accent-neuro disabled:opacity-50"
        />
        <label htmlFor="audio-first-checkbox" className="cursor-pointer select-none text-xs">
          <span className="flex items-center gap-1.5 font-medium text-slate-200">
            <FileAudio className="h-3.5 w-3.5 text-accent-vedal" />
            Kích hoạt Audio-First Invariant (Khuyên dùng)
          </span>
          <span className="mt-0.5 block text-[11px] text-slate-400">
            Chỉ tải luồng âm thanh nhẹ (&le; 450MB cho VOD 8 tiếng). Tiết kiệm &ge; 98.5% dung lượng
            mạng và ổ nhớ, hoàn tất trong &le; 90s.
          </span>
        </label>
      </div>

      {/* Anti-Bot & Cookie Settings Accordion */}
      <div className="overflow-hidden rounded-lg border border-border-dark bg-bg-tertiary/20">
        <button
          type="button"
          data-testid="cookie-accordion-toggle"
          onClick={() => setIsCookieConfigOpen((prev) => !prev)}
          className="flex w-full items-center justify-between px-3 py-2 text-xs font-medium text-slate-300 transition-colors hover:bg-bg-tertiary/40"
        >
          <span className="flex items-center gap-1.5">
            <Shield className="h-3.5 w-3.5 text-accent-chat" />
            Cấu hình Cookie & Vượt Rào Cản Anti-Bot (HTTP 403 / 429)
          </span>
          {isCookieConfigOpen ? (
            <ChevronUp className="h-3.5 w-3.5" />
          ) : (
            <ChevronDown className="h-3.5 w-3.5" />
          )}
        </button>

        {isCookieConfigOpen && (
          <div className="space-y-3 border-t border-border-dark bg-bg-tertiary/40 p-3 text-xs">
            <div>
              <label
                htmlFor="browser-cookies-select"
                className="mb-1 block text-[11px] text-slate-400"
              >
                Trích xuất Cookie từ Trình duyệt Desktop:
              </label>
              <select
                id="browser-cookies-select"
                data-testid="browser-cookies-select"
                value={browserCookies ?? ''}
                onChange={(e) => {
                  const val = e.target.value;
                  setBrowserCookies(val === '' ? null : (val as BrowserCookieType));
                }}
                disabled={isIngesting}
                className="w-full rounded border border-border-dark bg-bg-secondary px-2.5 py-1.5 text-xs text-slate-200 focus:border-accent-neuro/60 focus:outline-none"
              >
                <option value="">Không sử dụng (Mặc định)</option>
                <option value="chrome">Google Chrome</option>
                <option value="firefox">Mozilla Firefox</option>
                <option value="edge">Microsoft Edge</option>
              </select>
            </div>

            <div>
              <label htmlFor="cookies-txt-input" className="mb-1 block text-[11px] text-slate-400">
                Hoặc đường dẫn tệp cookies.txt (Netscape format):
              </label>
              <input
                id="cookies-txt-input"
                data-testid="cookies-txt-input"
                type="text"
                value={cookiesPath ?? ''}
                onChange={(e) => setCookiesPath(e.target.value)}
                placeholder="C:\Users\...\cookies.txt"
                disabled={isIngesting}
                className="w-full rounded border border-border-dark bg-bg-secondary px-2.5 py-1.5 text-xs text-slate-200 placeholder:text-slate-500 focus:border-accent-neuro/60 focus:outline-none"
              />
            </div>
          </div>
        )}
      </div>

      {/* Download Progress Monitor */}
      {isIngesting && progress && (
        <div
          data-testid="ingestion-progress-box"
          className="space-y-2 rounded-lg border border-border-dark bg-bg-tertiary/50 p-3"
        >
          <div className="flex items-center justify-between text-xs">
            <span className="flex items-center gap-1.5 font-medium text-slate-300">
              <DownloadCloud className="h-3.5 w-3.5 animate-pulse text-accent-neuro" />
              Đang tải âm thanh: {progress.percent.toFixed(1)}%
            </span>
            <span className="text-[11px] text-slate-400">
              {formatSpeed(progress.speedKbps)} | ETA: {formatDuration(progress.etaSec)}
            </span>
          </div>

          <div className="h-2 w-full overflow-hidden rounded-full border border-border-dark bg-bg-secondary">
            <div
              data-testid="progress-bar-fill"
              className="h-full bg-accent-neuro transition-all duration-300 ease-out"
              style={{ width: `${Math.min(100, Math.max(0, progress.percent))}%` }}
            />
          </div>
        </div>
      )}

      {/* Manifest Success Card */}
      {manifest && (
        <div
          data-testid="vod-manifest-success-card"
          className="space-y-1.5 rounded-lg border border-accent-vedal/30 bg-accent-vedal/10 p-3 text-xs text-slate-200"
        >
          <div className="flex items-center gap-1.5 font-semibold text-accent-vedal">
            <CheckCircle2 className="h-4 w-4" />
            Thu nạp thành công! Manifest Schema 5.2 đã được sinh.
          </div>
          <div className="grid grid-cols-2 gap-2 pt-1 text-[11px] text-slate-300">
            <div>
              <span className="text-slate-400">Mã VOD:</span> {manifest.vodId}
            </div>
            <div>
              <span className="text-slate-400">Thời lượng:</span>{' '}
              {formatDuration(manifest.durationSeconds)}
            </div>
            <div className="col-span-2 truncate">
              <span className="text-slate-400">File âm thanh:</span> {manifest.audioPath}
            </div>
          </div>
        </div>
      )}

      {/* Control Buttons */}
      <div className="flex items-center justify-end gap-2 border-t border-border-dark pt-2">
        {isIngesting ? (
          <button
            type="button"
            data-testid="cancel-ingest-button"
            onClick={() => void handleCancel()}
            className="flex items-center gap-1.5 rounded-lg border border-accent-evil/30 bg-accent-evil/15 px-4 py-2 text-xs font-semibold text-accent-evil transition-colors hover:bg-accent-evil/25"
          >
            <StopCircle className="h-4 w-4" />
            Hủy tác vụ
          </button>
        ) : (
          <button
            type="button"
            data-testid="start-ingest-button"
            onClick={() => void handleStartIngest()}
            disabled={!url.trim() || isLiveStreamBlocked || isProbing}
            className="flex items-center gap-1.5 rounded-lg bg-accent-neuro px-5 py-2 text-xs font-semibold text-slate-900 shadow-md shadow-accent-neuro/20 transition-colors hover:bg-accent-neuro/90 disabled:cursor-not-allowed disabled:opacity-40"
          >
            <DownloadCloud className="h-4 w-4" />
            Bắt đầu thu nạp
          </button>
        )}
      </div>
    </div>
  );
};
