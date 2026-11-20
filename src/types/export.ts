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

/**
 * Options for CapCut PC draft export.
 */
export interface CapCutExportOptions {
  /** Optional override for video file path */
  readonly videoFilePath?: string | undefined;
  /** Optional custom draft root directory (overrides %LOCALAPPDATA%/CapCut/...) */
  readonly customDraftRoot?: string | undefined;
}

/**
 * Result returned from CapCut PC draft generation with Dual Export Safeguard (ADR-005).
 */
export interface CapCutExportResult {
  /** Absolute directory path of the created CapCut draft project */
  readonly projectDir: string;
  /** Absolute directory path alias */
  readonly projectPath: string;
  /** Folder name of the draft project (e.g. "NeuroClip_Highlight_20261120_100000") */
  readonly draftName: string;
  /** Path to draft_content.json */
  readonly draftContentPath: string;
  /** Path to draft_meta_info.json */
  readonly draftMetaInfoPath: string;
  /** Path to backup subtitle.ass */
  readonly assBackupPath: string;
  /** Path to backup subtitle.srt */
  readonly srtBackupPath: string;
  /** List of dual safeguard backup file paths generated (.ass, .srt) */
  readonly dualSafeguardFiles: readonly string[];
  /** Generation duration in milliseconds (SLA <= 200ms) */
  readonly durationMs: number;

  // Snake_case aliases matching Serde JSON serialization
  readonly project_dir?: string;
  readonly project_path?: string;
  readonly draft_name?: string;
  readonly draft_content_path?: string;
  readonly draft_meta_info_path?: string;
  readonly ass_backup_path?: string;
  readonly srt_backup_path?: string;
  readonly dual_safeguard_files?: readonly string[];
  readonly duration_ms?: number;
}

/**
 * Normalizes raw CapCut export result from Tauri IPC into validated domain model.
 */
export function normalizeCapCutExportResult(raw: unknown): CapCutExportResult {
  if (typeof raw !== 'object' || raw === null) {
    throw new Error('Invalid CapCutExportResult payload: expected an object');
  }

  const rec = raw as Record<string, unknown>;

  const projectDir =
    typeof rec.projectDir === 'string'
      ? rec.projectDir
      : typeof rec.projectPath === 'string'
        ? rec.projectPath
        : typeof rec.project_dir === 'string'
          ? rec.project_dir
          : typeof rec.project_path === 'string'
            ? rec.project_path
            : '';

  const projectPath = projectDir;

  const draftName =
    typeof rec.draftName === 'string'
      ? rec.draftName
      : typeof rec.draft_name === 'string'
        ? rec.draft_name
        : '';

  const draftContentPath =
    typeof rec.draftContentPath === 'string'
      ? rec.draftContentPath
      : typeof rec.draft_content_path === 'string'
        ? rec.draft_content_path
        : '';

  const draftMetaInfoPath =
    typeof rec.draftMetaInfoPath === 'string'
      ? rec.draftMetaInfoPath
      : typeof rec.draft_meta_info_path === 'string'
        ? rec.draft_meta_info_path
        : '';

  const assBackupPath =
    typeof rec.assBackupPath === 'string'
      ? rec.assBackupPath
      : typeof rec.ass_backup_path === 'string'
        ? rec.ass_backup_path
        : '';

  const srtBackupPath =
    typeof rec.srtBackupPath === 'string'
      ? rec.srtBackupPath
      : typeof rec.srt_backup_path === 'string'
        ? rec.srt_backup_path
        : '';

  const rawSafeguard = rec.dualSafeguardFiles ?? rec.dual_safeguard_files;
  const dualSafeguardFiles: string[] = Array.isArray(rawSafeguard)
    ? rawSafeguard.filter((item): item is string => typeof item === 'string')
    : [assBackupPath, srtBackupPath].filter((p) => p.length > 0);

  const durationMs =
    typeof rec.durationMs === 'number'
      ? rec.durationMs
      : typeof rec.duration_ms === 'number'
        ? rec.duration_ms
        : 0;

  return {
    projectDir,
    projectPath,
    draftName,
    draftContentPath,
    draftMetaInfoPath,
    assBackupPath,
    srtBackupPath,
    dualSafeguardFiles,
    durationMs,
    project_dir: projectDir,
    project_path: projectPath,
    draft_name: draftName,
    draft_content_path: draftContentPath,
    draft_meta_info_path: draftMetaInfoPath,
    ass_backup_path: assBackupPath,
    srt_backup_path: srtBackupPath,
    dual_safeguard_files: dualSafeguardFiles,
    duration_ms: durationMs,
  };
}
