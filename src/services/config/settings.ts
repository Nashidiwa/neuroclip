/**
 * Application Configuration Data Contract & IPC Service Wrapper
 * Single Source of Truth for NeuroClip User Settings & Defaults.
 * Strict adherence to docs/CONVENTIONS.md (zero-any, exactOptionalPropertyTypes).
 */

import { invoke } from '@tauri-apps/api/core';
import { parseIpcError } from '../ipc/error.ts';

// ---------------------------------------------------------------------------
// Canonical Types
// ---------------------------------------------------------------------------

export type ThemeMode = 'dark' | 'light' | 'acrylic' | 'system';
export type Language = 'vi' | 'en';
export type SttProvider = 'groq' | 'local_whisper' | 'cloud_byok';

// ---------------------------------------------------------------------------
// Wire Format (snake_case - Serialization Contract with Rust Backend)
// ---------------------------------------------------------------------------

export interface SidecarPathsJson {
  readonly ffmpeg_path: string | null;
  readonly ytdlp_path: string | null;
  readonly whisper_worker_path: string | null;
}

export interface AppConfigJson {
  readonly theme: string;
  readonly language: string;
  readonly telemetry_enabled: boolean;
  readonly auto_update_sidecars: boolean;
  readonly output_directory: string;
  readonly cache_directory: string;
  readonly audio_buffer_size_kb: number;
  readonly active_stt_provider: string;
  readonly active_stt_model: string;
  readonly active_translation_model: string;
  readonly sidecar_paths: SidecarPathsJson;
  readonly groq_api_key: string;
  readonly gemini_api_key: string;
  readonly max_history_days: number;
}

// ---------------------------------------------------------------------------
// Frontend Domain Model (camelCase)
// ---------------------------------------------------------------------------

export interface SidecarPaths {
  readonly ffmpegPath?: string | undefined;
  readonly ytdlpPath?: string | undefined;
  readonly whisperWorkerPath?: string | undefined;
}

export interface AppConfig {
  readonly theme: ThemeMode;
  readonly language: Language;
  readonly telemetryEnabled: boolean;
  readonly autoUpdateSidecars: boolean;
  readonly outputDirectory: string;
  readonly cacheDirectory: string;
  readonly audioBufferSizeKb: number;
  readonly activeSttProvider: SttProvider;
  readonly activeSttModel: string;
  readonly activeTranslationModel: string;
  readonly sidecarPaths: SidecarPaths;
  readonly groqApiKey: string;
  readonly geminiApiKey: string;
  readonly maxHistoryDays: number;
}

// ---------------------------------------------------------------------------
// Frozen Default App Configuration
// ---------------------------------------------------------------------------

export const DEFAULT_APP_CONFIG: Readonly<AppConfig> = Object.freeze({
  theme: 'dark',
  language: 'vi',
  telemetryEnabled: false,
  autoUpdateSidecars: true,
  outputDirectory: '',
  cacheDirectory: '',
  audioBufferSizeKb: 64,
  activeSttProvider: 'groq',
  activeSttModel: 'whisper-large-v3',
  activeTranslationModel: 'gemini-1.5-flash',
  sidecarPaths: Object.freeze({}),
  groqApiKey: '',
  geminiApiKey: '',
  maxHistoryDays: 14,
});

// ---------------------------------------------------------------------------
// Zero-any Runtime Type Guards & Validators
// ---------------------------------------------------------------------------

export function isThemeMode(val: unknown): val is ThemeMode {
  return val === 'dark' || val === 'light' || val === 'acrylic' || val === 'system';
}

export function isLanguage(val: unknown): val is Language {
  return val === 'vi' || val === 'en';
}

export function isSttProvider(val: unknown): val is SttProvider {
  return val === 'groq' || val === 'local_whisper' || val === 'cloud_byok';
}

export function isAppConfig(val: unknown): val is AppConfig {
  if (typeof val !== 'object' || val === null) return false;
  const obj = val as Record<string, unknown>;

  return (
    isThemeMode(obj['theme']) &&
    isLanguage(obj['language']) &&
    typeof obj['telemetryEnabled'] === 'boolean' &&
    typeof obj['autoUpdateSidecars'] === 'boolean' &&
    typeof obj['outputDirectory'] === 'string' &&
    typeof obj['cacheDirectory'] === 'string' &&
    typeof obj['audioBufferSizeKb'] === 'number' &&
    isSttProvider(obj['activeSttProvider']) &&
    typeof obj['activeSttModel'] === 'string' &&
    typeof obj['activeTranslationModel'] === 'string' &&
    typeof obj['sidecarPaths'] === 'object' &&
    obj['sidecarPaths'] !== null &&
    typeof obj['groqApiKey'] === 'string' &&
    typeof obj['geminiApiKey'] === 'string' &&
    typeof obj['maxHistoryDays'] === 'number'
  );
}

