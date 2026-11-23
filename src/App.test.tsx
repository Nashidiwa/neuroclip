import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen, fireEvent, act, waitFor } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import App from './App';
import { GlobalErrorBoundary } from './components/common/ErrorBoundary';
import { useProjectStore } from './stores/useProjectStore';
import { useIngestionStore } from './stores/useIngestionStore';
import { useChatStore } from './stores/useChatStore';
import { useHighlightStore } from './stores/useHighlightStore';
import { normalizeHighlightCluster } from './types/highlight';
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
    useProjectStore.getState().reset();
    useIngestionStore.getState().reset();
    useChatStore.getState().reset();
    useHighlightStore.getState().reset();
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
      mockedInvoke.mockRejectedValueOnce({
        code: ERROR_CODES.INGEST_CHAT_UNAVAILABLE,
        message: 'Chat replay không khả dụng.',
        recoverable: true,
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
  });
});
