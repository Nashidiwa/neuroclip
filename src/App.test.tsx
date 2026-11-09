import fs from 'node:fs';
import path from 'node:path';
import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import App from './App';
import {
  loadTheme,
  resolveBackdropConfig,
  THEME_PALETTE,
  WINDOW_GEOMETRY,
  getWindowGeometry,
} from './styles/theme';

describe('TASK-M4-01: Tauri Desktop Shell & Acrylic Dark Theme', () => {
  describe('UT-01.1: Theme Palette Loading (Happy Path)', () => {
    it('should return exactly the specified Neuro-sama VTuber theme palette hex colors', () => {
      const theme = loadTheme();

      // Primary & Secondary dark backgrounds
      expect(theme.bgPrimary.toUpperCase()).toBe('#12131C');
      expect(theme.bgSecondary.toUpperCase()).toBe('#1A1C28');
      expect(theme.bgTertiary.toUpperCase()).toBe('#242738');

      // VTuber character accent colors
      expect(theme.accentNeuro.toUpperCase()).toBe('#FFB6C1'); // Neuro Pastel Pink
      expect(theme.accentEvil.toUpperCase()).toBe('#FF0055');  // Evil Red
      expect(theme.accentVedal.toUpperCase()).toBe('#98FB98'); // Vedal Green
      expect(theme.accentGuest.toUpperCase()).toBe('#FFD700'); // Guest Gold
      expect(theme.accentChat.toUpperCase()).toBe('#87CEEB');  // Chat Sky Blue

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
});
