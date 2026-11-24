import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen, fireEvent, act, waitFor } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import App from './App';
import { GlobalErrorBoundary } from './components/common/ErrorBoundary';
import { useProjectStore } from './stores/useProjectStore';
import { useIngestionStore } from './stores/useIngestionStore';
import { useChatStore } from './stores/useChatStore';
import { useHighlightStore } from './stores/useHighlightStore';
import { useStudioStore } from './stores/useStudioStore';
import { useTranscriptStore } from './stores/useTranscriptStore';
import { useSettingsStore } from './stores/settingsStore';
import { normalizeHighlightCluster } from './types/highlight';
import { normalizeTranscriptSegmentsContainer } from './types/transcript';
import { ERROR_CODES } from './services/ipc/error';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
  convertFileSrc: vi.fn((filePath: string) => `asset://localhost/${filePath}`),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock('@tauri-apps/api/webviewWindow', () => ({
  getCurrentWebviewWindow: () => ({
    onDragDropEvent: vi.fn().mockResolvedValue(() => {}),
  }),
}));
import {
  loadTheme,
  resolveBackdropConfig,
  THEME_PALETTE,
  WINDOW_GEOMETRY,
  getWindowGeometry,
} from './styles/theme';

describe('TASK-M4-01: Tauri Desktop Shell & Acrylic Dark Theme', () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue(undefined);
    window.HTMLMediaElement.prototype.pause = vi.fn();
    window.HTMLMediaElement.prototype.play = vi.fn().mockResolvedValue(undefined);
    useProjectStore.getState().reset();
    useIngestionStore.getState().reset();
    useChatStore.getState().reset();
    useHighlightStore.getState().reset();
    useStudioStore.getState().reset();
    useTranscriptStore.getState().reset();
    useSettingsStore.setState({
      config: { ...useSettingsStore.getState().config, isFirstRun: false },
    });
  });
  describe('UT-01.1: Theme Palette Loading (Happy Path)', () => {
    it('should return exactly the specified Neuro-sama VTuber theme palette hex colors', () => {
      const theme = loadTheme();

      // Primary & Secondary dark backgrounds
      expect(theme.bgPrimary.toUpperCase()).toBe('#12131C');
      expect(theme.bgSecondary.toUpperCase()).toBe('#1A1C28');
      expect(theme.bgTertiary.toUpperCase()).toBe('#242738');

      // VTuber character accent colors
      expect(theme.accentNeuro.toUpperCase()).toBe('#FFB6C1'); // Neuro Pastel Pink
      expect(theme.accentEvil.toUpperCase()).toBe('#FF0055'); // Evil Red
      expect(theme.accentVedal.toUpperCase()).toBe('#98FB98'); // Vedal Green
      expect(theme.accentGuest.toUpperCase()).toBe('#FFD700'); // Guest Gold
      expect(theme.accentChat.toUpperCase()).toBe('#87CEEB'); // Chat Sky Blue

      // Border token
      expect(theme.border.toUpperCase()).toBe('#2E3247');
    });

    it('should maintain immutable frozen theme palette object', () => {
      expect(Object.isFrozen(THEME_PALETTE)).toBe(true);
    });
  });

  describe('UT-01.2: Acrylic Fallback to Solid #12131C (Failure / Legacy Path)', () => {
    it('should resolve to solid-window-fallback and #12131C when acrylic is not supported', () => {
      const config = resolveBackdropConfig(false);

      expect(config.isAcrylicSupported).toBe(false);
      expect(config.activeClassName).toBe('solid-window-fallback');
      expect(config.solidFallbackHex.toUpperCase()).toBe('#12131C');
    });

    it('should resolve to acrylic-window class when acrylic is supported', () => {
      const config = resolveBackdropConfig(true);

      expect(config.isAcrylicSupported).toBe(true);
      expect(config.activeClassName).toBe('acrylic-window');
      expect(config.solidFallbackHex.toUpperCase()).toBe('#12131C');
    });
  });

  describe('Desktop Shell UI Component & SLA Indicators', () => {
    it('should render the desktop shell layout with title and branding', () => {
      render(<App />);

      expect(screen.getByText('NEUROCLIP')).toBeInTheDocument();
      expect(screen.getByText(/AI Subtitle Studio/i)).toBeInTheDocument();
      expect(screen.getByText(/Tauri v2 Native Desktop Shell/i)).toBeInTheDocument();
    });

    it('should display the target NFR SLA metrics: Idle RAM <= 90MB and TTI <= 800ms', () => {
      render(<App />);

      expect(screen.getByText('≤ 90 MB')).toBeInTheDocument();
      expect(screen.getByText('≤ 800 ms')).toBeInTheDocument();
      expect(screen.getByText('60 FPS')).toBeInTheDocument();
    });

    it('should render all character palette cards', () => {
      render(<App />);

      expect(screen.getByText('Neuro-sama')).toBeInTheDocument();
      expect(screen.getByText('Evil Neuro')).toBeInTheDocument();
      expect(screen.getByText('Vedal (Tutel)')).toBeInTheDocument();
      expect(screen.getByText('Khách mời (Gold)')).toBeInTheDocument();
      expect(screen.getByText('Chat / Swarm')).toBeInTheDocument();
    });

    it('should toggle between Acrylic and Solid fallback when clicking button', () => {
      render(<App />);

      const toggleButton = screen.getByTitle('Chuyển đổi hiệu ứng Acrylic / Solid Fallback');
      expect(toggleButton).toHaveTextContent('Acrylic: ON');

      // Click to toggle to solid fallback
      fireEvent.click(toggleButton);
      expect(toggleButton).toHaveTextContent('Solid Fallback: #12131C');

      const appContainer = screen.getByTestId('app-container');
      expect(appContainer).toHaveClass('solid-window-fallback');

      // Click again to re-enable acrylic
      fireEvent.click(toggleButton);
      expect(toggleButton).toHaveTextContent('Acrylic: ON');
      expect(appContainer).toHaveClass('acrylic-window');
    });
  });

  describe('IT-01.1: Window Geometry Contract Validation', () => {
    it('should export immutable WINDOW_GEOMETRY matching desktop specification', () => {
      const geometry = getWindowGeometry();

      // 1. Verify frozen immutability of the production token
      expect(Object.isFrozen(WINDOW_GEOMETRY)).toBe(true);

      // 2. Verify SLA requirements from docs/TASK_BOARD.md §3.1
      expect(geometry.defaultWidth).toBe(1280);
      expect(geometry.defaultHeight).toBe(800);
      expect(geometry.minWidth).toBe(1024);
      expect(geometry.minHeight).toBe(640);
    });

    it('should strictly align with native configuration in src-tauri/tauri.conf.json', () => {
      interface TauriWindowContract {
        title?: string;
        width: number;
        height: number;
        minWidth: number;
        minHeight: number;
        transparent?: boolean;
        decorations?: boolean;
        windowEffects?: {
          effects: string[];
          state: string;
        };
      }

      interface TauriConfigContract {
        productName: string;
        version: string;
        identifier: string;
        app?: {
          windows?: TauriWindowContract[];
        };
      }

      const tauriConfPath = path.resolve(process.cwd(), 'src-tauri/tauri.conf.json');
      expect(fs.existsSync(tauriConfPath)).toBe(true);

      const rawContent = fs.readFileSync(tauriConfPath, 'utf8');
      const tauriConfig = JSON.parse(rawContent) as TauriConfigContract;

      const mainWindow = tauriConfig.app?.windows?.[0];
      expect(mainWindow).toBeDefined();

      if (mainWindow) {
        expect(mainWindow.width).toBe(WINDOW_GEOMETRY.defaultWidth);
        expect(mainWindow.height).toBe(WINDOW_GEOMETRY.defaultHeight);
        expect(mainWindow.minWidth).toBe(WINDOW_GEOMETRY.minWidth);
        expect(mainWindow.minHeight).toBe(WINDOW_GEOMETRY.minHeight);
        expect(mainWindow.transparent).toBe(true);
        expect(mainWindow.decorations).toBe(false);
        expect(mainWindow.windowEffects?.effects).toContain('acrylic');
        expect(mainWindow.windowEffects?.effects).toContain('mica');
      }
    });

    it('should render authentic window geometry indicators in Desktop Shell UI', () => {
      render(<App />);

      const expectedGeometryBadge = `Window: ${WINDOW_GEOMETRY.defaultWidth}x${WINDOW_GEOMETRY.defaultHeight} (Min: ${WINDOW_GEOMETRY.minWidth}x${WINDOW_GEOMETRY.minHeight})`;
      expect(screen.getByText(expectedGeometryBadge)).toBeInTheDocument();
    });
  });

  describe('Workspace Tab Wiring & GlobalErrorBoundary Integration', () => {
    it('switches to ingestion tab and mounts IngestionDashboard and DropZone', () => {
      render(<App />);

      const ingestionTab = screen.getByTestId('tab-ingestion');
      fireEvent.click(ingestionTab);

      expect(ingestionTab).toHaveAttribute('aria-selected', 'true');
      expect(screen.getByTestId('ingestion-dashboard')).toBeInTheDocument();
      expect(screen.getByTestId('local-dropzone-container')).toBeInTheDocument();
    });

    it('switches to highlights tab and renders HighlightViewer backed by useHighlightStore', () => {
      const sampleCluster = normalizeHighlightCluster({
        cluster_id: 'cluster_app_1',
        start_sec: 120,
        end_sec: 150,
        peak_sec: 135,
        score: 91.5,
        primary_reason: 'Swarm hype peak',
        top_emotes: ['NeuroHeart'],
        is_bookmarked: false,
      });

      useHighlightStore.setState({
        vodId: 'vod_app_test',
        clusters: [sampleCluster],
        selectedClusterId: 'cluster_app_1',
      });

      render(<App />);

      const highlightsTab = screen.getByTestId('tab-highlights');
      fireEvent.click(highlightsTab);

      expect(highlightsTab).toHaveAttribute('aria-selected', 'true');
      expect(
        screen.getByRole('region', { name: 'Khung xem highlight đa phương thức' }),
      ).toBeInTheDocument();
      expect(screen.getByText('Swarm hype peak')).toBeInTheDocument();
    });

    it('renders GlobalErrorBoundary fallback with Khôi phục phiên làm việc when child throws', () => {
      const consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
      const ThrowingChild: React.FC = () => {
        throw new Error('Render crash simulation');
      };

      try {
        render(
          <GlobalErrorBoundary>
            <ThrowingChild />
          </GlobalErrorBoundary>,
        );

        expect(screen.getByTestId('error-boundary-fallback')).toBeInTheDocument();
        expect(screen.getByText('Khôi phục phiên làm việc')).toBeInTheDocument();
      } finally {
        consoleErrorSpy.mockRestore();
      }
    });

    it('clears stale URL/error and hides Trích xuất Chat after importing a local_file manifest', async () => {
      const mockedInvoke = vi.mocked(invoke);
      mockedInvoke.mockImplementation(async (cmd) => {
        if (cmd === 'import_local_video') {
          return {
            vod_id: 'local_vod_99',
            source_type: 'local_file',
            original_url: null,
            title: 'local_clip.mp4',
            streamer: null,
            duration_seconds: 120,
            audio_path: 'C:\\cache\\local_vod_99.wav',
            chat_file_path: null,
            video_stream_url: null,
            local_video_path: 'D:\\Videos\\local_clip.mp4',
            created_at: '2026-11-22T00:00:00Z',
          };
        }
        return null;
      });

      useIngestionStore.setState({
        url: 'https://www.twitch.tv/videos/999999',
        error: {
          code: ERROR_CODES.INGEST_EXTRACTION_FAILED,
          message: 'Old error',
          recoverable: true,
        },
      });
      useChatStore.setState({ vodId: 'old_online_vod' });

      render(<App />);
      fireEvent.click(screen.getByTestId('tab-ingestion'));

      fireEvent.change(screen.getByTestId('dropzone-path-input'), {
        target: { value: 'D:\\Videos\\local_clip.mp4' },
      });
      fireEvent.click(screen.getByTestId('dropzone-path-submit'));

      await waitFor(() => {
        expect(useIngestionStore.getState().manifest?.vodId).toBe('local_vod_99');
      });

      expect(useIngestionStore.getState().url).toBe('');
      expect(useIngestionStore.getState().error).toBeNull();
      expect(screen.queryByRole('button', { name: /Trích xuất Chat/i })).not.toBeInTheDocument();
    });

    it('disables Phân tích Highlight when manifest duration <= 0 or unavailable', () => {
      const mockedInvoke = vi.mocked(invoke);
      useHighlightStore.setState({ vodId: 'vod_no_manifest' });

      render(<App />);
      fireEvent.click(screen.getByTestId('tab-highlights'));

      const detectBtn = screen.getByRole('button', { name: 'Phân tích Highlight' });
      expect(detectBtn).toBeDisabled();
      fireEvent.click(detectBtn);
      expect(mockedInvoke).not.toHaveBeenCalledWith('detect_highlights', expect.anything());
    });

    it('renders amber ERR-05 fallback notice without duplicate red ErrorBanner on ERR_INGEST_CHAT_UNAVAILABLE', async () => {
      const mockedInvoke = vi.mocked(invoke);
      mockedInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'start_chat_extraction') {
          throw {
            code: ERROR_CODES.INGEST_CHAT_UNAVAILABLE,
            message: 'Chat replay không khả dụng.',
            recoverable: true,
          };
        }
        return undefined;
      });

      useIngestionStore.setState({
        url: 'https://www.twitch.tv/videos/12345',
        manifest: {
          vodId: 'vod_chat_missing',
          sourceType: 'twitch_vod',
          originalUrl: 'https://www.twitch.tv/videos/12345',
          title: 'VOD',
          streamer: 'Vedal',
          durationSeconds: 300,
          audioPath: 'C:\\cache\\audio.m4a',
          chatFilePath: null,
          videoStreamUrl: null,
          localVideoPath: null,
          createdAt: '2026-11-22T00:00:00Z',
          vod_id: 'vod_chat_missing',
          source_type: 'twitch_vod',
          original_url: 'https://www.twitch.tv/videos/12345',
          duration_seconds: 300,
          audio_path: 'C:\\cache\\audio.m4a',
          chat_file_path: null,
          video_stream_url: null,
          local_video_path: null,
          created_at: '2026-11-22T00:00:00Z',
        },
      });

      render(<App />);
      fireEvent.click(screen.getByTestId('tab-ingestion'));

      await act(async () => {
        fireEvent.click(screen.getByRole('button', { name: /Trích xuất Chat/i }));
      });

      await waitFor(() => {
        expect(screen.getByRole('status')).toHaveTextContent('Chat replay không khả dụng.');
      });
      expect(screen.queryByText('Lỗi trích xuất Chat Replay')).not.toBeInTheDocument();
    });

    it('automatically displays OnboardingWizardModal when isFirstRun is true (UT-28.1)', async () => {
      useSettingsStore.setState({
        config: { ...useSettingsStore.getState().config, isFirstRun: true },
      });

      render(<App />);

      await waitFor(() => {
        expect(
          screen.getByRole('heading', { name: /Chào Mừng Đến Với NeuroClip Studio/i }),
        ).toBeInTheDocument();
      });
    });

    it('does not display OnboardingWizardModal when isFirstRun is false (UT-28.1)', () => {
      useSettingsStore.setState({
        config: { ...useSettingsStore.getState().config, isFirstRun: false },
      });

      render(<App />);

      expect(
        screen.queryByRole('heading', { name: /Chào Mừng Đến Với NeuroClip Studio/i }),
      ).not.toBeInTheDocument();
    });

    it('allows opening OnboardingWizardModal on-demand via Setup Wizard header button (AC-28.3)', async () => {
      useSettingsStore.setState({
        config: { ...useSettingsStore.getState().config, isFirstRun: false },
      });

      render(<App />);

      expect(
        screen.queryByRole('heading', { name: /Chào Mừng Đến Với NeuroClip Studio/i }),
      ).not.toBeInTheDocument();

      const wizardBtn = screen.getByRole('button', { name: /Setup Wizard/i });
      fireEvent.click(wizardBtn);

      await waitFor(() => {
        expect(
          screen.getByRole('heading', { name: /Chào Mừng Đến Với NeuroClip Studio/i }),
        ).toBeInTheDocument();
      });
    });
  });

  describe('TASK-M4-29B: End-to-End Pipeline Wiring (HighlightViewer → Range Slicer → SubtitleStudio → Export)', () => {
    it('UT-29B.1 / IT-29B.1: slices highlight cluster, populates studioStore dual videoSrc/videoPath, syncs transcript, navigates to studio, and passes raw OS path to exporter', async () => {
      const sampleCluster = normalizeHighlightCluster({
        cluster_id: 'cluster_1',
        start_sec: 10.0,
        end_sec: 25.0,
        peak_sec: 18.0,
        score: 85.5,
        primary_reason: 'chat_velocity',
        top_emotes: ['KEKW', 'LUL'],
        is_bookmarked: false,
      });

      useHighlightStore.setState({
        vodId: 'vod_test_123',
        clusters: [sampleCluster],
      });

      useTranscriptStore.setState({
        container: normalizeTranscriptSegmentsContainer({
          clip_id: 'clip_vod_test_123',
          segments: [
            {
              segment_id: 'seg_1',
              start_sec: 10.5,
              end_sec: 15.0,
              speaker_id: 'neuro',
              text_en: 'Hello world!',
              text_vi: 'Xin chào thế giới!',
              confidence: 0.99,
              cps_score: 12.0,
              is_flagged_for_review: false,
              words: [],
            },
          ],
        }),
      });

      let capturedExportArgs: unknown = null;
      vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
        if (cmd === 'slice_video_segment') {
          return {
            vod_id: 'vod_test_123',
            clip_path: 'C:\\Videos\\output_clip_01.mp4',
            start_sec: 10.0,
            end_sec: 25.0,
            duration_seconds: 15.0,
            file_size_bytes: 1048576,
            stream_drift_ms: 0.0,
            format: 'mp4',
            keyframe_aligned: true,
            initial_pts_delay_sec: 0.0,
            cluster_id: 'cluster_1',
          };
        }
        if (cmd === 'export_capcut_draft') {
          capturedExportArgs = args;
          return {
            project_dir: 'C:\\CapCut\\Projects\\draft_01',
            project_path: 'C:\\CapCut\\Projects\\draft_01',
            draft_name: 'draft_01',
            draft_content_path: 'C:\\CapCut\\Projects\\draft_01\\draft_content.json',
          };
        }
        return undefined;
      });

      render(<App />);

      // 1. Navigate to Highlights tab
      const highlightsTabBtn = screen.getByTestId('tab-highlights');
      fireEvent.click(highlightsTabBtn);

      // 2. Locate and click "Cắt Clip" button
      const sliceBtn = await screen.findByTestId('slice-cluster-cluster_1');
      expect(sliceBtn).toBeInTheDocument();
      fireEvent.click(sliceBtn);

      // 3. Verify slice_video_segment IPC was invoked with correct range parameters
      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith('slice_video_segment', {
          vodId: 'vod_test_123',
          startSec: 10.0,
          endSec: 25.0,
          clusterId: 'cluster_1',
          outputFilename: null,
          customInputSource: null,
        });
      });

      // 4. Verify useStudioStore contains asset:// URL in videoSrc and raw OS path in videoPath
      await waitFor(() => {
        const studioState = useStudioStore.getState();
        expect(studioState.videoSrc).toContain('asset://');
        expect(studioState.videoPath).toBe('C:\\Videos\\output_clip_01.mp4');
      });

      // 5. Verify transcript segments were synchronized to studioStore
      const studioState = useStudioStore.getState();
      expect(studioState.segments.length).toBe(1);
      expect(studioState.segments[0]?.textVi).toBe('Xin chào thế giới!');

      // 6. Verify automatically navigated to Subtitle Studio tab
      await waitFor(() => {
        expect(screen.getByTestId('studio-export-button')).toBeInTheDocument();
      });

      // 7. Click "Xuất bản" to open ExportModal
      fireEvent.click(screen.getByTestId('studio-export-button'));
      await waitFor(() => {
        expect(screen.getByTestId('export-modal-title')).toBeInTheDocument();
      });

      // 8. Submit export (CapCut format default)
      fireEvent.click(screen.getByTestId('export-submit-button'));
      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith('export_capcut_draft', expect.anything());
      });

      // 9. Assert raw OS path was provided to exporter, with zero "asset://" prefix
      expect(capturedExportArgs).toBeDefined();
      const exportOptions = (capturedExportArgs as Record<string, unknown>)['options'] as Record<
        string,
        unknown
      >;
      expect(exportOptions['video_file_path']).toBe('C:\\Videos\\output_clip_01.mp4');
      expect(String(exportOptions['video_file_path'])).not.toContain('asset://');
    });

    it('UT-29B.2: renders progress banner and handles slice cancellation via cancelSliceVideoSegment', async () => {
      let progressCallback: ((event: { payload: unknown }) => void) | undefined;
      vi.mocked(listen).mockImplementation(async (event: string, cb: unknown) => {
        if (event === 'slice:progress') {
          progressCallback = cb as (event: { payload: unknown }) => void;
        }
        return () => {};
      });

      const sampleCluster = normalizeHighlightCluster({
        cluster_id: 'cluster_2',
        start_sec: 50.0,
        end_sec: 70.0,
        peak_sec: 60.0,
        score: 90.0,
        primary_reason: 'audio_energy',
        top_emotes: [],
        is_bookmarked: false,
      });

      useHighlightStore.setState({
        vodId: 'vod_test_cancel',
        clusters: [sampleCluster],
      });

      let resolveSlice: (val: unknown) => void;
      const slicePromise = new Promise((resolve) => {
        resolveSlice = resolve;
      });

      vi.mocked(invoke).mockImplementation(async (cmd: string) => {
        if (cmd === 'slice_video_segment') {
          return await slicePromise;
        }
        if (cmd === 'cancel_slice_video_segment') {
          return true;
        }
        return undefined;
      });

      render(<App />);
      fireEvent.click(screen.getByTestId('tab-highlights'));

      const sliceBtn = await screen.findByTestId('slice-cluster-cluster_2');
      fireEvent.click(sliceBtn);

      await waitFor(() => {
        expect(screen.getByTestId('slice-progress-banner')).toBeInTheDocument();
      });

      act(() => {
        progressCallback?.({
          payload: {
            taskId: 'task_1',
            stage: 'encoding',
            percent: 45.0,
          },
        });
      });

      expect(screen.getByText(/45%/)).toBeInTheDocument();

      const cancelBtn = screen.getByTestId('cancel-slice-button');
      fireEvent.click(cancelBtn);

      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith('cancel_slice_video_segment', expect.anything());
      });

      await act(async () => {
        resolveSlice!({
          vod_id: 'vod_test_cancel',
          clip_path: '',
          start_sec: 50.0,
          end_sec: 70.0,
          duration_sec: 20.0,
          format: 'mp4',
          keyframe_aligned: true,
          initial_pts_delay_sec: 0.0,
          cluster_id: 'cluster_2',
        });
        await Promise.resolve();
      });
    });

    it('UT-29B.3: displays sliceError banner when sliceVideoSegment rejects', async () => {
      const sampleCluster = normalizeHighlightCluster({
        cluster_id: 'cluster_err',
        start_sec: 5.0,
        end_sec: 15.0,
        peak_sec: 10.0,
        score: 75.0,
        primary_reason: 'chat_velocity',
        top_emotes: [],
        is_bookmarked: false,
      });

      useHighlightStore.setState({
        vodId: 'vod_test_err',
        clusters: [sampleCluster],
      });

      vi.mocked(invoke).mockImplementation(async (cmd: string) => {
        if (cmd === 'slice_video_segment') {
          throw {
            code: 'ERR_INGEST_SLICE_FAILED',
            message: 'FFmpeg stream copy error: keyframe misalignment',
          };
        }
        return undefined;
      });

      render(<App />);
      fireEvent.click(screen.getByTestId('tab-highlights'));

      const sliceBtn = await screen.findByTestId('slice-cluster-cluster_err');
      fireEvent.click(sliceBtn);

      await waitFor(() => {
        expect(screen.getByText('Lỗi cắt phân đoạn video')).toBeInTheDocument();
      });

      expect(screen.queryByTestId('studio-export-button')).not.toBeInTheDocument();
    });
  });
});
