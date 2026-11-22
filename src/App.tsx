import React, { useState, useEffect } from 'react';
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
import { SubtitleStudio } from './components/studio/SubtitleStudio.tsx';
import { useProjectStore } from './stores/useProjectStore.ts';
import { useIngestionStore } from './stores/useIngestionStore.ts';
import { useChatStore } from './stores/useChatStore.ts';
import { useHighlightStore } from './stores/useHighlightStore.ts';
import type { VodManifest } from './services/ipc/ingestion.ts';

type AppTab = 'overview' | 'ingestion' | 'highlights' | 'studio';

export const App: React.FC = () => {
  const theme = loadTheme();
  // Assume acrylic is supported if Windows 11/10 21H2+ is present, otherwise fallback
  const [isAcrylicSupported, setIsAcrylicSupported] = useState<boolean>(true);
  const [backdropConfig, setBackdropConfig] = useState<WindowBackdropConfig>(() =>
    resolveBackdropConfig(true),
  );
  const [activeTab, setActiveTab] = useState<AppTab>('overview');

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

  const chatVodId = useChatStore((s) => s.vodId);
  const isExtractingChat = useChatStore((s) => s.isExtracting);
  const chatFallbackNotice = useChatStore((s) => s.fallbackNotice);
  const chatError = useChatStore((s) => s.error);
  const extractChat = useChatStore((s) => s.extractChat);
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

  useEffect(() => {
    setBackdropConfig(resolveBackdropConfig(isAcrylicSupported));
  }, [isAcrylicSupported]);

  const handleManifestReady = (nextManifest: VodManifest): void => {
    useIngestionStore.setState({ manifest: nextManifest });
    useChatStore.getState().setVodId(nextManifest.vodId);
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
    if (!activeVodId || !ingestionUrl.trim()) return;
    void extractChat(activeVodId, ingestionUrl).catch(() => {});
  };

  const handleDetectHighlights = (): void => {
    if (!activeVodId) return;
    const duration = manifest?.durationSeconds ?? 0;
    const audioPath = manifest?.audioPath;
    void detectHighlightsForVod(activeVodId, duration, audioPath ? { audioPath } : undefined).catch(
      () => {},
    );
  };

  const handleWindowMinimize = (): void => {
    try {
      void import('@tauri-apps/api/window')
        .then(({ getCurrentWindow }) => {
          void getCurrentWindow()
            .minimize()
            .catch(() => {});
        })
        .catch(() => {});
    } catch {
      // Non-tauri browser environment fallback
    }
  };

  const handleWindowMaximize = (): void => {
    try {
      void import('@tauri-apps/api/window')
        .then(({ getCurrentWindow }) => {
          void getCurrentWindow()
            .toggleMaximize()
            .catch(() => {});
        })
        .catch(() => {});
    } catch {
      // Non-tauri browser environment fallback
    }
  };

  const handleWindowClose = (): void => {
    try {
      void import('@tauri-apps/api/window')
        .then(({ getCurrentWindow }) => {
          void getCurrentWindow()
            .close()
            .catch(() => {});
        })
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
                  {activeVodId && ingestionUrl.trim().length > 0 && (
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

              {activeVodId && (
                <div className="flex items-center justify-end">
                  <button
                    type="button"
                    onClick={handleDetectHighlights}
                    disabled={isLoadingHighlights}
                    className="rounded-lg border border-accent-neuro/40 bg-accent-neuro/20 px-3 py-1.5 text-xs text-accent-neuro transition-colors hover:bg-accent-neuro/30 disabled:opacity-50"
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
          <span className="font-semibold text-accent-neuro">NeuroClip Studio v0.1.0</span>
          <RefreshCw
            className="h-3 w-3 animate-spin text-slate-500"
            style={{ animationDuration: '8s' }}
          />
        </div>
      </footer>
    </div>
  );
};

export default App;
