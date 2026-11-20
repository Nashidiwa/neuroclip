/**
 * Supported subtitle and project export formats.
 */
export type SubtitleExportFormat = 'ass' | 'srt' | 'project_json';

/**
 * Validated target video aspect ratio / layout presets per Schema 5.6.
 */
export type TargetLayout = 'original_16_9' | 'preset_center_crop_9_16' | 'preset_two_stack_9_16';

/**
 * Export options for subtitle generation.
 */
export interface ExportSubtitlesOptions {
  /** Target layout preset (default: 'original_16_9') */
  readonly targetLayout?: TargetLayout | undefined;
  /** Custom project name metadata for Schema 5.6 */
  readonly projectName?: string | undefined;
  /** Associated VOD identifier */
  readonly vodId?: string | undefined;
}

/**
 * Result returned from simultaneous dual export (ADR-005).
 */
export interface DualExportResult {
  readonly assPath: string;
  readonly srtPath: string;
}

/**
 * Canonical PO Option 1 Character Palette in RGB Hex format (#RRGGBB).
 */
export const VTUBER_PALETTE_HEX = Object.freeze({
  vedal: '#98FB98',
  neuro: '#FFB6C1',
  evil: '#FF0055',
  guest: '#FFD700',
  chat: '#87CEEB',
  unknown: '#FFFFFF',
} as const);

/**
 * Canonical PO Option 1 Character Palette in Aegisub ASS BGR format (&HAABBGGRR&).
 */
export const VTUBER_PALETTE_ASS_BGR = Object.freeze({
  vedal: '&H0098FB98&',
  neuro: '&H00C1B6FF&',
  evil: '&H005500FF&',
  guest: '&H0000D7FF&',
  chat: '&H00EBCE87&',
  unknown: '&H00FFFFFF&',
} as const);

/**
 * Type guard for SubtitleExportFormat.
 */
export function isSubtitleExportFormat(value: unknown): value is SubtitleExportFormat {
  return typeof value === 'string' && (value === 'ass' || value === 'srt' || value === 'project_json');
}

/**
 * Type guard for TargetLayout.
 */
export function isTargetLayout(value: unknown): value is TargetLayout {
  return (
    typeof value === 'string' &&
    (value === 'original_16_9' ||
      value === 'preset_center_crop_9_16' ||
      value === 'preset_two_stack_9_16')
  );
}
