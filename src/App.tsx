import React, { useState, useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  Sparkles,
  Monitor,
  Activity,
  Layers,
  ShieldCheck,
  Cpu,
  RefreshCw,
  Sliders,
  HardDrive,
  MessageSquare,
} from 'lucide-react';
import {
  loadTheme,
  resolveBackdropConfig,
  type WindowBackdropConfig,
  WINDOW_GEOMETRY,
} from './styles/theme.ts';
import { ErrorBoundary } from './components/common/ErrorBoundary.tsx';
import { ErrorBanner } from './components/common/ErrorBanner.tsx';
import { IngestionDashboard } from './components/dashboard/IngestionDashboard.tsx';
import { DropZone } from './components/dashboard/DropZone.tsx';
import { HighlightViewer } from './components/highlight/HighlightViewer.tsx';
import { SubtitleStudio, deriveStudioClipId } from './components/studio/SubtitleStudio.tsx';
import { OnboardingWizardModal } from './components/onboarding/OnboardingWizardModal.tsx';
import { useSettingsStore } from './stores/settingsStore.ts';
import { useProjectStore } from './stores/useProjectStore.ts';
import { useIngestionStore } from './stores/useIngestionStore.ts';
import { useChatStore } from './stores/useChatStore.ts';
import { useHighlightStore } from './stores/useHighlightStore.ts';
import { useStudioStore } from './stores/useStudioStore.ts';
import { useTranscriptStore } from './stores/useTranscriptStore.ts';
import { toLocalAssetUrl, type VodManifest } from './services/ipc/ingestion.ts';
import {
  sliceVideoSegment,
  cancelSliceVideoSegment,
  listenSliceProgress,
  type SliceProgressPayload,
} from './services/ipc/slice.ts';
import { parseIpcError, type IpcErrorResponse } from './services/ipc/error.ts';
import type { ChatExtractionOptions } from './services/ipc/chat.ts';
import type { HighlightCluster } from './types/highlight.ts';

type AppTab = 'overview' | 'ingestion' | 'highlights' | 'studio';

function formatChatOffset(seconds: number): string {
  const safe = Number.isFinite(seconds) && seconds > 0 ? Math.floor(seconds) : 0;
  const hrs = Math.floor(safe / 3600);
  const mins = Math.floor((safe % 3600) / 60);
  const secs = safe % 60;
  if (hrs > 0) {
    return `${hrs}:${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}`;
  }
  return `${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}`;
}