// ---------------------------------------------------------------------------
// Lossless Bidirectional Serialization Mappers
// ---------------------------------------------------------------------------

export function toAppConfig(raw: AppConfigJson | AppConfig): AppConfig {
  if ('telemetryEnabled' in raw) {
    return raw;
  }

  const rawSidecars = raw.sidecar_paths ?? {
    ffmpeg_path: null,
    ytdlp_path: null,
    whisper_worker_path: null,
  };

  const sidecarPaths: SidecarPaths = {
    ...(typeof rawSidecars.ffmpeg_path === 'string' && rawSidecars.ffmpeg_path.length > 0
      ? { ffmpegPath: rawSidecars.ffmpeg_path }
      : {}),
    ...(typeof rawSidecars.ytdlp_path === 'string' && rawSidecars.ytdlp_path.length > 0
      ? { ytdlpPath: rawSidecars.ytdlp_path }
      : {}),
    ...(typeof rawSidecars.whisper_worker_path === 'string' &&
    rawSidecars.whisper_worker_path.length > 0
      ? { whisperWorkerPath: rawSidecars.whisper_worker_path }
      : {}),
  };

  const theme: ThemeMode = isThemeMode(raw.theme) ? raw.theme : 'dark';
  const language: Language = isLanguage(raw.language) ? raw.language : 'vi';
  const activeSttProvider: SttProvider = isSttProvider(raw.active_stt_provider)
    ? raw.active_stt_provider
    : 'groq';

  return {
    theme,
    language,
    telemetryEnabled: raw.telemetry_enabled,
    autoUpdateSidecars: raw.auto_update_sidecars,
    outputDirectory: raw.output_directory,
    cacheDirectory: raw.cache_directory,
    audioBufferSizeKb: raw.audio_buffer_size_kb,
    activeSttProvider,
    activeSttModel: raw.active_stt_model,
    activeTranslationModel: raw.active_translation_model,
    sidecarPaths,
    groqApiKey: raw.groq_api_key,
    geminiApiKey: raw.gemini_api_key,
    maxHistoryDays: raw.max_history_days,
  };
}

export function toAppConfigJson(config: AppConfig): AppConfigJson {
  const sidecar_paths: SidecarPathsJson = {
    ffmpeg_path:
      typeof config.sidecarPaths?.ffmpegPath === 'string' &&
      config.sidecarPaths.ffmpegPath.length > 0
        ? config.sidecarPaths.ffmpegPath
        : null,
    ytdlp_path:
      typeof config.sidecarPaths?.ytdlpPath === 'string' &&
      config.sidecarPaths.ytdlpPath.length > 0
        ? config.sidecarPaths.ytdlpPath
        : null,
    whisper_worker_path:
      typeof config.sidecarPaths?.whisperWorkerPath === 'string' &&
      config.sidecarPaths.whisperWorkerPath.length > 0
        ? config.sidecarPaths.whisperWorkerPath
        : null,
  };

  return {
    theme: config.theme,
    language: config.language,
    telemetry_enabled: config.telemetryEnabled,
    auto_update_sidecars: config.autoUpdateSidecars,
    output_directory: config.outputDirectory,
    cache_directory: config.cacheDirectory,
    audio_buffer_size_kb: config.audioBufferSizeKb,
    active_stt_provider: config.activeSttProvider,
    active_stt_model: config.activeSttModel,
    active_translation_model: config.activeTranslationModel,
    sidecar_paths,
    groq_api_key: config.groqApiKey,
    gemini_api_key: config.geminiApiKey,
    max_history_days: config.maxHistoryDays,
  };
}

// ---------------------------------------------------------------------------
// IPC Invoke Wrappers
// ---------------------------------------------------------------------------

/**
 * Fetches the user application configuration from the backend.
 */
export async function fetchAppConfig(): Promise<AppConfig> {
  try {
    const raw = await invoke<AppConfigJson>('get_app_config');
    return toAppConfig(raw);
  } catch (err: unknown) {
    const parsed = parseIpcError(err, 'Không thể nạp tệp cấu hình ứng dụng');
    throw parsed;
  }
}

/**
 * Persists the application configuration to disk using atomic operations.
 */
export async function saveAppConfig(config: AppConfig): Promise<void> {
  try {
    const payload = toAppConfigJson(config);
    await invoke<void>('save_app_config', { config: payload });
  } catch (err: unknown) {
    const parsed = parseIpcError(err, 'Không thể lưu tệp cấu hình ứng dụng');
    throw parsed;
  }
}
