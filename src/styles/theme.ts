/**
 * NeuroClip VTuber Theme Palette and Window Backdrop Resolution
 * Single Source of Truth for Neuro-sama styling tokens.
 */

export interface ThemePalette {
  bgPrimary: string;
  bgSecondary: string;
  bgTertiary: string;
  accentNeuro: string;
  accentEvil: string;
  accentVedal: string;
  accentGuest: string;
  accentChat: string;
  border: string;
}

export const THEME_PALETTE: Readonly<ThemePalette> = Object.freeze({
  bgPrimary: '#12131C',
  bgSecondary: '#1A1C28',
  bgTertiary: '#242738',
  accentNeuro: '#FFB6C1',
  accentEvil: '#FF0055',
  accentVedal: '#98FB98',
  accentGuest: '#FFD700',
  accentChat: '#87CEEB',
  border: '#2E3247',
});

export interface WindowBackdropConfig {
  isAcrylicSupported: boolean;
  activeClassName: string;
  solidFallbackHex: string;
}

/**
 * Resolves window backdrop styling based on OS acrylic/mica support.
 * Falls back safely to solid background #12131C on legacy systems without crashing.
 */
export function resolveBackdropConfig(isAcrylicSupported: boolean): WindowBackdropConfig {
  return {
    isAcrylicSupported,
    activeClassName: isAcrylicSupported ? 'acrylic-window' : 'solid-window-fallback',
    solidFallbackHex: THEME_PALETTE.bgPrimary,
  };
}

/**
 * Returns a copy of the theme palette constants.
 */
export function loadTheme(): ThemePalette {
  return { ...THEME_PALETTE };
}

export interface WindowGeometry {
  defaultWidth: number;
  defaultHeight: number;
  minWidth: number;
  minHeight: number;
}

/**
 * Tauri native desktop shell window geometry specifications.
 * Enforces SLA defined in docs/TASK_BOARD.md §3.1 (IT-01.1).
 */
export const WINDOW_GEOMETRY: Readonly<WindowGeometry> = Object.freeze({
  defaultWidth: 1280,
  defaultHeight: 800,
  minWidth: 1024,
  minHeight: 640,
});

/**
 * Returns a copy of the window geometry constants.
 */
export function getWindowGeometry(): WindowGeometry {
  return { ...WINDOW_GEOMETRY };
}