export const App: React.FC = () => {
  const theme = loadTheme();
  // Assume acrylic is supported if Windows 11/10 21H2+ is present, otherwise fallback
  const [isAcrylicSupported, setIsAcrylicSupported] = useState<boolean>(true);
  const [backdropConfig, setBackdropConfig] = useState<WindowBackdropConfig>(() =>
    resolveBackdropConfig(true),
  );
  const [activeTab, setActiveTab] = useState<AppTab>('overview');

  const isFirstRunConfig = useSettingsStore((s) => s.config.isFirstRun);
  const loadConfig = useSettingsStore((s) => s.loadConfig);
  const [isOnboardingOpen, setIsOnboardingOpen] = useState<boolean>(false);

  useEffect(() => {
    void loadConfig();
  }, [loadConfig]);

  useEffect(() => {
    if (isFirstRunConfig) {
      setIsOnboardingOpen(true);
    }
  }, [isFirstRunConfig]);

  const customCacheDir = useProjectStore((s) => s.customCacheDir);
  const isCheckingStorage = useProjectStore((s) => s.isCheckingStorage);
  const isInsufficientSpace = useProjectStore((s) => s.isInsufficientSpace);
  const storageError = useProjectStore((s) => s.storageError);
  const projectId = useProjectStore((s) => s.projectId);
  const checkPreflight = useProjectStore((s) => s.checkPreflight);
  const initializeProject = useProjectStore((s) => s.initializeProject);
  const clearStorageError = useProjectStore((s) => s.clearStorageError);

  const manifest = useIngestionStore((s) => s.manifest);
  const ingestionUrl = useIngestionStore((s) => s.url);
  const ingestionCookiesPath = useIngestionStore((s) => s.cookiesPath);
  const ingestionBrowserCookies = useIngestionStore((s) => s.browserCookies);

  const chatVodId = useChatStore((s) => s.vodId);
  const isExtractingChat = useChatStore((s) => s.isExtracting);
  const hasChatReplay = useChatStore((s) => s.hasChatReplay);
  const chatFilePath = useChatStore((s) => s.chatFilePath);
  const chatProgress = useChatStore((s) => s.progress);
  const chatHeatmap = useChatStore((s) => s.chatHeatmap);
  const chatFallbackNotice = useChatStore((s) => s.fallbackNotice);
  const chatError = useChatStore((s) => s.error);
  const extractChat = useChatStore((s) => s.extractChat);
  const cancelChat = useChatStore((s) => s.cancelChat);
  const clearFallbackNotice = useChatStore((s) => s.clearFallbackNotice);
  const clearChatError = useChatStore((s) => s.clearError);

  const highlightVodId = useHighlightStore((s) => s.vodId);
  const clusters = useHighlightStore((s) => s.clusters);
  const selectedClusterId = useHighlightStore((s) => s.selectedClusterId);
  const isLoadingHighlights = useHighlightStore((s) => s.isLoading);
  const highlightError = useHighlightStore((s) => s.error);
  const selectCluster = useHighlightStore((s) => s.selectCluster);
  const detectHighlightsForVod = useHighlightStore((s) => s.detectHighlightsForVod);
  const fetchClustersForVod = useHighlightStore((s) => s.fetchClustersForVod);
  const toggleBookmark = useHighlightStore((s) => s.toggleBookmark);
  const clearHighlightError = useHighlightStore((s) => s.clearError);

  const activeVodId = highlightVodId ?? chatVodId ?? manifest?.vodId ?? null;
  const isLocalFileManifest = manifest?.sourceType === 'local_file';
  const canDetectHighlights =
    Boolean(activeVodId) &&
    manifest !== null &&
    manifest.vodId === activeVodId &&
    Number.isFinite(manifest.durationSeconds) &&
    manifest.durationSeconds > 0;

  useEffect(() => {
    setBackdropConfig(resolveBackdropConfig(isAcrylicSupported));
  }, [isAcrylicSupported]);

  const handleManifestReady = (nextManifest: VodManifest): void => {
    useIngestionStore.setState({
      manifest: nextManifest,
      error: null,
      ...(nextManifest.sourceType === 'local_file' ? { url: '' } : {}),
    });

    if (useChatStore.getState().vodId !== nextManifest.vodId) {
      useChatStore.getState().reset();
    }
    useChatStore.getState().setVodId(nextManifest.vodId);

    if (useHighlightStore.getState().vodId !== nextManifest.vodId) {
      useHighlightStore.getState().reset();
    }
    useHighlightStore.getState().setVodId(nextManifest.vodId);

    void initializeProject(
      nextManifest.vodId,
      nextManifest.title,
      customCacheDir ?? undefined,
    ).catch(() => {});
  };

  const handleRunPreflightCheck = (): void => {
    void checkPreflight(customCacheDir ?? undefined).catch(() => {});
  };

  const handleExtractChat = (): void => {
    if (!activeVodId || isLocalFileManifest || !ingestionUrl.trim()) return;
    const chatOptions: { cookiesPath?: string; browserCookies?: string } = {};
    if (ingestionCookiesPath && ingestionCookiesPath.trim().length > 0) {
      chatOptions.cookiesPath = ingestionCookiesPath.trim();
    }
    if (ingestionBrowserCookies && ingestionBrowserCookies.trim().length > 0) {
      chatOptions.browserCookies = ingestionBrowserCookies;
    }
    const resolvedOptions: ChatExtractionOptions | undefined =
      Object.keys(chatOptions).length > 0 ? chatOptions : undefined;
    void extractChat(activeVodId, ingestionUrl, resolvedOptions).catch(() => {});
  };

  const handleDetectHighlights = (): void => {
    if (!activeVodId || !canDetectHighlights || !manifest) return;
    const duration = manifest.durationSeconds;
    const detectOptions: { chatEventsPath?: string; audioPath?: string } = {};
    if (manifest.audioPath) {
      detectOptions.audioPath = manifest.audioPath;
    }
    if (chatFilePath) {
      detectOptions.chatEventsPath = chatFilePath;
    }
    void detectHighlightsForVod(
      activeVodId,
      duration,
      Object.keys(detectOptions).length > 0 ? detectOptions : undefined,
    ).catch(() => {});
  };

  const [isSlicing, setIsSlicing] = useState<boolean>(false);
  const [slicingClusterId, setSlicingClusterId] = useState<string | null>(null);
  const [slicingTaskId, setSlicingTaskId] = useState<string | null>(null);
  const [sliceProgress, setSliceProgress] = useState<SliceProgressPayload | null>(null);
  const [sliceError, setSliceError] = useState<IpcErrorResponse | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listenSliceProgress((payload) => {
      setSliceProgress(payload);
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});
    return () => {
      unlisten?.();
    };
  }, []);

  const handleSliceCluster = async (cluster: HighlightCluster): Promise<void> => {
    const vodId = activeVodId ?? manifest?.vodId ?? 'vod_current';
    const customInputSource = manifest?.localVideoPath ?? manifest?.videoStreamUrl ?? undefined;
    const taskId =
      typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function'
        ? crypto.randomUUID()
        : `slice_${Date.now()}`;

    setIsSlicing(true);
    setSlicingClusterId(cluster.clusterId);
    setSlicingTaskId(taskId);
    setSliceProgress({ taskId, stage: 'starting', percent: 0 });
    setSliceError(null);

    try {
      const res = await sliceVideoSegment({
        vodId,
        startSec: cluster.startSec,
        endSec: cluster.endSec,
        clusterId: cluster.clusterId,
        customInputSource,
      });

      if (res.clipPath) {
        useStudioStore.getState().setVideoSrc(toLocalAssetUrl(res.clipPath), res.clipPath);
        const transcriptState = useTranscriptStore.getState();
        const segments =
          transcriptState.container?.segments ?? transcriptState.streamedChunks ?? [];
        if (segments.length > 0) {
          useStudioStore.getState().setSegments(segments);
        }
        if (transcriptState.clipId === null) {
          useTranscriptStore
            .getState()
            .setClipId(deriveStudioClipId(`${vodId}_${cluster.clusterId}`, res.clipPath));
        }
        setActiveTab('studio');
      }
    } catch (err: unknown) {
      const ipcErr = parseIpcError(err, 'Lỗi cắt phân đoạn Highlight');
      setSliceError(ipcErr);
    } finally {
      setIsSlicing(false);
      setSlicingClusterId(null);
      setSlicingTaskId(null);
      setSliceProgress(null);
    }
  };

  const handleCancelSlice = async (): Promise<void> => {
    if (!slicingTaskId) return;
    try {
      await cancelSliceVideoSegment(slicingTaskId);
    } catch {
      // Ignored
    } finally {
      setIsSlicing(false);
      setSlicingClusterId(null);
      setSlicingTaskId(null);
      setSliceProgress(null);
    }
  };

  const handleWindowMinimize = (): void => {
    try {
      void getCurrentWindow()
        .minimize()
        .catch(() => {});
    } catch {
      // Non-tauri browser environment fallback
    }
  };

  const handleWindowMaximize = (): void => {
    try {
      void getCurrentWindow()
        .toggleMaximize()
        .catch(() => {});
    } catch {
      // Non-tauri browser environment fallback
    }
  };

  const handleWindowClose = (): void => {
    try {
      void getCurrentWindow()
        .close()
        .catch(() => {});
    } catch {
      // Non-tauri browser environment fallback
    }
  };

  return (
    <div
      className={`flex h-screen w-screen flex-col overflow-hidden text-slate-100 ${backdropConfig.activeClassName}`}
      data-testid="app-container"
      style={{
        backgroundColor: isAcrylicSupported ? undefined : backdropConfig.solidFallbackHex,
      }}
    >
      {/* Window TitleBar (Custom Tauri Drag Region) */}
      <header
        data-tauri-drag-region
        className="flex h-10 select-none items-center justify-between border-b border-border-dark bg-bg-secondary/60 px-3 backdrop-blur-md"
      >
        <div className="pointer-events-none flex items-center space-x-2">
          <div className="h-3 w-3 animate-pulse rounded-full bg-accent-neuro" />
          <span className="text-xs font-semibold tracking-wider text-accent-neuro">NEUROCLIP</span>
          <span className="text-xs text-slate-400">| AI Subtitle Studio</span>
        </div>

        <div className="pointer-events-none font-mono text-[11px] text-slate-400">
          Window: {WINDOW_GEOMETRY.defaultWidth}x{WINDOW_GEOMETRY.defaultHeight} (Min:{' '}
          {WINDOW_GEOMETRY.minWidth}x{WINDOW_GEOMETRY.minHeight})
        </div>

        <div className="flex items-center space-x-2">
          {/* Setup Wizard Button (AC-28.3) */}
          <button
            type="button"
            data-tauri-no-drag
            onClick={() => setIsOnboardingOpen(true)}
            title="Mở trình hướng dẫn khởi động Onboarding"
            className="flex items-center space-x-1 rounded bg-bg-tertiary px-2 py-0.5 text-[11px] text-accent-neuro transition-colors hover:bg-slate-700"
          >
            <Sparkles className="h-3 w-3 text-accent-neuro" />
            <span>Setup Wizard</span>
          </button>

          {/* Acrylic / Solid Toggle Button */}
          <button
            type="button"
            data-tauri-no-drag
            onClick={() => setIsAcrylicSupported((prev) => !prev)}
            title="Chuyển đổi hiệu ứng Acrylic / Solid Fallback"
            className="flex items-center space-x-1 rounded bg-bg-tertiary px-2 py-0.5 text-[11px] text-slate-300 transition-colors hover:bg-slate-700"
          >
            <Sliders className="h-3 w-3 text-accent-vedal" />
            <span>{isAcrylicSupported ? 'Acrylic: ON' : 'Solid Fallback: #12131C'}</span>
          </button>

          {/* Window Control Buttons */}
          <div className="ml-2 flex items-center space-x-1" data-tauri-no-drag>
            <button
              type="button"
              data-tauri-no-drag
              onClick={handleWindowMinimize}
              title="Thu nhỏ cửa sổ"
              className="flex h-6 w-7 items-center justify-center rounded font-mono text-xs text-slate-400 transition-colors hover:bg-bg-tertiary hover:text-white"
            >
              &#x2014;
            </button>
            <button
              type="button"
              data-tauri-no-drag
              onClick={handleWindowMaximize}
              title="Phóng to / Khôi phục cửa sổ"
              className="flex h-6 w-7 items-center justify-center rounded font-mono text-xs text-slate-400 transition-colors hover:bg-bg-tertiary hover:text-white"
            >
              &#x25A2;
            </button>
            <button
              type="button"
              data-tauri-no-drag
              onClick={handleWindowClose}
              title="Đóng ứng dụng"
              className="flex h-6 w-7 items-center justify-center rounded font-mono text-xs text-slate-400 transition-colors hover:bg-accent-evil hover:text-white"
            >
              &#x2715;
            </button>
          </div>
        </div>
      </header>

      {/* Workspace Tab Navigation */}
      <nav
        role="tablist"
        aria-label="Điều hướng không gian làm việc"
        className="flex items-center justify-between border-b border-border-dark bg-bg-secondary/40 px-4 py-1.5"
      >
        <div className="flex items-center space-x-1.5">
          <button
            type="button"
            role="tab"
            data-testid="tab-overview"
            aria-selected={activeTab === 'overview'}
            onClick={() => setActiveTab('overview')}
            className={`rounded-md px-3 py-1 text-xs font-medium transition-colors ${
              activeTab === 'overview'
                ? 'border border-accent-neuro/40 bg-accent-neuro/20 text-accent-neuro'
                : 'text-slate-400 hover:bg-bg-tertiary/60 hover:text-slate-200'
            }`}
          >
            Tổng quan Shell
          </button>
          <button
            type="button"
            role="tab"
            data-testid="tab-ingestion"
            aria-selected={activeTab === 'ingestion'}
            onClick={() => setActiveTab('ingestion')}
            className={`rounded-md px-3 py-1 text-xs font-medium transition-colors ${
              activeTab === 'ingestion'
                ? 'border border-accent-neuro/40 bg-accent-neuro/20 text-accent-neuro'
                : 'text-slate-400 hover:bg-bg-tertiary/60 hover:text-slate-200'
            }`}
          >
            Thu nạp VOD
          </button>
          <button
            type="button"
            role="tab"
            data-testid="tab-highlights"
            aria-selected={activeTab === 'highlights'}
            onClick={() => setActiveTab('highlights')}
            className={`rounded-md px-3 py-1 text-xs font-medium transition-colors ${
              activeTab === 'highlights'
                ? 'border border-accent-neuro/40 bg-accent-neuro/20 text-accent-neuro'
                : 'text-slate-400 hover:bg-bg-tertiary/60 hover:text-slate-200'
            }`}
          >
            Highlight
          </button>
          <button
            type="button"
            role="tab"
            data-testid="tab-studio"
            aria-selected={activeTab === 'studio'}
            onClick={() => setActiveTab('studio')}
            className={`rounded-md px-3 py-1 text-xs font-medium transition-colors ${
              activeTab === 'studio'
                ? 'border border-accent-neuro/40 bg-accent-neuro/20 text-accent-neuro'
                : 'text-slate-400 hover:bg-bg-tertiary/60 hover:text-slate-200'
            }`}
          >
            Subtitle Studio
          </button>
        </div>

        <div className="flex items-center space-x-2 font-mono text-[11px] text-slate-400">
          {projectId && <span>Project: {projectId}</span>}
          {activeVodId && <span>VOD: {activeVodId}</span>}
          {isInsufficientSpace && (
            <span className="font-semibold text-accent-evil">Thiếu dung lượng đĩa</span>
          )}
        </div>
      </nav>

      {/* Main Workspace Body */}
      <main className="flex flex-1 flex-col space-y-6 overflow-y-auto p-6">
        {storageError && (
          <ErrorBanner
            error={storageError}
            onDismiss={clearStorageError}
            onRetry={handleRunPreflightCheck}
            title="Cảnh báo lưu trữ ổ đĩa"
          />
        )}

        {activeTab === 'overview' && (
          <>
            {/* Hero Section */}
            <div className="flex items-center justify-between rounded-xl border border-border-dark bg-bg-secondary/70 p-6 shadow-acrylic">
              <div className="space-y-2">
                <div className="flex items-center space-x-2">
                  <Sparkles className="h-6 w-6 text-accent-neuro" />
                  <h1 className="text-xl font-bold tracking-tight text-white">
                    Tauri v2 Native Desktop Shell
                  </h1>
                </div>
                <p className="max-w-2xl text-sm leading-relaxed text-slate-300">
                  Khung ứng dụng máy tính đa nền tảng tối ưu hóa cho Windows 10/11. Hệ thống giao
                  diện Acrylic Dark Theme phong cách VTuber Neuro-sama, tích hợp font chữ Be Vietnam
                  Pro và Inter tối ưu dấu tiếng Việt.
                </p>
              </div>

              <div className="flex flex-col items-end space-y-1 text-right">
                <span className="font-mono text-xs text-slate-400">Milestone Foundation</span>
                <span className="rounded-full border border-accent-neuro/40 bg-accent-neuro/20 px-2.5 py-1 text-xs font-semibold text-accent-neuro">
                  TASK-M4-01 Verified
                </span>
              </div>
            </div>

            {/* VTuber Neuro-sama Character Palette Badges */}
            <section className="space-y-3">
              <h2 className="flex items-center space-x-1.5 text-xs font-semibold uppercase tracking-wider text-slate-400">
                <Layers className="h-4 w-4 text-accent-neuro" />
                <span>Bảng Màu Nhân Vật VTuber & Giao Diện (Neuro-sama Lore Palette)</span>
              </h2>

              <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 md:grid-cols-5">
                <div className="flex items-center space-x-3 rounded-lg border border-border-dark bg-bg-secondary p-3">
                  <div
                    className="h-5 w-5 rounded-md shadow-sm"
                    style={{ backgroundColor: theme.accentNeuro }}
                  />
                  <div>
                    <p className="text-xs font-semibold text-white">Neuro-sama</p>
                    <p className="font-mono text-[10px] text-slate-400">{theme.accentNeuro}</p>
                  </div>
                </div>

                <div className="flex items-center space-x-3 rounded-lg border border-border-dark bg-bg-secondary p-3">
                  <div
                    className="h-5 w-5 rounded-md shadow-sm"
                    style={{ backgroundColor: theme.accentEvil }}
                  />
                  <div>
                    <p className="text-xs font-semibold text-white">Evil Neuro</p>
                    <p className="font-mono text-[10px] text-slate-400">{theme.accentEvil}</p>
                  </div>
                </div>

                <div className="flex items-center space-x-3 rounded-lg border border-border-dark bg-bg-secondary p-3">
                  <div
                    className="h-5 w-5 rounded-md shadow-sm"
                    style={{ backgroundColor: theme.accentVedal }}
                  />
                  <div>
                    <p className="text-xs font-semibold text-white">Vedal (Tutel)</p>
                    <p className="font-mono text-[10px] text-slate-400">{theme.accentVedal}</p>
                  </div>
                </div>

                <div className="flex items-center space-x-3 rounded-lg border border-border-dark bg-bg-secondary p-3">
                  <div
                    className="h-5 w-5 rounded-md shadow-sm"
                    style={{ backgroundColor: theme.accentGuest }}
                  />
                  <div>
                    <p className="text-xs font-semibold text-white">Khách mời (Gold)</p>
                    <p className="font-mono text-[10px] text-slate-400">{theme.accentGuest}</p>
                  </div>
                </div>

                <div className="flex items-center space-x-3 rounded-lg border border-border-dark bg-bg-secondary p-3">
                  <div
                    className="h-5 w-5 rounded-md shadow-sm"
                    style={{ backgroundColor: theme.accentChat }}
                  />
                  <div>
                    <p className="text-xs font-semibold text-white">Chat / Swarm</p>
                    <p className="font-mono text-[10px] text-slate-400">{theme.accentChat}</p>
                  </div>
                </div>
              </div>
            </section>

            {/* SLA and Architecture Metrics */}
            <section className="grid grid-cols-1 gap-4 md:grid-cols-3">
              <div className="space-y-2 rounded-xl border border-border-dark bg-bg-secondary p-4">
                <div className="flex items-center justify-between">
                  <span className="text-xs font-medium text-slate-400">Static Idle RAM</span>
                  <Cpu className="h-4 w-4 text-accent-vedal" />
                </div>
                <div className="text-2xl font-bold text-white">≤ 90 MB</div>
                <p className="text-[11px] leading-snug text-slate-400">
                  Kiểm soát bộ nhớ nghiêm ngặt qua Windows Task Manager khi chưa nạp video.
                </p>
              </div>

              <div className="space-y-2 rounded-xl border border-border-dark bg-bg-secondary p-4">
                <div className="flex items-center justify-between">
                  <span className="text-xs font-medium text-slate-400">Khởi Động Nhanh (TTI)</span>
                  <Activity className="h-4 w-4 text-accent-neuro" />
                </div>
                <div className="text-2xl font-bold text-white">≤ 800 ms</div>
                <p className="text-[11px] leading-snug text-slate-400">
                  Thời gian sẵn sàng tương tác trên WebView2 Windows 10/11.
                </p>
              </div>

              <div className="space-y-2 rounded-xl border border-border-dark bg-bg-secondary p-4">
                <div className="flex items-center justify-between">
                  <span className="text-xs font-medium text-slate-400">Độ Mượt Render</span>
                  <Monitor className="h-4 w-4 text-accent-chat" />
                </div>
                <div className="text-2xl font-bold text-white">60 FPS</div>
                <p className="text-[11px] leading-snug text-slate-400">
                  Duy trì ≥ 55 FPS ổn định khi kéo giãn thay đổi kích thước cửa sổ.
                </p>
              </div>
            </section>

            {/* Architectural Directives Checklist */}
            <section className="space-y-3 rounded-xl border border-border-dark bg-bg-secondary/50 p-5">
              <div className="flex items-center space-x-2">
                <ShieldCheck className="h-5 w-5 text-accent-vedal" />
                <h3 className="text-sm font-semibold text-white">
                  Tiêu Chuẩn Kỹ Thuật Foundation Bedrock
                </h3>
              </div>

              <div className="grid grid-cols-1 gap-2 text-xs text-slate-300 md:grid-cols-2">
                <div className="flex items-center space-x-2">
                  <span className="h-1.5 w-1.5 rounded-full bg-accent-vedal" />
                  <span>Rust Backend: Edition 2021, target x86_64-pc-windows-msvc</span>
                </div>
                <div className="flex items-center space-x-2">
                  <span className="h-1.5 w-1.5 rounded-full bg-accent-vedal" />
                  <span>Clippy Zero-Panic: #![deny(clippy::unwrap_used, clippy::panic)]</span>
                </div>
                <div className="flex items-center space-x-2">
                  <span className="h-1.5 w-1.5 rounded-full bg-accent-vedal" />
                  <span>Process Supervision: Windows Job Object Kill-on-Drop</span>
                </div>
                <div className="flex items-center space-x-2">
                  <span className="h-1.5 w-1.5 rounded-full bg-accent-vedal" />
                  <span>TypeScript Strict: noImplicitAny, exactOptionalPropertyTypes</span>
                </div>
              </div>
            </section>
          </>
        )}

        {activeTab === 'ingestion' && (
          <ErrorBoundary title="Sự Cố Bảng Thu Nạp">
            <div className="space-y-4">
              <div className="flex flex-wrap items-center justify-between gap-2 rounded-xl border border-border-dark bg-bg-secondary/70 p-3">
                <div className="flex items-center space-x-2 text-xs text-slate-300">
                  <HardDrive className="h-4 w-4 text-accent-vedal" />
                  <span>Kiểm tra không gian lưu trữ &amp; trích xuất Chat Replay</span>
                </div>
                <div className="flex items-center space-x-2">
                  <button
                    type="button"
                    onClick={handleRunPreflightCheck}
                    disabled={isCheckingStorage}
                    className="rounded-lg border border-border-dark bg-bg-tertiary px-3 py-1.5 text-xs text-slate-200 transition-colors hover:bg-slate-700 disabled:opacity-50"
                  >
                    {isCheckingStorage ? 'Đang kiểm tra ổ đĩa...' : 'Kiểm tra ổ đĩa'}
                  </button>
                  {activeVodId && !isLocalFileManifest && ingestionUrl.trim().length > 0 && (
                    <button
                      type="button"
                      onClick={handleExtractChat}
                      disabled={isExtractingChat}
                      className="flex items-center space-x-1.5 rounded-lg border border-accent-chat/40 bg-accent-chat/20 px-3 py-1.5 text-xs text-accent-chat transition-colors hover:bg-accent-chat/30 disabled:opacity-50"
                    >
                      <MessageSquare className="h-3.5 w-3.5" />
                      <span>
                        {isExtractingChat ? 'Đang trích xuất Chat...' : 'Trích xuất Chat'}
                      </span>
                    </button>
                  )}
                </div>
              </div>

              {isExtractingChat && (
                <div
                  data-testid="chat-progress-banner"
                  className="flex items-center justify-between rounded-lg border border-accent-chat/40 bg-bg-secondary p-3"
                >
                  <div className="flex flex-1 items-center space-x-3 pr-4">
                    <RefreshCw className="h-4 w-4 animate-spin text-accent-chat" />
                    <div className="flex-1 text-xs text-slate-200">
                      <div className="flex flex-wrap items-center justify-between gap-2">
                        <span>Đang trích xuất Chat Replay...</span>
                        <span className="font-mono text-accent-chat">
                          {chatProgress?.messagesExtracted ?? 0} tin nhắn (
                          {formatChatOffset(chatProgress?.currentOffsetSec ?? 0)})
                        </span>
                      </div>
                    </div>
                  </div>
                  <button
                    type="button"
                    data-testid="cancel-chat-button"
                    onClick={() => {
                      void cancelChat().catch(() => {});
                    }}
                    className="rounded border border-red-500/40 bg-red-500/10 px-2.5 py-1 text-xs text-red-400 hover:bg-red-500/20"
                  >
                    Hủy
                  </button>
                </div>
              )}

              {!isExtractingChat && hasChatReplay && (
                <div
                  role="status"
                  data-testid="chat-ready-badge"
                  className="flex items-center justify-between rounded-lg border border-accent-vedal/40 bg-accent-vedal/10 p-3 text-xs text-accent-vedal"
                >
                  <div className="flex items-center space-x-2">
                    <ShieldCheck className="h-4 w-4 shrink-0 text-accent-vedal" />
                    <span>
                      Đã trích xuất Chat Replay
                      {chatHeatmap
                        ? `: ${chatHeatmap.points.reduce((sum, pt) => sum + pt.messageCount, 0)} tin nhắn (peak ${formatChatOffset(chatHeatmap.peakTimestampSec)})`
                        : chatProgress && chatProgress.messagesExtracted > 0
                          ? `: ${chatProgress.messagesExtracted} tin nhắn`
                          : ''}
                    </span>
                  </div>
                </div>
              )}

              {chatFallbackNotice && (
                <div
                  role="status"
                  className="flex items-center justify-between rounded-lg border border-amber-500/30 bg-amber-500/10 p-3 text-xs text-amber-200"
                >
                  <span>{chatFallbackNotice.message}</span>
                  <button
                    type="button"
                    onClick={clearFallbackNotice}
                    className="ml-3 text-amber-100 underline"
                  >
                    Đóng
                  </button>
                </div>
              )}

              {chatError && (
                <ErrorBanner
                  error={chatError}
                  onDismiss={clearChatError}
                  title="Lỗi trích xuất Chat Replay"
                />
              )}

              <IngestionDashboard onIngestSuccess={handleManifestReady} />
              <DropZone onImportSuccess={handleManifestReady} customCacheDir={customCacheDir} />
            </div>
          </ErrorBoundary>
        )}

        {activeTab === 'highlights' && (
          <ErrorBoundary title="Sự Cố Khung Highlight">
            <div className="space-y-4">
              {highlightError && (
                <ErrorBanner
                  error={highlightError}
                  onDismiss={clearHighlightError}
                  title="Lỗi phân tích Highlight"
                />
              )}

              {sliceError && (
                <ErrorBanner
                  error={sliceError}
                  onDismiss={() => setSliceError(null)}
                  title="Lỗi cắt phân đoạn video"
                />
              )}

              {isSlicing && sliceProgress && (
                <div
                  data-testid="slice-progress-banner"
                  className="flex items-center justify-between rounded-lg border border-accent-vedal/40 bg-bg-secondary p-3"
                >
                  <div className="flex flex-1 items-center space-x-3 pr-4">
                    <RefreshCw className="h-4 w-4 animate-spin text-accent-vedal" />
                    <div className="flex-1">
                      <div className="flex justify-between text-xs text-slate-300">
                        <span>Đang cắt phân đoạn: {sliceProgress.stage}</span>
                        <span>{Math.round(sliceProgress.percent)}%</span>
                      </div>
                      <div className="mt-1.5 h-1.5 w-full overflow-hidden rounded-full bg-bg-primary">
                        <div
                          className="h-full bg-accent-vedal transition-all duration-300"
                          style={{
                            width: `${Math.min(100, Math.max(0, sliceProgress.percent))}%`,
                          }}
                        />
                      </div>
                    </div>
                  </div>
                  <button
                    type="button"
                    data-testid="cancel-slice-button"
                    onClick={() => {
                      void handleCancelSlice();
                    }}
                    className="rounded border border-red-500/40 bg-red-500/10 px-2.5 py-1 text-xs text-red-400 hover:bg-red-500/20"
                  >
                    Hủy
                  </button>
                </div>
              )}

              {activeVodId && (
                <div className="flex items-center justify-end gap-3">
                  {!canDetectHighlights && (
                    <span className="text-[11px] text-amber-300">
                      Cần nạp VOD có thời lượng hợp lệ (&gt; 0s) trước khi phân tích Highlight.
                    </span>
                  )}
                  <button
                    type="button"
                    onClick={handleDetectHighlights}
                    disabled={isLoadingHighlights || !canDetectHighlights}
                    title={
                      canDetectHighlights
                        ? 'Phân tích khoảnh khắc Highlight từ Chat và Âm thanh'
                        : 'Cần nạp VOD manifest có thời lượng > 0s để phân tích Highlight'
                    }
                    className="rounded-lg border border-accent-neuro/40 bg-accent-neuro/20 px-3 py-1.5 text-xs text-accent-neuro transition-colors hover:bg-accent-neuro/30 disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    Phân tích Highlight
                  </button>
                </div>
              )}

              <HighlightViewer
                vodId={activeVodId ?? undefined}
                clusters={clusters}
                selectedClusterId={selectedClusterId}
                isLoading={isLoadingHighlights}
                onSelectCluster={(cluster) => selectCluster(cluster.clusterId)}
                onSliceCluster={(cluster) => {
                  void handleSliceCluster(cluster);
                }}
                isSlicing={isSlicing}
                slicingClusterId={slicingClusterId}
                onToggleBookmark={(clusterId, nextBookmarked) => {
                  void toggleBookmark(clusterId, nextBookmarked).catch(() => {});
                }}
                onRefresh={
                  activeVodId
                    ? () => {
                        void fetchClustersForVod(activeVodId).catch(() => {});
                      }
                    : undefined
                }
              />
            </div>
          </ErrorBoundary>
        )}

        {activeTab === 'studio' && (
          <div className="-m-6 flex min-h-[600px] flex-1 flex-col overflow-hidden">
            <SubtitleStudio />
          </div>
        )}
      </main>

      {/* Bottom Status Bar */}
      <footer className="flex h-7 select-none items-center justify-between border-t border-border-dark bg-bg-secondary px-4 font-mono text-[11px] text-slate-400">
        <div className="flex items-center space-x-4">
          <span className="flex items-center space-x-1.5">
            <span className="h-2 w-2 rounded-full bg-accent-vedal" />
            <span>Tauri v2 Desktop Shell Ready</span>
          </span>
          <span>|</span>
          <span>SLA Idle RAM: &lt;90MB</span>
          <span>|</span>
          <span>Theme: {isAcrylicSupported ? 'Acrylic / Mica' : 'Solid Fallback #12131C'}</span>
        </div>

        <div className="flex items-center space-x-3">
          <span className="font-semibold text-accent-neuro">NeuroClip Studio v1.0.0</span>
          <RefreshCw
            className="h-3 w-3 animate-spin text-slate-500"
            style={{ animationDuration: '8s' }}
          />
        </div>
      </footer>

      {/* Onboarding Wizard Modal (UT-28.1 / AC-28.3) */}
      <OnboardingWizardModal isOpen={isOnboardingOpen} onClose={() => setIsOnboardingOpen(false)} />
    </div>
  );
};

export default App;
