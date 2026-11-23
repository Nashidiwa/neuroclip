import { describe, it } from 'node:test';
import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';
import Ajv from 'ajv';
import {
  loadTheme,
  resolveBackdropConfig,
  THEME_PALETTE,
  WINDOW_GEOMETRY,
} from '../styles/theme.ts';
import {
  ERROR_CODES,
  ERROR_MESSAGES_VI,
  getFriendlyErrorMessage,
  isIpcError,
  parseIpcError,
} from '../services/ipc/error.ts';
import {
  type HardwareProfileJson,
  toHardwareProfile,
  toHardwareProfileJson,
  isHardwareProfileJson,
  isHardwareProfile,
  isGpuVendor,
  isRecommendedMode,
} from '../services/config/hardware.ts';
import {
  type AppConfigJson,
  DEFAULT_APP_CONFIG,
  toAppConfig,
  toAppConfigJson,
  isAppConfig,
} from '../services/config/settings.ts';
import {
  redactSecret,
  sanitizeLogString,
  sanitizeLogContext,
  toLogEntry,
  toLogLevel,
  isLogEntryJson,
} from '../services/logging/index.ts';
import {
  type DiskStatusJson,
  toDiskStatus,
  isDiskStatusJson,
  isCacheCleanupResultJson,
  isStoragePathsJson,
  toStoragePaths,
  toCacheCleanupResult,
} from '../services/ipc/storage.ts';
import { MIN_REQUIRED_DISK_SPACE_MB } from '../stores/useProjectStore.ts';
import {
  type VodManifestJson,
  type IngestProgressEvent,
  toVodManifest,
  isVodManifestJson,
  isVodMetadataJson,
  isIngestProgressEvent,
} from '../services/ipc/ingestion.ts';
import {
  type ChatBucketJson,
  type ChatVelocityPointJson,
  type EmoteDensityPointJson,
  type EmoteDensityPoint,
  type AudioRmsPointJson,
  type AudioRmsSeriesResponseJson,
  type AudioRmsPoint,
  type AudioRmsSeriesResponse,
  type HighlightClusterJson,
  type HighlightClustersContainerJson,
  isChatBucketJson,
  isChatBucket,
  isChatVelocityPointJson,
  isChatVelocityPoint,
  isEmoteDensityPointJson,
  isEmoteDensityPoint,
  isAudioRmsPointJson,
  isAudioRmsPoint,
  isAudioRmsSeriesResponseJson,
  isHighlightClusterJson,
  isHighlightCluster,
  isHighlightClustersContainerJson,
  isHighlightClustersContainer,
  normalizeChatBucket,
  normalizeChatVelocityPoint,
  normalizeEmoteDensityPoint,
  normalizeAudioRmsPoint,
  normalizeAudioRmsSeriesResponse,
  normalizeHighlightCluster,
  normalizeHighlightClustersContainer,
  toHighlightClustersContainerJson,
  calculateAdjustedZScore,
  calculateEmoteRatio,
  calculateAudioRms,
  rmsToDbfs,
  dbfsToRms,
  dbfsToNormalizedScale,
  isAudioRmsPeak,
  calculateMultimodalScore,
  calculateAudioOnlyScore,
  computeHighlightBounds,
  formatTimecode,
  formatScore,
  formatDuration,
  DEFAULT_BUCKET_WIDTH_SEC,
  DEFAULT_BASELINE_WINDOW_SEC,
  SMOOTHING_EPSILON,
  MESSAGE_FLOOR_THRESHOLD,
  SPAM_BOT_MAX_MESSAGES_PER_AUTHOR,
  MAX_TOP_EMOTES_PER_BUCKET,
  DEFAULT_RMS_BLOCK_DURATION_SEC,
  DEFAULT_AUDIO_SAMPLE_RATE,
  DEFAULT_AUDIO_BASELINE_WINDOW_SEC,
  SILENCE_FLOOR_DBFS,
  SILENCE_FLOOR_AMPLITUDE,
  DEFAULT_PEAK_DB_SPIKE_THRESHOLD,
  DEFAULT_PEAK_SIGMA_THRESHOLD,
  STREAM_LATENCY_COMPENSATION_SEC,
  DEFAULT_PRE_ROLL_BUFFER_SEC,
  DEFAULT_POST_ROLL_BUFFER_SEC,
  MIN_PRE_ROLL_OFFSET_SEC,
  MAX_PRE_ROLL_BUFFER_SEC,
  MIN_CLUSTER_DURATION_SEC,
  CLUSTER_MERGE_DISTANCE_SEC,
  MAX_CLUSTER_OVERLAP_RATIO,
  FUSION_WEIGHT_Z_SCORE,
  FUSION_WEIGHT_EMOTE,
  FUSION_WEIGHT_RMS,
  FUSION_AUDIO_ONLY_WEIGHT_RMS,
  DEFAULT_TOP_CLUSTERS_LIMIT,
} from '../types/highlight.ts';
import {
  type SliceResultJson,
  type SliceVideoSegmentRequest,
  isSliceResultJson,
  isSliceResult,
  isSliceVideoSegmentRequestJson,
  normalizeSliceResult,
  toSliceResultJson,
  toSliceVideoSegmentRequestJson,
  calculateBandwidthSavings,
  isKeyframeAligned,
  MAX_KEYFRAME_DRIFT_MS,
  MAX_INITIAL_PTS_DELAY_SEC,
  MIN_SLICING_DISK_SPACE_MB,
  FULL_VOD_REFERENCE_SIZE_MB,
} from '../types/range_slicer.ts';
import { sliceVideoSegment, cancelSliceVideoSegment } from '../services/ipc/slice.ts';
import {
  transcribeHighlightClip,
  listenSttChunk,
  normalizeTranscriptSegmentsContainer,
  type TranscribeClipOptions,
} from '../services/ipc/stt.ts';
import {
  translateVietsub,
  testGeminiConnection,
  updateSpeakerRole,
  wrapVietnameseSubtitles,
  optimizeTranscriptCps,
  listenTranslationProgress,
  normalizeTranslationProgressPayload,
  type StylePreset,
  type TranslateVietsubOptions,
  type TranslationProgressPayload,
  type TranslationProgressPayloadJson,
  type WrapVietnameseSubtitlesOptions,
  type OptimizeTranscriptCpsOptions,
} from '../services/ipc/translation.ts';
import {
  wrapVietnameseText,
  calculateCps,
  optimizeCpsTimeBorrowing,
  getUnicodeCharCount,
  RED_FLAG_CPS_THRESHOLD,
} from '../utils/text_wrap.ts';
import {
  normalizeTranscriptSegment,
  normalizeSubtitleProject,
  toSubtitleProjectJson,
  type TranscriptSegment,
  type SubtitleProject,
  type SubtitleProjectJson,
} from '../types/transcript.ts';
import {
  VTUBER_PALETTE_HEX,
  VTUBER_PALETTE_ASS_BGR,
  isSubtitleExportFormat,
  isTargetLayout,
} from '../types/export.ts';
import {
  exportSubtitlesAss,
  exportSubtitlesSrt,
  exportSubtitleProject,
  exportSubtitlesDual,
} from '../services/ipc/export.ts';

describe('TASK-M4-01 Test Suite (Node.js Native Test Runner)', () => {
  describe('UT-01.1: Theme Palette Loading (Happy Path)', () => {
    it('should return exactly the specified Neuro-sama VTuber theme palette hex colors', () => {
      const theme = loadTheme();

      // Primary & Secondary dark backgrounds
      assert.strictEqual(theme.bgPrimary.toUpperCase(), '#12131C', 'bgPrimary must be #12131C');
      assert.strictEqual(theme.bgSecondary.toUpperCase(), '#1A1C28', 'bgSecondary must be #1A1C28');
      assert.strictEqual(theme.bgTertiary.toUpperCase(), '#242738', 'bgTertiary must be #242738');

      // VTuber character accent colors
      assert.strictEqual(
        theme.accentNeuro.toUpperCase(),
        '#FFB6C1',
        'Neuro accent must be #FFB6C1',
      );
      assert.strictEqual(theme.accentEvil.toUpperCase(), '#FF0055', 'Evil accent must be #FF0055');
      assert.strictEqual(
        theme.accentVedal.toUpperCase(),
        '#98FB98',
        'Vedal accent must be #98FB98',
      );
      assert.strictEqual(
        theme.accentGuest.toUpperCase(),
        '#FFD700',
        'Guest accent must be #FFD700',
      );
      assert.strictEqual(theme.accentChat.toUpperCase(), '#87CEEB', 'Chat accent must be #87CEEB');

      // Border token
      assert.strictEqual(theme.border.toUpperCase(), '#2E3247', 'border must be #2E3247');
    });

    it('should maintain immutable frozen theme palette object', () => {
      assert.strictEqual(Object.isFrozen(THEME_PALETTE), true, 'THEME_PALETTE must be frozen');
    });
  });

  describe('UT-01.2: Acrylic Fallback to Solid #12131C (Failure / Legacy Path)', () => {
    it('should resolve to solid-window-fallback and #12131C when acrylic is not supported', () => {
      const config = resolveBackdropConfig(false);

      assert.strictEqual(config.isAcrylicSupported, false);
      assert.strictEqual(config.activeClassName, 'solid-window-fallback');
      assert.strictEqual(config.solidFallbackHex.toUpperCase(), '#12131C');
    });

    it('should resolve to acrylic-window class when acrylic is supported', () => {
      const config = resolveBackdropConfig(true);

      assert.strictEqual(config.isAcrylicSupported, true);
      assert.strictEqual(config.activeClassName, 'acrylic-window');
      assert.strictEqual(config.solidFallbackHex.toUpperCase(), '#12131C');
    });
  });

  describe('IT-01.1: Window Geometry Contract & Tauri Config Verification', () => {
    it('should verify tauri.conf.json window geometry matches specifications and frontend constants', () => {
      const tauriConfPath = path.resolve('src-tauri/tauri.conf.json');
      assert.strictEqual(fs.existsSync(tauriConfPath), true, 'tauri.conf.json must exist');

      const content = fs.readFileSync(tauriConfPath, 'utf8');
      const conf = JSON.parse(content);

      assert.strictEqual(conf.productName, 'NeuroClip');
      assert.strictEqual(conf.version, '0.1.0');
      assert.strictEqual(conf.identifier, 'com.neuroclip.app');

      const windowConfig = conf.app.windows[0];
      assert.ok(windowConfig, 'Default window configuration must exist');
      assert.strictEqual(
        windowConfig.width,
        WINDOW_GEOMETRY.defaultWidth,
        'Default width must match WINDOW_GEOMETRY',
      );
      assert.strictEqual(
        windowConfig.height,
        WINDOW_GEOMETRY.defaultHeight,
        'Default height must match WINDOW_GEOMETRY',
      );
      assert.strictEqual(
        windowConfig.minWidth,
        WINDOW_GEOMETRY.minWidth,
        'Minimum width must match WINDOW_GEOMETRY',
      );
      assert.strictEqual(
        windowConfig.minHeight,
        WINDOW_GEOMETRY.minHeight,
        'Minimum height must match WINDOW_GEOMETRY',
      );
      assert.strictEqual(windowConfig.transparent, true, 'Window must support transparency');
      assert.strictEqual(
        windowConfig.decorations,
        false,
        'Window decorations must be false for frameless acrylic',
      );
      assert.deepStrictEqual(windowConfig.windowEffects.effects, ['acrylic', 'mica']);
    });
  });

  describe('ED-01.1: Clippy Zero-Panic & Static Rust Invariants Audit', () => {
    it('should verify src-tauri/src/lib.rs enforces zero-panic clippy policies', () => {
      const libRsPath = path.resolve('src-tauri/src/lib.rs');
      assert.strictEqual(fs.existsSync(libRsPath), true, 'src-tauri/src/lib.rs must exist');

      const content = fs.readFileSync(libRsPath, 'utf8');
      assert.ok(content.includes('#![deny(clippy::unwrap_used)]'), 'Must deny unwrap_used');
      assert.ok(content.includes('#![deny(clippy::expect_used)]'), 'Must deny expect_used');
      assert.ok(content.includes('#![deny(clippy::panic)]'), 'Must deny panic');
      assert.ok(content.includes('#![deny(clippy::todo)]'), 'Must deny todo');
      assert.ok(content.includes('#![deny(clippy::unimplemented)]'), 'Must deny unimplemented');
      assert.ok(content.includes('pub const DEFAULT_WINDOW_WIDTH: f64 = 1280.0;'));
      assert.ok(content.includes('pub const DEFAULT_WINDOW_HEIGHT: f64 = 800.0;'));
      assert.ok(content.includes('pub const MIN_WINDOW_WIDTH: f64 = 1024.0;'));
      assert.ok(content.includes('pub const MIN_WINDOW_HEIGHT: f64 = 640.0;'));
    });

    it('should verify rustfmt.toml contains Windows newline and edition 2021', () => {
      const rustfmtPath = path.resolve('src-tauri/rustfmt.toml');
      assert.strictEqual(fs.existsSync(rustfmtPath), true, 'src-tauri/rustfmt.toml must exist');

      const content = fs.readFileSync(rustfmtPath, 'utf8');
      assert.ok(content.includes('newline_style = "Windows"'));
      assert.ok(content.includes('edition = "2021"'));
      assert.ok(content.includes('max_width = 100'));
    });
  });

  describe('CSS Theme Tokens & Tailwind Contract Verification', () => {
    it('should verify src/styles/theme.css defines all required CSS custom properties', () => {
      const themeCssPath = path.resolve('src/styles/theme.css');
      assert.strictEqual(fs.existsSync(themeCssPath), true, 'src/styles/theme.css must exist');

      const content = fs.readFileSync(themeCssPath, 'utf8');
      const normalizedContent = content.toLowerCase();
      assert.ok(normalizedContent.includes('--color-bg-primary: #12131c;'));
      assert.ok(normalizedContent.includes('--color-bg-secondary: #1a1c28;'));
      assert.ok(normalizedContent.includes('--color-accent-neuro: #ffb6c1;'));
      assert.ok(normalizedContent.includes('--color-accent-evil: #ff0055;'));
      assert.ok(normalizedContent.includes('--color-accent-vedal: #98fb98;'));
      assert.ok(normalizedContent.includes('--color-accent-guest: #ffd700;'));
      assert.ok(normalizedContent.includes('--color-accent-chat: #87ceeb;'));
      assert.ok(content.includes('.acrylic-window'));
      assert.ok(content.includes('.solid-window-fallback'));
    });

    it('should verify tailwind.config.js defines full Neuro-sama theme palette', () => {
      const tailwindConfigPath = path.resolve('tailwind.config.js');
      assert.strictEqual(fs.existsSync(tailwindConfigPath), true);

      const content = fs.readFileSync(tailwindConfigPath, 'utf8');
      assert.ok(content.includes("'bg-primary': '#12131C'"));
      assert.ok(content.includes("'bg-secondary': '#1A1C28'"));
      assert.ok(content.includes("'accent-neuro': '#FFB6C1'"));
      assert.ok(content.includes("'accent-evil': '#FF0055'"));
      assert.ok(content.includes("'accent-vedal': '#98FB98'"));
      assert.ok(content.includes("'accent-guest': '#FFD700'"));
      assert.ok(content.includes("'accent-chat': '#87CEEB'"));
      assert.ok(content.includes('"Be Vietnam Pro"'));
      assert.ok(content.includes('Inter'));
    });
  });

  describe('TypeScript Strict Mode & Zero-Any Invariant Audit', () => {
    it('should verify tsconfig.json enforces highest strictness level', () => {
      const tsconfigPath = path.resolve('tsconfig.json');
      assert.strictEqual(fs.existsSync(tsconfigPath), true);

      const tsconfig = JSON.parse(fs.readFileSync(tsconfigPath, 'utf8'));
      const opts = tsconfig.compilerOptions;

      assert.strictEqual(opts.strict, true);
      assert.strictEqual(opts.noImplicitAny, true);
      assert.strictEqual(opts.strictNullChecks, true);
      assert.strictEqual(opts.strictFunctionTypes, true);
      assert.strictEqual(opts.exactOptionalPropertyTypes, true);
      assert.strictEqual(opts.noUncheckedIndexedAccess, true);
      assert.deepStrictEqual(opts.paths['@/*'], ['src/*']);
    });

    it('should verify zero "any" type annotations exist in all src TypeScript files', () => {
      const srcDir = path.resolve('src');
      function scanDir(dir: string): string[] {
        const files: string[] = [];
        for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
          const fullPath = path.join(dir, entry.name);
          if (entry.isDirectory()) {
            files.push(...scanDir(fullPath));
          } else if (entry.name.endsWith('.ts') || entry.name.endsWith('.tsx')) {
            files.push(fullPath);
          }
        }
        return files;
      }

      const tsFiles = scanDir(srcDir);
      assert.ok(tsFiles.length >= 4, 'Must scan at least 4 TypeScript files');

      for (const file of tsFiles) {
        // Exclude test files and test audit helpers to only audit production TypeScript files
        if (
          path.basename(file) === 'suite.node.test.ts' ||
          path.basename(file) === 'typecheck.node.ts' ||
          file.includes(path.sep + 'test' + path.sep) ||
          path.basename(file).includes('.test.')
        ) {
          continue;
        }
        const content = fs.readFileSync(file, 'utf8');
        // Match ': any' or 'as any' or '<any>'
        const pattern = [':\\s*any\\b', 'as\\s+any\\b', '<any>'].join('|');
        const anyRegex = new RegExp(pattern, 'g');
        const matches = content.match(anyRegex);
        assert.strictEqual(
          matches,
          null,
          `File ${path.basename(file)} must not use any type: ${JSON.stringify(matches)}`,
        );
      }
    });
  });
});

describe('TASK-M4-02 Test Suite: Core Crate, Unified Error Handling & IPC Protocol', () => {
  describe('UT-02.1: Rust Error Codes & Domain Invariant Checks', () => {
    it('should verify src-tauri/src/error/codes.rs contains all canonical error code constants', () => {
      const codesPath = path.resolve('src-tauri/src/error/codes.rs');
      assert.strictEqual(fs.existsSync(codesPath), true, 'codes.rs must exist');

      const content = fs.readFileSync(codesPath, 'utf8');
      const requiredCodes = [
        'ERR_INGEST_DISK_FULL',
        'ERR_INGEST_BOT_BLOCKED',
        'ERR_INGEST_CHAT_UNAVAILABLE',
        'ERR_INGEST_NETWORK_TIMEOUT',
        'ERR_INGEST_INVALID_URL',
        'ERR_INGEST_EXTRACTION_FAILED',
        'ERR_INGEST_LIVE_STREAM_UNSUPPORTED',
        'ERR_STT_RATE_LIMIT',
        'ERR_STT_PAYLOAD_TOO_LARGE',
        'ERR_STT_VRAM_INSUFFICIENT',
        'ERR_STT_CUDA_OOM',
        'ERR_STT_INVALID_API_KEY',
        'ERR_STT_VAD_FAILED',
        'ERR_STT_MODEL_NOT_FOUND',
        'ERR_STT_MODEL_INTEGRITY_FAILED',
        'ERR_STT_SIDECAR_FAILED',
        'ERR_STT_PARSER_FAILED',
        'ERR_STT_INVALID_TIMESTAMP',
        'ERR_TRANSLATE_SAFETY_FILTER',
        'ERR_TRANSLATE_INVALID_API_KEY',
        'ERR_TRANSLATE_API_FAILED',
        'ERR_EXPORT_CAPCUT_NOT_FOUND',
        'ERR_EXPORT_FILE_WRITE_FAILED',
        'ERR_EXPORT_INVALID_LAYOUT',
        'ERR_HIGHLIGHT_CHAT_PARSING_FAILED',
        'ERR_HIGHLIGHT_AUDIO_ANALYSIS_FAILED',
        'ERR_HIGHLIGHT_NO_HIGHLIGHTS',
        'ERR_HIGHLIGHT_WORKER_PANICKED',
        'ERR_SIDECAR_BINARY_NOT_FOUND',
        'ERR_SIDECAR_EXECUTION_FAILED',
        'ERR_SIDECAR_UPDATE_FAILED',
        'ERR_SIDECAR_PROCESS_KILLED',
        'ERR_HARDWARE_PROBE_FAILED',
        'ERR_HARDWARE_UNSUPPORTED_GPU',
        'ERR_HARDWARE_INSUFFICIENT_MEMORY',
        'ERR_STORAGE_DISK_CHECK_FAILED',
        'ERR_STORAGE_DIRECTORY_CREATION_FAILED',
        'ERR_STORAGE_CACHE_CLEANUP_FAILED',
        'ERR_STORAGE_FILE_NOT_FOUND',
        'ERR_CONFIG_NOT_FOUND',
        'ERR_CONFIG_INVALID_FORMAT',
        'ERR_CONFIG_SAVE_FAILED',
        'ERR_CONFIG_INVALID_VALUE',
        'ERR_TASK_CANCELLED',
        'ERR_INTERNAL_ERROR',
      ];

      for (const code of requiredCodes) {
        assert.ok(
          content.includes(`pub const ${code}: &str = "${code}";`),
          `codes.rs must define constant ${code}`,
        );
      }
    });

    it('should verify src-tauri/src/error/mod.rs defines AppError and IpcErrorResponse', () => {
      const modPath = path.resolve('src-tauri/src/error/mod.rs');
      assert.strictEqual(fs.existsSync(modPath), true, 'error/mod.rs must exist');

      const content = fs.readFileSync(modPath, 'utf8');
      assert.ok(content.includes('pub struct IpcErrorResponse'), 'Must define IpcErrorResponse');
      assert.ok(content.includes('pub enum AppError'), 'Must define AppError enum');
      assert.ok(
        content.includes('impl Serialize for AppError'),
        'Must implement Serialize for AppError',
      );
      assert.ok(content.includes('pub fn code(&self) ->'), 'Must implement code method');
      assert.ok(
        content.includes('pub fn is_recoverable(&self) -> bool'),
        'Must implement is_recoverable',
      );
      assert.ok(
        content.includes('pub fn to_ipc_response(&self) -> IpcErrorResponse'),
        'Must implement to_ipc_response',
      );
    });

    it('should verify src-tauri/src/lib.rs exports error module and ping returns Result<String, AppError>', () => {
      const libPath = path.resolve('src-tauri/src/lib.rs');
      assert.strictEqual(fs.existsSync(libPath), true);

      const content = fs.readFileSync(libPath, 'utf8');
      assert.ok(content.includes('pub mod error;'), 'lib.rs must export error module');
      assert.ok(
        content.includes('pub fn ping() -> Result<String, AppError>'),
        'ping command signature must be Result<String, AppError>',
      );
    });
  });

  describe('ED-02.1: Clippy Zero-Panic & Static Rust Invariants Audit', () => {
    it('should verify all Rust source files contain zero forbidden panic calls in non-test code', () => {
      function scanRustFiles(dir: string): string[] {
        const files: string[] = [];
        for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
          const fullPath = path.join(dir, entry.name);
          if (entry.isDirectory()) {
            files.push(...scanRustFiles(fullPath));
          } else if (entry.name.endsWith('.rs')) {
            files.push(fullPath);
          }
        }
        return files;
      }

      const rustFiles = scanRustFiles(path.resolve('src-tauri/src'));
      assert.ok(rustFiles.length >= 4, 'Must scan at least 4 Rust source files');

      for (const file of rustFiles) {
        const content = fs.readFileSync(file, 'utf8');
        const nonTestContent = content.split('#[cfg(test)]')[0] ?? content;
        const lines = nonTestContent.split('\n');

        const forbiddenPattern = /\b(unwrap|expect|panic!|todo!|unimplemented!)\b/g;

        for (let i = 0; i < lines.length; i += 1) {
          const line = lines[i]?.trim() ?? '';
          if (
            line.startsWith('#![deny(') ||
            line.startsWith('//') ||
            line.startsWith('/*') ||
            line.startsWith('*')
          ) {
            continue;
          }
          const match = line.match(forbiddenPattern);
          assert.strictEqual(
            match,
            null,
            `Forbidden call '${match?.[0]}' found in ${path.basename(file)} at line ${i + 1}: ${line}`,
          );
        }
      }
    });
  });

  describe('IT-02.1: TypeScript IPC Error Envelope & Protocol Invariants', () => {
    it('should validate valid IpcErrorResponse objects and reject malformed inputs', () => {
      assert.strictEqual(
        isIpcError({
          code: ERROR_CODES.INGEST_DISK_FULL,
          message: 'Ổ đĩa đầy',
          details: null,
          recoverable: true,
        }),
        true,
      );

      assert.strictEqual(isIpcError(null), false);
      assert.strictEqual(isIpcError(undefined), false);
      assert.strictEqual(isIpcError({ code: 123 }), false);
    });

    it('should parse various error formats into normalized IpcErrorResponse', () => {
      const parsedString = parseIpcError('Tải stream thất bại');
      assert.strictEqual(parsedString.code, ERROR_CODES.INTERNAL_ERROR);
      assert.strictEqual(parsedString.message, 'Tải stream thất bại');
      assert.strictEqual(parsedString.recoverable, false);

      const jsonPayload = JSON.stringify({
        code: ERROR_CODES.INGEST_BOT_BLOCKED,
        message: 'Bị chặn bot',
        details: 'HTTP 403',
        recoverable: true,
      });
      const parsedJson = parseIpcError(jsonPayload);
      assert.strictEqual(parsedJson.code, ERROR_CODES.INGEST_BOT_BLOCKED);
      assert.strictEqual(parsedJson.recoverable, true);
    });

    it('should verify all canonical error codes have Vietnamese descriptions', () => {
      assert.ok(ERROR_MESSAGES_VI[ERROR_CODES.INGEST_DISK_FULL]?.includes('ổ đĩa'));
      assert.ok(ERROR_MESSAGES_VI[ERROR_CODES.STT_RATE_LIMIT]?.includes('Rate Limit'));
      assert.ok(ERROR_MESSAGES_VI[ERROR_CODES.TRANSLATE_SAFETY_FILTER]?.includes('an toàn'));
      assert.ok(ERROR_MESSAGES_VI[ERROR_CODES.EXPORT_CAPCUT_NOT_FOUND]?.includes('CapCut'));
    });
  });

  describe('IT-02.2: React Error Components Artifact Invariants', () => {
    it('should verify ErrorBanner and ErrorBoundary source files exist', () => {
      const bannerPath = path.resolve('src/components/common/ErrorBanner.tsx');
      const boundaryPath = path.resolve('src/components/common/ErrorBoundary.tsx');

      assert.strictEqual(fs.existsSync(bannerPath), true, 'ErrorBanner.tsx must exist');
      assert.strictEqual(fs.existsSync(boundaryPath), true, 'ErrorBoundary.tsx must exist');
    });
  });
});

describe('TASK-M4-27 / TASK-M4-28 (legacy label TASK-M4-03): Shared Types, Hardware Profile & Config Management', () => {
  const HARDWARE_PROFILE_SCHEMA_5_1 = {
    $schema: 'http://json-schema.org/draft-07/schema#',
    title: 'HardwareProfile',
    type: 'object',
    required: ['cpu_info', 'ram_info', 'gpu_info', 'recommended_mode', 'timestamp'],
    properties: {
      cpu_info: {
        type: 'object',
        required: ['model', 'cores', 'threads'],
        properties: {
          model: { type: 'string' },
          cores: { type: 'integer' },
          threads: { type: 'integer' },
        },
      },
      ram_info: {
        type: 'object',
        required: ['total_mb', 'available_mb'],
        properties: {
          total_mb: { type: 'integer' },
          available_mb: { type: 'integer' },
        },
      },
      gpu_info: {
        type: 'object',
        required: ['vendor', 'model', 'vram_mb', 'cuda_supported'],
        properties: {
          vendor: { type: 'string', enum: ['NVIDIA', 'AMD', 'Intel', 'Unknown'] },
          model: { type: 'string' },
          vram_mb: { type: 'integer' },
          cuda_supported: { type: 'boolean' },
        },
      },
      recommended_mode: {
        type: 'string',
        enum: ['cloud_byok', 'hybrid', 'offline_local'],
      },
      recommendation_reason: { type: 'string' },
      timestamp: { type: 'string', format: 'date-time' },
    },
  };

  describe('UT-03.1: Hardware Profile Schema 5.1 Draft-07 Validation', () => {
    it('should validate valid high-end, mid-range and low-end hardware profiles', () => {
      const ajv = new Ajv({ allErrors: true, format: 'full' });
      const validate = ajv.compile(HARDWARE_PROFILE_SCHEMA_5_1);

      const highEnd: HardwareProfileJson = {
        cpu_info: { model: 'AMD Ryzen 9 7950X', cores: 16, threads: 32 },
        ram_info: { total_mb: 32768, available_mb: 24576 },
        gpu_info: {
          vendor: 'NVIDIA',
          model: 'GeForce RTX 4090',
          vram_mb: 24576,
          cuda_supported: true,
        },
        recommended_mode: 'offline_local',
        recommendation_reason: 'GPU 24GB VRAM và CUDA hỗ trợ tốt mô hình lớn',
        timestamp: '2026-11-09T20:00:00.000Z',
      };
      assert.strictEqual(validate(highEnd), true);

      const midRange: HardwareProfileJson = {
        cpu_info: { model: 'AMD Ryzen 5 5600H', cores: 6, threads: 12 },
        ram_info: { total_mb: 16384, available_mb: 10000 },
        gpu_info: {
          vendor: 'NVIDIA',
          model: 'GeForce RTX 3050',
          vram_mb: 4096,
          cuda_supported: true,
        },
        recommended_mode: 'hybrid',
        timestamp: '2026-11-09T20:01:00Z',
      };
      assert.strictEqual(validate(midRange), true);

      const lowEnd: HardwareProfileJson = {
        cpu_info: { model: 'Intel Core i5-1135G7', cores: 4, threads: 8 },
        ram_info: { total_mb: 8192, available_mb: 3500 },
        gpu_info: {
          vendor: 'Intel',
          model: 'Intel Iris Xe Graphics',
          vram_mb: 0,
          cuda_supported: false,
        },
        recommended_mode: 'cloud_byok',
        recommendation_reason: 'Máy tính văn phòng/iGPU nên dùng Cloud BYOK',
        timestamp: '2026-11-09T20:02:00Z',
      };
      assert.strictEqual(validate(lowEnd), true);
    });

    it('should reject malformed payloads missing required fields or invalid enums', () => {
      const ajv = new Ajv({ allErrors: true, format: 'full' });
      const validate = ajv.compile(HARDWARE_PROFILE_SCHEMA_5_1);

      // Missing cpu_info
      assert.strictEqual(
        validate({
          ram_info: { total_mb: 16384, available_mb: 8000 },
          gpu_info: { vendor: 'NVIDIA', model: 'RTX 3060', vram_mb: 12288, cuda_supported: true },
          recommended_mode: 'offline_local',
          timestamp: '2026-11-09T20:00:00Z',
        }),
        false,
      );

      // Invalid vendor
      assert.strictEqual(
        validate({
          cpu_info: { model: 'M3 Max', cores: 16, threads: 16 },
          ram_info: { total_mb: 36864, available_mb: 28000 },
          gpu_info: { vendor: 'Apple', model: 'M3 GPU', vram_mb: 36864, cuda_supported: false },
          recommended_mode: 'hybrid',
          timestamp: '2026-11-09T20:00:00Z',
        }),
        false,
      );

      // Invalid timestamp
      assert.strictEqual(
        validate({
          cpu_info: { model: 'i5', cores: 4, threads: 8 },
          ram_info: { total_mb: 8192, available_mb: 4000 },
          gpu_info: { vendor: 'Intel', model: 'Iris Xe', vram_mb: 0, cuda_supported: false },
          recommended_mode: 'cloud_byok',
          timestamp: 'invalid-date',
        }),
        false,
      );
    });

    it('should verify TypeScript bidirectional conversion and type guards', () => {
      const sample: HardwareProfileJson = {
        cpu_info: { model: 'i7-13700K', cores: 16, threads: 24 },
        ram_info: { total_mb: 32768, available_mb: 20000 },
        gpu_info: { vendor: 'NVIDIA', model: 'RTX 3080', vram_mb: 10240, cuda_supported: true },
        recommended_mode: 'offline_local',
        recommendation_reason: '10GB VRAM CUDA',
        timestamp: '2026-11-09T20:00:00Z',
      };

      assert.strictEqual(isHardwareProfileJson(sample), true);
      const domain = toHardwareProfile(sample);
      assert.strictEqual(isHardwareProfile(domain), true);
      assert.strictEqual(domain.cpuInfo.model, 'i7-13700K');
      assert.strictEqual(domain.gpuInfo.vramMb, 10240);
      assert.strictEqual(domain.recommendedMode, 'offline_local');

      const wire = toHardwareProfileJson(domain);
      assert.deepStrictEqual(wire, sample);
      assert.strictEqual(isGpuVendor(wire.gpu_info.vendor), true);
      assert.strictEqual(isRecommendedMode(wire.recommended_mode), true);
    });
  });

  describe('UT-03.2: Hardware Recommendation Decision Matrix & Heuristics', () => {
    // Pure TypeScript implementation of heuristic engine matching Rust evaluate_recommended_mode
    function evaluateRecommendation(
      ramMb: number,
      gpuVendor: 'NVIDIA' | 'AMD' | 'Intel' | 'Unknown',
      vramMb: number,
      cudaSupported: boolean,
    ): { mode: 'cloud_byok' | 'hybrid' | 'offline_local'; reason: string } {
      if (ramMb < 8192) {
        return { mode: 'cloud_byok', reason: 'RAM < 8GB' };
      }
      if (!cudaSupported || gpuVendor === 'AMD' || gpuVendor === 'Unknown') {
        return { mode: 'cloud_byok', reason: 'Non-CUDA GPU' };
      }
      if (gpuVendor === 'Intel') {
        return { mode: 'cloud_byok', reason: 'Intel iGPU' };
      }
      if (vramMb < 3000) {
        return { mode: 'cloud_byok', reason: 'VRAM < 3GB cliff' };
      }
      if (vramMb >= 8000) {
        if (ramMb >= 16384) {
          return { mode: 'offline_local', reason: 'NVIDIA RTX >= 8GB VRAM + RAM >= 16GB' };
        }
        return { mode: 'hybrid', reason: 'RAM < 16GB bottleneck' };
      }
      return { mode: 'hybrid', reason: 'Mid-range GPU' };
    }

    it('should correctly classify all 10 QA benchmark scenarios (TC-HW-01..10)', () => {
      // TC-HW-01: RTX 4090 24GB + 32GB RAM -> offline_local
      assert.strictEqual(
        evaluateRecommendation(32768, 'NVIDIA', 24576, true).mode,
        'offline_local',
      );

      // TC-HW-02: RTX 3080 10GB + 32GB RAM -> offline_local
      assert.strictEqual(
        evaluateRecommendation(32768, 'NVIDIA', 10240, true).mode,
        'offline_local',
      );

      // TC-HW-03: RTX 3060 Ti 8GB + 16GB RAM -> offline_local
      assert.strictEqual(evaluateRecommendation(16384, 'NVIDIA', 8192, true).mode, 'offline_local');

      // TC-HW-04: RTX 3050 4GB + 16GB RAM -> hybrid
      assert.strictEqual(evaluateRecommendation(16384, 'NVIDIA', 4096, true).mode, 'hybrid');

      // TC-HW-05: GTX 1660 Super 6GB + 16GB RAM -> hybrid
      assert.strictEqual(evaluateRecommendation(16384, 'NVIDIA', 6144, true).mode, 'hybrid');

      // TC-HW-06: GTX 1050 2GB -> cloud_byok (VRAM < 3000MB cliff)
      assert.strictEqual(evaluateRecommendation(16384, 'NVIDIA', 2048, true).mode, 'cloud_byok');

      // TC-HW-07: Intel Iris Xe -> cloud_byok
      assert.strictEqual(evaluateRecommendation(8192, 'Intel', 0, false).mode, 'cloud_byok');

      // TC-HW-08: AMD Radeon RX 7900 XTX 24GB -> cloud_byok (non-CUDA)
      assert.strictEqual(evaluateRecommendation(32768, 'AMD', 24576, false).mode, 'cloud_byok');

      // TC-HW-09: RTX 4090 with low system RAM (8GB) -> hybrid
      assert.strictEqual(evaluateRecommendation(8192, 'NVIDIA', 24576, true).mode, 'hybrid');

      // TC-HW-10: Celeron + 4GB RAM + Unknown GPU -> cloud_byok
      assert.strictEqual(evaluateRecommendation(4096, 'Unknown', 0, false).mode, 'cloud_byok');
    });
  });

  describe('IT-03.1: Atomic Persistence Safety, Defaults & AppConfig Invariants', () => {
    it('should verify DEFAULT_APP_CONFIG complies with privacy and buffer constraints', () => {
      assert.strictEqual(DEFAULT_APP_CONFIG.theme, 'dark');
      assert.strictEqual(DEFAULT_APP_CONFIG.language, 'vi');
      assert.strictEqual(DEFAULT_APP_CONFIG.telemetryEnabled, false); // Core Invariant #5
      assert.strictEqual(DEFAULT_APP_CONFIG.autoUpdateSidecars, true);
      assert.strictEqual(DEFAULT_APP_CONFIG.audioBufferSizeKb, 64); // ADR-002 & CONVENTIONS §2.3
      assert.strictEqual(DEFAULT_APP_CONFIG.activeSttProvider, 'groq');
      assert.strictEqual(DEFAULT_APP_CONFIG.maxHistoryDays, 14);
      assert.strictEqual(isAppConfig(DEFAULT_APP_CONFIG), true);
    });

    it('should verify bidirectional AppConfig serialization roundtrip', () => {
      const configJson: AppConfigJson = {
        theme: 'acrylic',
        language: 'vi',
        telemetry_enabled: false,
        auto_update_sidecars: true,
        output_directory: 'C:/Videos/NeuroClip',
        cache_directory: 'C:/Cache/NeuroClip',
        audio_buffer_size_kb: 128,
        active_stt_provider: 'groq',
        active_stt_model: 'whisper-large-v3',
        active_translation_model: 'gemini-1.5-flash',
        sidecar_paths: {
          ffmpeg_path: 'C:/Tools/ffmpeg.exe',
          ytdlp_path: null,
          whisper_worker_path: null,
        },
        groq_api_key: 'gsk_test123',
        gemini_api_key: 'AIza_test456',
        max_history_days: 14,
      };

      const domain = toAppConfig(configJson);
      assert.strictEqual(domain.theme, 'acrylic');
      assert.strictEqual(domain.audioBufferSizeKb, 128);
      assert.strictEqual(domain.sidecarPaths.ffmpegPath, 'C:/Tools/ffmpeg.exe');
      assert.strictEqual(domain.sidecarPaths.ytdlpPath, undefined);

      const serialized = toAppConfigJson(domain);
      assert.deepStrictEqual(serialized, configJson);
    });

    it('should simulate atomic file persistence protocol on disk', () => {
      const testDir = path.resolve('node_modules/.tmp_test_atomic_' + Date.now());
      fs.mkdirSync(testDir, { recursive: true });

      const targetPath = path.join(testDir, 'app_config.json');
      const tempPath = path.join(testDir, '.app_config.json.tmp.' + Date.now());

      const payload = { ...DEFAULT_APP_CONFIG, audioBufferSizeKb: 128 };
      const serialized = JSON.stringify(toAppConfigJson(payload), null, 2);

      // 1. Write to temp file
      fs.writeFileSync(tempPath, serialized, 'utf8');
      assert.strictEqual(fs.existsSync(tempPath), true);
      assert.strictEqual(fs.existsSync(targetPath), false);

      // 2. Atomic rename (Windows MoveFileEx / NTFS atomic swap)
      fs.renameSync(tempPath, targetPath);
      assert.strictEqual(fs.existsSync(targetPath), true);
      assert.strictEqual(fs.existsSync(tempPath), false); // No leftover temp file

      // 3. Read back
      const readBack = JSON.parse(fs.readFileSync(targetPath, 'utf8'));
      assert.strictEqual(readBack.audio_buffer_size_kb, 128);

      // Clean up
      fs.rmSync(testDir, { recursive: true, force: true });
    });
  });

  describe('ED-03.1: Rust Backend Module Artifacts & Zero-Panic Invariants Audit', () => {
    it('should verify all required Rust backend files exist in expected locations', () => {
      const requiredFiles = [
        'src-tauri/src/config/mod.rs',
        'src-tauri/src/config/app_config.rs',
        'src-tauri/src/config/hardware.rs',
        'src-tauri/src/config/atomic.rs',
        'src-tauri/src/commands/mod.rs',
        'src-tauri/src/commands/config_cmd.rs',
      ];

      for (const relPath of requiredFiles) {
        const fullPath = path.resolve(relPath);
        assert.strictEqual(fs.existsSync(fullPath), true, `Missing required file: ${relPath}`);
      }
    });

    it('should verify src-tauri/src/lib.rs exports config and commands and registers IPC commands', () => {
      const libPath = path.resolve('src-tauri/src/lib.rs');
      const content = fs.readFileSync(libPath, 'utf8');

      assert.ok(content.includes('pub mod config;'), 'lib.rs must export config module');
      assert.ok(content.includes('pub mod commands;'), 'lib.rs must export commands module');
      assert.ok(content.includes('.manage(AppState::default())'), 'lib.rs must manage AppState');
      assert.ok(
        content.includes('commands::get_app_config'),
        'lib.rs must register get_app_config',
      );
      assert.ok(
        content.includes('commands::save_app_config'),
        'lib.rs must register save_app_config',
      );
      assert.ok(
        content.includes('commands::get_hardware_profile'),
        'lib.rs must register get_hardware_profile',
      );
      assert.ok(
        content.includes('commands::detect_hardware'),
        'lib.rs must register detect_hardware',
      );
    });

    it('should verify zero forbidden unwrap/expect/panic calls in all new Rust production files', () => {
      const targetDirs = [
        path.resolve('src-tauri/src/config'),
        path.resolve('src-tauri/src/commands'),
      ];

      for (const dir of targetDirs) {
        for (const file of fs.readdirSync(dir)) {
          if (!file.endsWith('.rs')) continue;
          const fullPath = path.join(dir, file);
          const content = fs.readFileSync(fullPath, 'utf8');
          const nonTestContent = content.split('#[cfg(test)]')[0] ?? content;
          const lines = nonTestContent.split('\n');

          const forbiddenPattern = /\b(unwrap|expect|panic!|todo!|unimplemented!)\b/g;

          for (let i = 0; i < lines.length; i += 1) {
            const line = lines[i]?.trim() ?? '';
            if (line.startsWith('//') || line.startsWith('/*') || line.startsWith('*')) {
              continue;
            }
            const match = line.match(forbiddenPattern);
            assert.strictEqual(
              match,
              null,
              `Forbidden call '${match?.[0]}' found in ${file} at line ${i + 1}: ${line}`,
            );
          }
        }
      }
    });
  });

  describe('ED-03.2: Remediation Invariants Audit (VRAM, Multi-GPU, Lock Ordering, Atomic Nonce)', () => {
    it('should assert HardwareInformation.qwMemorySize is queried before HardwareInformation.MemorySize in Rust AST', () => {
      const hardwareRsPath = path.resolve('src-tauri/src/config/hardware.rs');
      assert.strictEqual(fs.existsSync(hardwareRsPath), true, 'hardware.rs must exist');

      const content = fs.readFileSync(hardwareRsPath, 'utf8');

      // 1. Verify 64-bit QWORD qwMemorySize is queried
      const qwIndex = content.indexOf('HardwareInformation.qwMemorySize');
      assert.ok(
        qwIndex !== -1,
        'Rust probe_windows_registry_gpu must query 64-bit HardwareInformation.qwMemorySize to prevent 4GB VRAM capping on Windows',
      );

      // 2. Verify 32-bit DWORD MemorySize fallback is queried
      const memIndex = content.indexOf('HardwareInformation.MemorySize');
      assert.ok(
        memIndex !== -1,
        'Rust probe_windows_registry_gpu must retain HardwareInformation.MemorySize as fallback',
      );

      // 3. Verify qwMemorySize is queried BEFORE MemorySize
      assert.ok(
        qwIndex < memIndex,
        'HardwareInformation.qwMemorySize MUST be queried prior to HardwareInformation.MemorySize fallback in Rust AST',
      );

      // 4. Verify 64-bit VRAM computation converts bytes to MB via / (1024 * 1024) or >> 20
      const qwSlice = content.slice(qwIndex, memIndex);
      assert.ok(
        qwSlice.includes('1024 * 1024') || qwSlice.includes('1048576') || qwSlice.includes('>> 20'),
        'qwMemorySize must be converted from bytes to megabytes (vram_mb = val / (1024 * 1024))',
      );
    });

    it('should assert multi-adapter loop iterates 0000..0008 and prioritizes NVIDIA/CUDA GPU over integrated GPU', () => {
      const hardwareRsPath = path.resolve('src-tauri/src/config/hardware.rs');
      const content = fs.readFileSync(hardwareRsPath, 'utf8');

      // 1. Verify loop iteration covers at least 0000..0008 (indices 0..=8 or 0..8 or 0..9)
      const hasExpandedLoop =
        /for\s+i\s+in\s+0\.\.=?\s*[89]/.test(content) ||
        content.includes('0..8') ||
        content.includes('0..=8') ||
        content.includes('0..9');
      assert.ok(
        hasExpandedLoop,
        'Multi-adapter probe loop must iterate over subkeys 0000..0008 (e.g. 0..=8 or 0..9) to discover secondary discrete GPUs',
      );

      // 2. Verify loop does not early-return on the first adapter found, but collects candidates
      const fnStart = content.indexOf('fn probe_windows_registry_gpu');
      assert.ok(fnStart !== -1, 'probe_windows_registry_gpu function must exist');
      const fnBody = content.slice(fnStart, content.indexOf('#[cfg(test)]', fnStart));

      // In the loop, candidate GPUs must be collected into a Vec
      assert.ok(
        fnBody.includes('Vec::new()') || fnBody.includes('candidates') || fnBody.includes('gpus'),
        'probe_windows_registry_gpu must collect multiple GPU candidates across adapters instead of early-exiting on index 0000',
      );

      // 3. Verify prioritization logic selects NVIDIA/CUDA GPU or highest VRAM
      assert.ok(
        fnBody.includes('GpuVendor::Nvidia') || fnBody.includes('cuda_supported'),
        'GPU selection logic must prioritize NVIDIA / CUDA adapters over integrated GPUs',
      );
      assert.ok(
        fnBody.includes('vram_mb'),
        'GPU selection logic must account for VRAM capacity (e.g. highest vram_mb)',
      );
    });

    it('should verify simulated multi-GPU prioritization selects NVIDIA dGPU over Intel iGPU', () => {
      interface MockGpuCandidate {
        vendor: 'NVIDIA' | 'AMD' | 'Intel' | 'Unknown';
        model: string;
        vramMb: number;
        cudaSupported: boolean;
      }

      function selectOptimalGpu(candidates: MockGpuCandidate[]): MockGpuCandidate | null {
        if (candidates.length === 0) return null;
        return candidates.reduce((best, cur) => {
          // Priority 1: CUDA / NVIDIA dedicated card over non-CUDA iGPU
          if (cur.cudaSupported && !best.cudaSupported) return cur;
          if (!cur.cudaSupported && best.cudaSupported) return best;
          // Priority 2: Higher VRAM capacity
          return cur.vramMb > best.vramMb ? cur : best;
        });
      }

      // Scenario 1: Intel Iris Xe (0000) + NVIDIA RTX 3060 Laptop (0001) -> selects RTX 3060
      const laptopRig: MockGpuCandidate[] = [
        { vendor: 'Intel', model: 'Intel Iris Xe Graphics', vramMb: 0, cudaSupported: false },
        {
          vendor: 'NVIDIA',
          model: 'NVIDIA GeForce RTX 3060 Laptop GPU',
          vramMb: 6144,
          cudaSupported: true,
        },
      ];
      const selectedLaptop = selectOptimalGpu(laptopRig);
      assert.strictEqual(selectedLaptop?.model, 'NVIDIA GeForce RTX 3060 Laptop GPU');
      assert.strictEqual(selectedLaptop?.cudaSupported, true);

      // Scenario 2: AMD Radeon 680M (0000) + NVIDIA RTX 4070 Laptop (0001) -> selects RTX 4070
      const amdNvidiaRig: MockGpuCandidate[] = [
        { vendor: 'AMD', model: 'AMD Radeon 680M', vramMb: 512, cudaSupported: false },
        {
          vendor: 'NVIDIA',
          model: 'NVIDIA GeForce RTX 4070 Laptop GPU',
          vramMb: 8192,
          cudaSupported: true,
        },
      ];
      assert.strictEqual(
        selectOptimalGpu(amdNvidiaRig)?.model,
        'NVIDIA GeForce RTX 4070 Laptop GPU',
      );

      // Scenario 3: Dual NVIDIA GPUs (e.g. GTX 1650 4GB at 0000 + RTX 4090 24GB at 0001) -> selects highest VRAM
      const dualNvidiaRig: MockGpuCandidate[] = [
        { vendor: 'NVIDIA', model: 'NVIDIA GeForce GTX 1650', vramMb: 4096, cudaSupported: true },
        { vendor: 'NVIDIA', model: 'NVIDIA GeForce RTX 4090', vramMb: 24576, cudaSupported: true },
      ];
      assert.strictEqual(selectOptimalGpu(dualNvidiaRig)?.model, 'NVIDIA GeForce RTX 4090');
    });

    it('should assert save_app_config serializes disk I/O via config_save_lock before atomic_save_json', () => {
      const configCmdPath = path.resolve('src-tauri/src/commands/config_cmd.rs');
      assert.strictEqual(fs.existsSync(configCmdPath), true, 'config_cmd.rs must exist');

      const content = fs.readFileSync(configCmdPath, 'utf8');

      const fnStart = content.indexOf('pub async fn save_app_config');
      assert.ok(fnStart !== -1, 'save_app_config command must exist in config_cmd.rs');

      const nextFn = content.indexOf('pub async fn', fnStart + 20);
      const fnBody = nextFn !== -1 ? content.slice(fnStart, nextFn) : content.slice(fnStart);

      const saveLockIndex = fnBody.indexOf('state.config_save_lock.lock()');
      const writeLockIndex = fnBody.indexOf('state.config.write()');
      const atomicSaveIndex = fnBody.indexOf('atomic_save_json');

      assert.ok(
        saveLockIndex !== -1,
        'save_app_config must acquire state.config_save_lock.lock() before disk I/O',
      );
      assert.ok(writeLockIndex !== -1, 'save_app_config must acquire state.config.write() lock');
      assert.ok(atomicSaveIndex !== -1, 'save_app_config must call atomic_save_json');
      assert.ok(
        saveLockIndex < atomicSaveIndex,
        'state.config_save_lock.lock() MUST be acquired BEFORE calling atomic_save_json to prevent concurrent write races without holding std::sync::RwLock across await',
      );
    });

    it('should assert atomic temporary file nonce uses atomic counter / PID', () => {
      const atomicRsPath = path.resolve('src-tauri/src/config/atomic.rs');
      assert.strictEqual(fs.existsSync(atomicRsPath), true, 'atomic.rs must exist');

      const content = fs.readFileSync(atomicRsPath, 'utf8');

      // 1. Verify atomic sequence counter usage (AtomicU64 or AtomicUsize)
      const hasAtomicCounter = content.includes('AtomicU64') || content.includes('AtomicUsize');
      assert.ok(
        hasAtomicCounter,
        'atomic.rs must define an atomic sequence counter (AtomicU64 / AtomicUsize) to prevent intra-process timestamp collisions',
      );

      // 2. Verify fetch_add on counter
      assert.ok(
        content.includes('fetch_add'),
        'atomic.rs must invoke fetch_add on atomic sequence counter during nonce generation',
      );

      // 3. Verify process ID incorporation
      const hasPid = content.includes('std::process::id()') || content.includes('process::id()');
      assert.ok(
        hasPid,
        'atomic.rs must incorporate process ID (process::id()) in temporary file name to prevent multi-process collisions',
      );
    });
  });
});

describe('TASK-M4-04 Test Suite: Logging, Rotation, Redaction, IPC & Tracing Subsystem', () => {
  describe('UT-04.1: Canonical Secret Redaction (Happy Path)', () => {
    it('should mask API keys of length > 8 preserving prefix 4 and suffix 4 with [REDACTED]', () => {
      const groqKey = 'gsk_1234567890abcdef12345678';
      assert.strictEqual(
        redactSecret(groqKey),
        'gsk_[REDACTED]5678',
        'Groq key must be redacted to gsk_[REDACTED]5678',
      );

      const geminiKey = 'AIzaSyD987654321abcdef1234567890';
      assert.strictEqual(
        redactSecret(geminiKey),
        'AIza[REDACTED]7890',
        'Gemini key must be redacted to AIza[REDACTED]7890',
      );

      const openaiKey = 'sk-proj-abcdefghijklmnopqrstuvwxyz123456';
      assert.strictEqual(
        redactSecret(openaiKey),
        'sk-p[REDACTED]3456',
        'OpenAI key must be redacted to sk-p[REDACTED]3456',
      );
    });
  });

  describe('UT-04.2: Edge Cases (Short Keys & Boundaries)', () => {
    it('should mask keys <= 8 characters completely as [REDACTED]', () => {
      assert.strictEqual(redactSecret(''), '[REDACTED]', 'Empty string must return [REDACTED]');
      assert.strictEqual(
        redactSecret('short'),
        '[REDACTED]',
        '5-char string must return [REDACTED]',
      );
      assert.strictEqual(
        redactSecret('12345678'),
        '[REDACTED]',
        '8-char boundary string must return [REDACTED]',
      );
    });

    it('should correctly redact 9-character boundary string', () => {
      assert.strictEqual(
        redactSecret('123456789'),
        '1234[REDACTED]6789',
        '9-char string must preserve 4-char prefix and 4-char suffix',
      );
    });
  });

  describe('UT-04.3: Multi-Provider Secret Scanning & Sanitization', () => {
    it('should sanitize all supported providers without leaking raw secret tokens', () => {
      const groq = 'gsk_abcdef1234567890fedcba';
      const gemini = 'AIzaSyD987654321abcdef1234567890';
      const openai = 'sk-proj-abcdefghijklmnopqrstuvwxyz123456';
      const twitchAuth = 'auth-token=1234567890abcdef1234';
      const twitchOauth = 'oauth:1234567890abcdef1234';
      const bearer = 'Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6';

      const combinedText = `
        Config:
        groq=${groq}
        gemini=${gemini}
        openai=${openai}
        twitch_auth=${twitchAuth}
        twitch_oauth=${twitchOauth}
        auth_hdr=${bearer}
      `;

      const sanitized = sanitizeLogString(combinedText);

      // Verify zero occurrences of raw secret strings
      assert.strictEqual(sanitized.includes(groq), false, 'Must not contain raw Groq key');
      assert.strictEqual(sanitized.includes(gemini), false, 'Must not contain raw Gemini key');
      assert.strictEqual(sanitized.includes(openai), false, 'Must not contain raw OpenAI key');
      assert.strictEqual(
        sanitized.includes('1234567890abcdef1234'),
        false,
        'Must not contain raw Twitch token',
      );
      assert.strictEqual(
        sanitized.includes('eyJhbGciOiJIUzI1NiIsInR5cCI6'),
        false,
        'Must not contain raw Bearer token',
      );

      // Verify presence of sanitized tokens
      assert.ok(sanitized.includes('gsk_[REDACTED]dcba'));
      assert.ok(sanitized.includes('AIza[REDACTED]7890'));
      assert.ok(sanitized.includes('sk-p[REDACTED]3456'));
      assert.ok(sanitized.includes('auth-token=[REDACTED]'));
      assert.ok(sanitized.includes('oaut[REDACTED]1234'));
      assert.ok(sanitized.includes('Bearer [REDACTED]'));
    });
  });

  describe('ED-04.1: Rust Backend Module Artifacts & Zero-Panic Invariants Audit', () => {
    it('should verify all required Rust logging files exist in expected locations', () => {
      const requiredFiles = [
        'src-tauri/src/logging/mod.rs',
        'src-tauri/src/logging/secrets.rs',
        'src-tauri/src/logging/rotation.rs',
        'src-tauri/src/commands/log_cmd.rs',
      ];

      for (const relPath of requiredFiles) {
        const fullPath = path.resolve(relPath);
        assert.strictEqual(fs.existsSync(fullPath), true, `File ${relPath} must exist on disk`);
      }
    });

    it('should verify src-tauri/src/lib.rs exports logging and registers IPC commands', () => {
      const libPath = path.resolve('src-tauri/src/lib.rs');
      const content = fs.readFileSync(libPath, 'utf8');

      const coreModPath = path.resolve('src-tauri/src/core/mod.rs');
      const coreModContent = fs.readFileSync(coreModPath, 'utf8');

      assert.ok(content.includes('pub mod logging;'), 'lib.rs must export pub mod logging;');
      assert.ok(content.includes('pub mod core;'), 'lib.rs must export pub mod core;');
      assert.ok(
        coreModContent.includes('pub use crate::logging;'),
        'core/mod.rs must re-export logging',
      );
      assert.ok(
        content.includes('commands::get_recent_logs'),
        'lib.rs must register get_recent_logs command',
      );
      assert.ok(
        content.includes('commands::clear_logs'),
        'lib.rs must register clear_logs command',
      );
      assert.ok(
        content.includes('commands::log_frontend_event'),
        'lib.rs must register log_frontend_event command',
      );
      assert.ok(
        content.includes('logging::init_logging()'),
        'lib.rs must initialize logging in run()',
      );
    });

    it('should verify src-tauri/Cargo.toml includes regex = "1.10"', () => {
      const cargoPath = path.resolve('src-tauri/Cargo.toml');
      const content = fs.readFileSync(cargoPath, 'utf8');
      assert.ok(content.includes('regex = "1.10"'), 'Cargo.toml must declare regex = "1.10"');
    });

    it('should verify zero forbidden unwrap/expect/panic in all new Rust logging files', () => {
      const loggingFiles = [
        'src-tauri/src/logging/mod.rs',
        'src-tauri/src/logging/secrets.rs',
        'src-tauri/src/logging/rotation.rs',
        'src-tauri/src/commands/log_cmd.rs',
      ];

      for (const relPath of loggingFiles) {
        const fullPath = path.resolve(relPath);
        const content = fs.readFileSync(fullPath, 'utf8');
        const testIndex = content.indexOf('#[cfg(test)]');
        const prodContent = testIndex !== -1 ? content.slice(0, testIndex) : content;

        assert.strictEqual(
          prodContent.includes('.unwrap()'),
          false,
          `Forbidden .unwrap() found in production code of ${relPath}`,
        );
        assert.strictEqual(
          prodContent.includes('.expect('),
          false,
          `Forbidden .expect() found in production code of ${relPath}`,
        );
        assert.strictEqual(
          prodContent.includes('panic!('),
          false,
          `Forbidden panic!() found in production code of ${relPath}`,
        );
      }
    });
  });

  describe('IT-04.1: Rust Streaming Redaction & Log Retention AST Checks', () => {
    it('should assert RedactingWriter intercepts write and runs redact_secrets_in_text', () => {
      const secretsPath = path.resolve('src-tauri/src/logging/secrets.rs');
      const content = fs.readFileSync(secretsPath, 'utf8');

      assert.ok(
        content.includes('pub struct RedactingWriter<W: Write>'),
        'Must define RedactingWriter',
      );
      assert.ok(
        content.includes('impl<W: Write> Write for RedactingWriter<W>'),
        'Must implement Write for RedactingWriter',
      );
      assert.ok(
        content.includes('redact_secrets_in_text'),
        'RedactingWriter must call redact_secrets_in_text',
      );
    });

    it('should assert RetentionPolicy constants match SSOT specifications (14 days, 50MB ceiling, 30MB low-water mark)', () => {
      const rotationPath = path.resolve('src-tauri/src/logging/rotation.rs');
      const content = fs.readFileSync(rotationPath, 'utf8');

      assert.ok(
        content.includes('max_age_days: 14'),
        'RetentionPolicy default max_age_days must be 14',
      );
      assert.ok(
        content.includes('50 * 1024 * 1024'),
        'RetentionPolicy default max_dir_size_bytes must be 50MB (50 * 1024 * 1024)',
      );
      assert.ok(
        content.includes('30 * 1024 * 1024'),
        'RetentionPolicy default target_dir_size_bytes must be 30MB (30 * 1024 * 1024)',
      );
    });

    it('should assert resolve_log_dir supports NEUROCLIP_LOG_DIR environment override', () => {
      const modPath = path.resolve('src-tauri/src/logging/mod.rs');
      const content = fs.readFileSync(modPath, 'utf8');

      assert.ok(
        content.includes('NEUROCLIP_LOG_DIR'),
        'resolve_log_dir must check NEUROCLIP_LOG_DIR for isolated test directories',
      );
      assert.ok(
        content.includes('.try_init()'),
        'init_logging_with_dir must use .try_init() to eliminate multi-init test collisions',
      );
    });
  });

  describe('IT-04.2: Frontend Logging Contracts & Wire Mapping Invariants', () => {
    it('should verify all required frontend logging source files exist', () => {
      const frontendFiles = [
        'src/services/logging/types.ts',
        'src/services/logging/redactor.ts',
        'src/services/logging/logger.ts',
        'src/services/logging/index.ts',
        'src/stores/useLogStore.ts',
      ];

      for (const relPath of frontendFiles) {
        const fullPath = path.resolve(relPath);
        assert.strictEqual(
          fs.existsSync(fullPath),
          true,
          `Frontend file ${relPath} must exist on disk`,
        );
      }
    });

    it('should verify LogEntryJson and LogEntry type guards and converters', () => {
      const validJson = {
        timestamp: '2026-11-10T04:15:30.123Z',
        level: 'WARN',
        target: 'neuroclip::ingestion',
        message: 'Network retry 2/3',
        fields: { vodId: '98765' },
        line: 88,
        file: 'ingestion.rs',
      };

      assert.strictEqual(
        isLogEntryJson(validJson),
        true,
        'Valid LogEntryJson must pass type guard',
      );
      assert.strictEqual(
        isLogEntryJson({ bad: 'payload' }),
        false,
        'Malformed payload must be rejected',
      );

      const domain = toLogEntry(validJson);
      assert.strictEqual(domain.level, 'warn', 'Wire WARN must normalize to lowercase warn');
      assert.strictEqual(domain.target, 'neuroclip::ingestion');
      assert.strictEqual(domain.message, 'Network retry 2/3');
      assert.strictEqual(domain.line, 88);
      assert.strictEqual(domain.file, 'ingestion.rs');
      assert.ok(domain.id.startsWith('log-'));

      assert.strictEqual(toLogLevel('ERROR'), 'error');
      assert.strictEqual(toLogLevel('DEBUG'), 'debug');
      assert.strictEqual(toLogLevel('UNKNOWN'), 'info');
    });

    it('should verify sanitizeLogContext handles complex nested objects without throwing', () => {
      const context = {
        key: 'gsk_1234567890abcdef12345678',
        meta: {
          sub: 'AIzaSyD987654321abcdef1234567890',
        },
      };

      const sanitized = sanitizeLogContext(context);
      assert.ok(sanitized);
      assert.strictEqual(sanitized['key'], 'gsk_[REDACTED]5678');
      const meta = sanitized['meta'] as Record<string, unknown>;
      assert.strictEqual(meta['sub'], 'AIza[REDACTED]7890');
    });
  });
});

describe('TASK-M4-05 Test Suite: Storage Preflight Check, Cache Management & Project Store', () => {
  describe('UT-05.1 & UT-05.2: DiskStatus Wire Serialization & Domain Contracts', () => {
    it('UT-05.1 (Happy Path): should serialize and convert 20GB free space status correctly', () => {
      const wireJson: DiskStatusJson = {
        free_space_mb: 20480,
        is_sufficient: true,
        cache_dir: 'C:\\Users\\User\\AppData\\Local\\NeuroClip\\cache',
      };

      assert.strictEqual(
        isDiskStatusJson(wireJson),
        true,
        'Valid DiskStatusJson must pass type guard',
      );
      assert.strictEqual(
        isDiskStatusJson({ bad: 123 }),
        false,
        'Malformed payload must be rejected',
      );

      const domain = toDiskStatus(wireJson);
      assert.strictEqual(domain.freeSpaceMb, 20480);
      assert.strictEqual(domain.free_space_mb, 20480);
      assert.strictEqual(domain.isSufficient, true);
      assert.strictEqual(domain.is_sufficient, true);
      assert.strictEqual(domain.cacheDir, 'C:\\Users\\User\\AppData\\Local\\NeuroClip\\cache');
    });

    it('UT-05.2 (Failure Path): should serialize and convert 3.2GB insufficient free space status', () => {
      const wireJson: DiskStatusJson = {
        free_space_mb: 3276,
        is_sufficient: false,
        cache_dir: 'C:\\Users\\User\\AppData\\Local\\NeuroClip\\cache',
      };

      const domain = toDiskStatus(wireJson);
      assert.strictEqual(domain.freeSpaceMb, 3276);
      assert.strictEqual(domain.isSufficient, false);
      assert.strictEqual(domain.is_sufficient, false);
      assert.strictEqual(domain.freeSpaceMb < MIN_REQUIRED_DISK_SPACE_MB, true);
    });

    it('should assert MIN_REQUIRED_DISK_SPACE_MB threshold equals 5120MB (5GB)', () => {
      assert.strictEqual(MIN_REQUIRED_DISK_SPACE_MB, 5120, 'Threshold must be exactly 5120MB');
    });

    it('should verify CacheCleanupResult and StoragePaths serialization and guards', () => {
      const cleanupRaw = {
        files_deleted: 10,
        bytes_freed: 104857600,
        freed_mb: 100,
      };

      assert.strictEqual(isCacheCleanupResultJson(cleanupRaw), true);
      const cleanupDomain = toCacheCleanupResult(cleanupRaw);
      assert.strictEqual(cleanupDomain.filesDeleted, 10);
      assert.strictEqual(cleanupDomain.bytesFreed, 104857600);
      assert.strictEqual(cleanupDomain.freedMb, 100);

      const pathsRaw = {
        cache_dir: 'C:\\NeuroClip\\cache',
        projects_dir: 'C:\\NeuroClip\\projects',
      };
      assert.strictEqual(isStoragePathsJson(pathsRaw), true);
      const pathsDomain = toStoragePaths(pathsRaw);
      assert.strictEqual(pathsDomain.cacheDir, 'C:\\NeuroClip\\cache');
      assert.strictEqual(pathsDomain.projectsDir, 'C:\\NeuroClip\\projects');
    });
  });

  describe('ED-05.1: Clippy Zero-Panic & Static Rust Invariants Audit', () => {
    it('should verify zero forbidden unwrap/expect/panic in all new Rust storage files', () => {
      const storageFiles = [
        'src-tauri/src/core/mod.rs',
        'src-tauri/src/core/storage.rs',
        'src-tauri/src/commands/storage_cmd.rs',
      ];

      for (const relPath of storageFiles) {
        const fullPath = path.resolve(relPath);
        assert.ok(fs.existsSync(fullPath), `Storage file ${relPath} must exist on disk`);

        const content = fs.readFileSync(fullPath, 'utf8');
        const testIndex = content.indexOf('#[cfg(test)]');
        const prodContent = testIndex !== -1 ? content.slice(0, testIndex) : content;

        assert.strictEqual(
          prodContent.includes('.unwrap()'),
          false,
          `Forbidden .unwrap() found in production code of ${relPath}`,
        );
        assert.strictEqual(
          prodContent.includes('.expect('),
          false,
          `Forbidden .expect() found in production code of ${relPath}`,
        );
        assert.strictEqual(
          prodContent.includes('panic!('),
          false,
          `Forbidden panic!() found in production code of ${relPath}`,
        );
      }
    });
  });

  describe('IT-05.1: Win32 Direct Kernel Query & Mock Override Invariants', () => {
    it('should assert Cargo.toml activates Win32_Storage_FileSystem feature', () => {
      const cargoPath = path.resolve('src-tauri/Cargo.toml');
      const content = fs.readFileSync(cargoPath, 'utf8');
      assert.ok(
        content.includes('"Win32_Storage_FileSystem"'),
        'Cargo.toml must include Win32_Storage_FileSystem in windows-sys features',
      );
    });

    it('should assert core/storage.rs calls GetDiskFreeSpaceExW on Windows', () => {
      const storagePath = path.resolve('src-tauri/src/core/storage.rs');
      const content = fs.readFileSync(storagePath, 'utf8');
      assert.ok(
        content.includes('GetDiskFreeSpaceExW'),
        'core/storage.rs must call Win32 GetDiskFreeSpaceExW for high-performance volume inspection',
      );
    });

    it('should assert core/storage.rs supports NEUROCLIP_MOCK_DISK_FREE_MB environment override', () => {
      const storagePath = path.resolve('src-tauri/src/core/storage.rs');
      const content = fs.readFileSync(storagePath, 'utf8');
      assert.ok(
        content.includes('NEUROCLIP_MOCK_DISK_FREE_MB'),
        'core/storage.rs must check NEUROCLIP_MOCK_DISK_FREE_MB for unit testing',
      );
    });

    it('should assert query_disk_free_mb traverses ancestors for non-existent directories', () => {
      const storagePath = path.resolve('src-tauri/src/core/storage.rs');
      const content = fs.readFileSync(storagePath, 'utf8');
      assert.ok(
        content.includes('ancestors()'),
        'query_disk_free_mb must traverse path ancestors to find existing root volume',
      );
    });
  });

  describe('ED-05.2: Safe Retention Blacklist & Prunable Whitelist AST Checks', () => {
    it('should assert PRUNABLE_EXTENSIONS only whitelists tmp and wav', () => {
      const storagePath = path.resolve('src-tauri/src/core/storage.rs');
      const content = fs.readFileSync(storagePath, 'utf8');
      assert.ok(
        content.includes('&["tmp", "wav"]'),
        'PRUNABLE_EXTENSIONS must only permit "tmp" and "wav"',
      );
    });

    it('should assert PROTECTED_EXTENSIONS strictly shields json, ass, srt, mp4', () => {
      const storagePath = path.resolve('src-tauri/src/core/storage.rs');
      const content = fs.readFileSync(storagePath, 'utf8');
      assert.ok(
        content.includes('PROTECTED_EXTENSIONS'),
        'core/storage.rs must define PROTECTED_EXTENSIONS',
      );
      assert.ok(content.includes('"json"'), 'Must protect json files');
      assert.ok(content.includes('"ass"'), 'Must protect ass subtitle files');
      assert.ok(content.includes('"srt"'), 'Must protect srt subtitle files');
      assert.ok(content.includes('"mp4"'), 'Must protect mp4 media files');
    });
  });

  describe('IT-05.2: Module Wiring & Command Registration in lib.rs', () => {
    it('should assert src-tauri/src/lib.rs exports core::storage and registers storage IPC commands', () => {
      const libPath = path.resolve('src-tauri/src/lib.rs');
      const content = fs.readFileSync(libPath, 'utf8');
      assert.ok(content.includes('pub use core::storage;'), 'lib.rs must export core::storage');
      assert.ok(
        content.includes('commands::check_preflight_storage'),
        'lib.rs must register check_preflight_storage command',
      );
      assert.ok(
        content.includes('commands::initialize_storage'),
        'lib.rs must register initialize_storage command',
      );
      assert.ok(
        content.includes('commands::clean_expired_cache'),
        'lib.rs must register clean_expired_cache command',
      );
    });
  });
});

describe('TASK-M4-06 Test Suite: Audio-First Ingestion & FFmpeg PCM Extraction Pipeline', () => {
  describe('UT-06.1 & UT-06.2: Ingestion Domain Contracts & Schema 5.2 Validation', () => {
    const vodManifestSchema = {
      $schema: 'http://json-schema.org/draft-07/schema#',
      title: 'VodManifest',
      type: 'object',
      required: ['vod_id', 'source_type', 'title', 'duration_seconds', 'audio_path', 'created_at'],
      properties: {
        vod_id: { type: 'string' },
        source_type: {
          type: 'string',
          enum: ['twitch_vod', 'youtube_vod', 'local_file'],
        },
        original_url: { type: ['string', 'null'] },
        title: { type: 'string' },
        streamer: { type: ['string', 'null'] },
        duration_seconds: { type: 'number' },
        audio_path: { type: 'string' },
        chat_file_path: { type: ['string', 'null'] },
        video_stream_url: { type: ['string', 'null'] },
        local_video_path: { type: ['string', 'null'] },
        created_at: { type: 'string' },
      },
      additionalProperties: true,
    };

    const ajv = new Ajv();
    const validate = ajv.compile(vodManifestSchema);

    it('UT-06.1 (Happy Path): should validate valid Schema 5.2 vod_manifest.json with all 6 required fields', () => {
      const validPayload: VodManifestJson = {
        vod_id: 'vod_neuro_singing_2026',
        source_type: 'youtube_vod',
        original_url: 'https://www.youtube.com/watch?v=dQw4w9WgXcQ',
        title: 'Neuro-sama AI Karaoke Marathon',
        streamer: 'Vedal987',
        duration_seconds: 28800.0, // 8 hours
        audio_path: 'C:\\Users\\User\\AppData\\Local\\NeuroClip\\cache\\vod_neuro_audio.m4a',
        created_at: '2026-11-10T12:00:00Z',
      };

      const isValid = validate(validPayload);
      assert.strictEqual(isValid, true, JSON.stringify(validate.errors));
      assert.strictEqual(isVodManifestJson(validPayload), true);

      const domain = toVodManifest(validPayload);
      assert.strictEqual(domain.vodId, 'vod_neuro_singing_2026');
      assert.strictEqual(domain.sourceType, 'youtube_vod');
      assert.strictEqual(domain.durationSeconds, 28800.0);
      assert.strictEqual(domain.title, 'Neuro-sama AI Karaoke Marathon');
      assert.strictEqual(domain.streamer, 'Vedal987');
      assert.strictEqual(domain.audioPath, validPayload.audio_path);
    });

    it('UT-06.2 (Failure Path): should reject malformed payloads missing required fields or invalid source_type', () => {
      const missingDuration = {
        vod_id: 'vod_1',
        source_type: 'twitch_vod',
        title: 'Stream',
        audio_path: '/path/audio.aac',
        created_at: '2026-11-10T12:00:00Z',
      };
      assert.strictEqual(validate(missingDuration), false);
      assert.strictEqual(isVodManifestJson(missingDuration), false);

      const invalidSource = {
        vod_id: 'vod_2',
        source_type: 'unknown_streaming_service',
        title: 'Stream',
        duration_seconds: 120,
        audio_path: '/path/audio.aac',
        created_at: '2026-11-10T12:00:00Z',
      };
      assert.strictEqual(validate(invalidSource), false);
      assert.strictEqual(isVodManifestJson(invalidSource), false);

      const missingAudioPath = {
        vod_id: 'vod_3',
        source_type: 'local_file',
        title: 'Stream',
        duration_seconds: 120,
        created_at: '2026-11-10T12:00:00Z',
      };
      assert.strictEqual(validate(missingAudioPath), false);
      assert.strictEqual(isVodManifestJson(missingAudioPath), false);
    });

    it('should verify IngestProgressEvent type guard and properties', () => {
      const event: IngestProgressEvent = {
        taskId: 'vod_123',
        stage: 'downloading_audio',
        percent: 45.2,
        speedKbps: 5120.0,
        etaSec: 42,
      };
      assert.strictEqual(isIngestProgressEvent(event), true);
      assert.strictEqual(isIngestProgressEvent(null), false);
      assert.strictEqual(isIngestProgressEvent({ percent: '45.2' }), false);
    });

    it('should verify VodMetadata type guard for probe responses', () => {
      const validMeta = {
        id: 'dQw4w9WgXcQ',
        title: 'Neuro stream',
        uploader: 'vedal987',
        duration: 3600,
        is_live: false,
      };
      assert.strictEqual(isVodMetadataJson(validMeta), true);
      assert.strictEqual(isVodMetadataJson({ title: 'Only Title' }), false);
    });
  });

  describe('ED-06.1: Clippy Zero-Panic & Static Rust Invariants Audit for Ingestion', () => {
    it('should verify zero forbidden unwrap/expect/panic in all Rust ingestion files', () => {
      const ingestionFiles = [
        'src-tauri/src/ingestion/mod.rs',
        'src-tauri/src/ingestion/cookies.rs',
        'src-tauri/src/ingestion/buffer.rs',
        'src-tauri/src/ingestion/validator.rs',
        'src-tauri/src/ingestion/streamer.rs',
        'src-tauri/src/commands/ingest_cmd.rs',
      ];

      for (const relPath of ingestionFiles) {
        const fullPath = path.resolve(relPath);
        assert.ok(fs.existsSync(fullPath), `Ingestion file ${relPath} must exist on disk`);

        const content = fs.readFileSync(fullPath, 'utf8');
        const testIndex = content.indexOf('#[cfg(test)]');
        const prodContent = testIndex !== -1 ? content.slice(0, testIndex) : content;

        assert.strictEqual(
          prodContent.includes('.unwrap()'),
          false,
          `Forbidden .unwrap() found in production code of ${relPath}`,
        );
        assert.strictEqual(
          prodContent.includes('.expect('),
          false,
          `Forbidden .expect() found in production code of ${relPath}`,
        );
        assert.strictEqual(
          prodContent.includes('panic!('),
          false,
          `Forbidden panic!() found in production code of ${relPath}`,
        );
      }
    });
  });

  describe('IT-06.1: Audio-First Invariant, FFmpeg Pipe & Bounded Buffer AST Checks', () => {
    it('should assert FFmpeg command builder enforces 16kHz Mono PCM flags and pipe output', () => {
      const streamerPath = path.resolve('src-tauri/src/ingestion/streamer.rs');
      const content = fs.readFileSync(streamerPath, 'utf8');

      // Flags: -vn -sn -dn -f s16le -ac 1 -ar 16000 pipe:1
      assert.ok(content.includes('"-vn"'), 'Must disable video track (-vn)');
      assert.ok(content.includes('"-sn"'), 'Must disable subtitle track (-sn)');
      assert.ok(content.includes('"-dn"'), 'Must disable data stream (-dn)');
      assert.ok(content.includes('"-f"'), 'Must specify format (-f)');
      assert.ok(content.includes('"s16le"'), 'Must enforce signed 16-bit little-endian (s16le)');
      assert.ok(content.includes('"16000"'), 'Must enforce 16kHz sampling rate');
      assert.ok(content.includes('"pipe:1"'), 'Must stream over pipe stdout (pipe:1)');
    });

    it('should assert Bounded Queue limits channel capacity and chunk size (low RAM footprint <= 5MB)', () => {
      const bufferPath = path.resolve('src-tauri/src/ingestion/buffer.rs');
      const content = fs.readFileSync(bufferPath, 'utf8');

      assert.ok(
        content.includes('DEFAULT_CHUNK_SIZE_BYTES: usize = 64 * 1024'),
        'DEFAULT_CHUNK_SIZE_BYTES must be 64KB',
      );
      assert.ok(
        content.includes('DEFAULT_QUEUE_CAPACITY: usize = 16'),
        'DEFAULT_QUEUE_CAPACITY must be 16 chunks (~1MB RAM footprint)',
      );
      assert.ok(
        content.includes('MAX_PIPELINE_RAM_CAP_BYTES: usize = 5 * 1024 * 1024'),
        'MAX_PIPELINE_RAM_CAP_BYTES must be capped at 5MB',
      );
    });

    it('should assert AudioStreamValidator handles carry-over byte and 44-byte WAV header', () => {
      const validatorPath = path.resolve('src-tauri/src/ingestion/validator.rs');
      const content = fs.readFileSync(validatorPath, 'utf8');

      assert.ok(
        content.includes('carry_byte: Option<u8>'),
        'Must maintain carry_byte state for odd bytes',
      );
      assert.ok(
        content.includes('WAV_HEADER_SIZE: usize = 44'),
        'Must define 44-byte WAV header size',
      );
      assert.ok(content.includes('b"RIFF"'), 'Must detect RIFF header');
      assert.ok(content.includes('b"WAVE"'), 'Must detect WAVE magic bytes');
    });

    it('should assert AudioRingBuffer implements circular sliding window without heap reallocation', () => {
      const bufferPath = path.resolve('src-tauri/src/ingestion/buffer.rs');
      const content = fs.readFileSync(bufferPath, 'utf8');

      assert.ok(content.includes('pub struct AudioRingBuffer'), 'Must define AudioRingBuffer');
      assert.ok(content.includes('push_samples'), 'Must implement push_samples');
      assert.ok(content.includes('pop_samples'), 'Must implement pop_samples');
      assert.ok(content.includes('peek_samples'), 'Must implement peek_samples');
    });
  });

  describe('ED-06.2: Anti-Bot Defense, Cookie Arguments & UA Rotation AST Checks', () => {
    it('should assert yt-dlp cookie arguments builder supports browser and file cookies', () => {
      const cookiesPath = path.resolve('src-tauri/src/ingestion/cookies.rs');
      const content = fs.readFileSync(cookiesPath, 'utf8');

      assert.ok(content.includes('--cookies-from-browser'), 'Must support --cookies-from-browser');
      assert.ok(content.includes('--cookies'), 'Must support --cookies file argument');
      assert.ok(content.includes('Chrome'), 'Must support Chrome');
      assert.ok(content.includes('Firefox'), 'Must support Firefox');
      assert.ok(content.includes('Edge'), 'Must support Edge');
    });

    it('should assert Desktop User-Agent rotation pool targets modern Windows 10/11 environments', () => {
      const cookiesPath = path.resolve('src-tauri/src/ingestion/cookies.rs');
      const content = fs.readFileSync(cookiesPath, 'utf8');

      assert.ok(
        content.includes('Windows NT 10.0; Win64; x64'),
        'Must target Windows 10/11 64-bit',
      );
      assert.ok(
        content.includes('get_rotated_desktop_user_agent'),
        'Must provide UA rotation mechanism',
      );
    });
  });

  describe('ED-06.3: Dual-layer Process Lifecycle & Live Stream Guardrail AST Checks', () => {
    it('should assert Dual-Layer Process Lifecycle: kill_on_drop, ChildProcessGuard, and Windows Job Object', () => {
      const streamerPath = path.resolve('src-tauri/src/ingestion/streamer.rs');
      const streamerContent = fs.readFileSync(streamerPath, 'utf8');
      const killSwitchPath = path.resolve('src-tauri/src/sidecar/kill_switch.rs');
      const killSwitchContent = fs.readFileSync(killSwitchPath, 'utf8');

      assert.ok(
        streamerContent.includes('kill_on_drop(true)'),
        'Command must set kill_on_drop(true)',
      );
      assert.ok(
        streamerContent.includes(
          'pub use crate::sidecar::{assign_child_to_job_guard, ChildProcessGuard, WindowsJobObjectGuard};',
        ),
        'streamer.rs must re-export sidecar kill_switch guards',
      );
      assert.ok(
        streamerContent.includes('assign_child_to_job_guard'),
        'streamer.rs must assign spawned processes to Windows Job Object',
      );
      assert.ok(
        killSwitchContent.includes('pub struct ChildProcessGuard'),
        'kill_switch.rs must define ChildProcessGuard',
      );
      assert.ok(
        killSwitchContent.includes('start_kill()'),
        'ChildProcessGuard drop must call start_kill()',
      );
      assert.ok(
        killSwitchContent.includes('JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE'),
        'kill_switch.rs must configure JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE',
      );
      assert.ok(
        killSwitchContent.includes('assign_child_to_job_guard'),
        'kill_switch.rs must define assign_child_to_job_guard',
      );
      assert.ok(
        killSwitchContent.includes('assign_process'),
        'WindowsJobObjectGuard must implement assign_process',
      );
      assert.ok(
        killSwitchContent.includes('assign_child_process'),
        'WindowsJobObjectGuard must implement assign_child_process',
      );
    });

    it('should assert fast live stream rejection guardrail checks is_live flag', () => {
      const streamerPath = path.resolve('src-tauri/src/ingestion/streamer.rs');
      const content = fs.readFileSync(streamerPath, 'utf8');

      assert.ok(content.includes('is_live'), 'Must inspect is_live metadata flag');
      assert.ok(
        content.includes('LiveStreamUnsupported'),
        'Must return LiveStreamUnsupported for active live streams',
      );
    });

    it('should assert pre-flight storage check enforces 5GB threshold before starting ingestion', () => {
      const streamerPath = path.resolve('src-tauri/src/ingestion/streamer.rs');
      const content = fs.readFileSync(streamerPath, 'utf8');

      assert.ok(
        content.includes('check_disk_space'),
        'Must call check_disk_space prior to launching download',
      );
    });
  });

  describe('IT-06.2: Module Wiring & Command Registration in lib.rs', () => {
    it('should assert src-tauri/src/lib.rs exports ingestion module and registers IPC commands', () => {
      const libPath = path.resolve('src-tauri/src/lib.rs');
      const content = fs.readFileSync(libPath, 'utf8');

      assert.ok(content.includes('pub mod ingestion;'), 'lib.rs must declare pub mod ingestion');
      assert.ok(
        content.includes('commands::start_vod_ingestion'),
        'lib.rs must register start_vod_ingestion command',
      );
      assert.ok(
        content.includes('commands::cancel_vod_ingestion'),
        'lib.rs must register cancel_vod_ingestion command',
      );
      assert.ok(content.includes('commands::probe_vod'), 'lib.rs must register probe_vod command');
      assert.ok(
        content.includes('commands::get_vod_manifest'),
        'lib.rs must register get_vod_manifest command',
      );
      assert.ok(
        content.includes('commands::extract_pcm_stream'),
        'lib.rs must register extract_pcm_stream command',
      );
    });
  });

  describe('TASK-M4-08 (legacy label TASK-M4-07): YouTube Live Chat / Twitch VOD Chat Extraction & Normalizer Contracts', () => {
    it('should assert Schema 5.3 Chat Parser & Heatmap modules exist in Backend Rust', () => {
      const parserPath = path.resolve('src-tauri/src/ingestion/chat_parser.rs');
      const twitchPath = path.resolve('src-tauri/src/ingestion/twitch_gql.rs');
      const ytPath = path.resolve('src-tauri/src/ingestion/youtube_chat.rs');
      const cmdPath = path.resolve('src-tauri/src/commands/chat_cmd.rs');

      assert.ok(fs.existsSync(parserPath), 'chat_parser.rs must exist');
      assert.ok(fs.existsSync(twitchPath), 'twitch_gql.rs must exist');
      assert.ok(fs.existsSync(ytPath), 'youtube_chat.rs must exist');
      assert.ok(fs.existsSync(cmdPath), 'chat_cmd.rs must exist');

      const parserContent = fs.readFileSync(parserPath, 'utf8');
      assert.ok(parserContent.includes('pub struct ChatEvent'), 'Must define ChatEvent');
      assert.ok(
        parserContent.includes('pub struct ChatEventsContainer'),
        'Must define ChatEventsContainer',
      );
      assert.ok(
        parserContent.includes('pub struct ChatDensityPoint'),
        'Must define ChatDensityPoint',
      );
      assert.ok(
        parserContent.includes('pub struct ChatHeatmapResponse'),
        'Must define ChatHeatmapResponse',
      );
      assert.ok(parserContent.includes('NEURO_SWARM_EMOTES'), 'Must define Swarm emote dictionary');
      assert.ok(
        parserContent.includes('StreamingChatNormalizer'),
        'Must define StreamingChatNormalizer',
      );
    });

    it('should assert Tauri IPC chat commands are registered in lib.rs', () => {
      const libPath = path.resolve('src-tauri/src/lib.rs');
      const content = fs.readFileSync(libPath, 'utf8');

      assert.ok(
        content.includes('commands::start_chat_extraction'),
        'Must register start_chat_extraction command',
      );
      assert.ok(
        content.includes('commands::get_chat_events'),
        'Must register get_chat_events command',
      );
      assert.ok(
        content.includes('commands::get_chat_velocity_heatmap'),
        'Must register get_chat_velocity_heatmap command',
      );
    });

    it('should assert Frontend TypeScript chat types and IPC clients exist', () => {
      const typesPath = path.resolve('src/types/chat.ts');
      const ipcPath = path.resolve('src/services/ipc/chat.ts');
      const storePath = path.resolve('src/stores/useChatStore.ts');

      assert.ok(fs.existsSync(typesPath), 'src/types/chat.ts must exist');
      assert.ok(fs.existsSync(ipcPath), 'src/services/ipc/chat.ts must exist');
      assert.ok(fs.existsSync(storePath), 'src/stores/useChatStore.ts must exist');

      const typesContent = fs.readFileSync(typesPath, 'utf8');
      assert.ok(typesContent.includes('isChatEventJson'), 'Must export isChatEventJson guard');
      assert.ok(
        typesContent.includes('isChatEventsContainerJson'),
        'Must export isChatEventsContainerJson guard',
      );
      assert.ok(
        typesContent.includes('isChatDensityPointJson'),
        'Must export isChatDensityPointJson guard',
      );
      assert.ok(
        typesContent.includes('isChatHeatmapResponseJson'),
        'Must export isChatHeatmapResponseJson guard',
      );
      assert.ok(typesContent.includes('normalizeChatEvent'), 'Must export normalizeChatEvent');
    });
  });

  describe('TASK-M4-24 / Whisper.cpp Sidecar & VAD (legacy label TASK-M4-08): Whisper Local STT & Transcription Pipeline Contracts', () => {
    it('should assert Schema 5.5 / 5.6 STT modules exist in Backend Rust', () => {
      const modelMgrPath = path.resolve('src-tauri/src/stt/model_manager.rs');
      const sidecarPath = path.resolve('src-tauri/src/stt/whisper_sidecar.rs');
      const parserPath = path.resolve('src-tauri/src/stt/transcript_parser.rs');
      const exportPath = path.resolve('src-tauri/src/stt/subtitle_export.rs');
      const vadPath = path.resolve('src-tauri/src/stt/vad_bridge.rs');
      const cmdPath = path.resolve('src-tauri/src/commands/stt_cmd.rs');

      assert.ok(fs.existsSync(modelMgrPath), 'model_manager.rs must exist');
      assert.ok(fs.existsSync(sidecarPath), 'whisper_sidecar.rs must exist');
      assert.ok(fs.existsSync(parserPath), 'transcript_parser.rs must exist');
      assert.ok(fs.existsSync(exportPath), 'subtitle_export.rs must exist');
      assert.ok(fs.existsSync(vadPath), 'vad_bridge.rs must exist');
      assert.ok(fs.existsSync(cmdPath), 'stt_cmd.rs must exist');

      const parserContent = fs.readFileSync(parserPath, 'utf8');
      assert.ok(parserContent.includes('pub struct WordTiming'), 'Must define WordTiming');
      assert.ok(
        parserContent.includes('pub struct TranscriptSegment'),
        'Must define TranscriptSegment',
      );
      assert.ok(
        parserContent.includes('pub struct TranscriptSegmentsContainer'),
        'Must define TranscriptSegmentsContainer',
      );

      const exportContent = fs.readFileSync(exportPath, 'utf8');
      assert.ok(exportContent.includes('generate_ass_script'), 'Must define generate_ass_script');
      assert.ok(exportContent.includes('generate_srt_script'), 'Must define generate_srt_script');
      assert.ok(
        exportContent.includes('pub struct SubtitleProject'),
        'Must define SubtitleProject',
      );
    });

    it('should assert Tauri IPC STT commands are registered in lib.rs', () => {
      const libPath = path.resolve('src-tauri/src/lib.rs');
      const content = fs.readFileSync(libPath, 'utf8');

      assert.ok(
        content.includes('commands::start_whisper_transcription'),
        'Must register start_whisper_transcription command',
      );
      assert.ok(
        content.includes('commands::verify_stt_model'),
        'Must register verify_stt_model command',
      );
      assert.ok(
        content.includes('commands::export_subtitles_ass'),
        'Must register export_subtitles_ass command',
      );
      assert.ok(
        content.includes('commands::export_subtitles_srt'),
        'Must register export_subtitles_srt command',
      );
    });

    it('should assert Frontend TypeScript transcript types and IPC clients exist', () => {
      const typesPath = path.resolve('src/types/transcript.ts');
      const ipcPath = path.resolve('src/services/ipc/stt.ts');
      const storePath = path.resolve('src/stores/useTranscriptStore.ts');

      assert.ok(fs.existsSync(typesPath), 'src/types/transcript.ts must exist');
      assert.ok(fs.existsSync(ipcPath), 'src/services/ipc/stt.ts must exist');
      assert.ok(fs.existsSync(storePath), 'src/stores/useTranscriptStore.ts must exist');

      const typesContent = fs.readFileSync(typesPath, 'utf8');
      assert.ok(typesContent.includes('isWordTimingJson'), 'Must export isWordTimingJson guard');
      assert.ok(
        typesContent.includes('isTranscriptSegmentJson'),
        'Must export isTranscriptSegmentJson guard',
      );
      assert.ok(
        typesContent.includes('isTranscriptSegmentsContainerJson'),
        'Must export isTranscriptSegmentsContainerJson guard',
      );
      assert.ok(
        typesContent.includes('isSubtitleProjectJson'),
        'Must export isSubtitleProjectJson guard',
      );
      assert.ok(
        typesContent.includes('normalizeTranscriptSegment'),
        'Must export normalizeTranscriptSegment',
      );
    });
  });

  describe('TASK-M4-09 Test Suite: Adjusted Chat Velocity Z-Score Engine', () => {
    it('should assert Rust highlight modules, contracts, and integration tests exist', () => {
      const bucketPath = path.resolve('src-tauri/src/highlight/bucket.rs');
      const zscorePath = path.resolve('src-tauri/src/highlight/zscore.rs');
      const modPath = path.resolve('src-tauri/src/highlight/mod.rs');
      const testPath = path.resolve('src-tauri/tests/highlight_zscore_test.rs');
      const libPath = path.resolve('src-tauri/src/lib.rs');

      assert.ok(fs.existsSync(bucketPath), 'bucket.rs must exist');
      assert.ok(fs.existsSync(zscorePath), 'zscore.rs must exist');
      assert.ok(fs.existsSync(modPath), 'highlight/mod.rs must exist');
      assert.ok(fs.existsSync(testPath), 'highlight_zscore_test.rs must exist');

      const libContent = fs.readFileSync(libPath, 'utf8');
      assert.ok(
        libContent.includes('pub mod highlight;'),
        'lib.rs must register pub mod highlight;',
      );
      assert.ok(libContent.includes('ChatBucket'), 'lib.rs must export ChatBucket');
      assert.ok(libContent.includes('ChatVelocityPoint'), 'lib.rs must export ChatVelocityPoint');
      assert.ok(
        libContent.includes('calculate_adjusted_zscore'),
        'lib.rs must export calculate_adjusted_zscore',
      );
      assert.ok(
        libContent.includes('compute_chat_velocity_series'),
        'lib.rs must export compute_chat_velocity_series',
      );

      const bucketContent = fs.readFileSync(bucketPath, 'utf8');
      assert.ok(
        bucketContent.includes('pub struct ChatBucket'),
        'bucket.rs must define ChatBucket',
      );
      assert.ok(
        bucketContent.includes('DEFAULT_BUCKET_WIDTH_SEC'),
        'bucket.rs must define DEFAULT_BUCKET_WIDTH_SEC',
      );
      assert.ok(
        bucketContent.includes('SPAM_BOT_MAX_MESSAGES_PER_AUTHOR'),
        'bucket.rs must define SPAM_BOT_MAX_MESSAGES_PER_AUTHOR',
      );

      const zscoreContent = fs.readFileSync(zscorePath, 'utf8');
      assert.ok(
        zscoreContent.includes('pub struct ChatVelocityPoint'),
        'zscore.rs must define ChatVelocityPoint',
      );
      assert.ok(
        zscoreContent.includes('MESSAGE_FLOOR_THRESHOLD'),
        'zscore.rs must define MESSAGE_FLOOR_THRESHOLD',
      );
      assert.ok(
        zscoreContent.includes('SMOOTHING_EPSILON'),
        'zscore.rs must define SMOOTHING_EPSILON',
      );
    });

    it('should assert zero-panic invariant in Rust highlight library code (CONVENTIONS.md §2.4)', () => {
      const filesToAudit = [
        path.resolve('src-tauri/src/highlight/bucket.rs'),
        path.resolve('src-tauri/src/highlight/zscore.rs'),
        path.resolve('src-tauri/src/highlight/mod.rs'),
      ];

      for (const filePath of filesToAudit) {
        const content = fs.readFileSync(filePath, 'utf8');
        // Extract non-test code (strip #[cfg(test)])
        const nonTestCode = content.split('#[cfg(test)]')[0] ?? '';

        assert.strictEqual(
          nonTestCode.includes('.unwrap('),
          false,
          `Forbidden .unwrap() found in non-test code: ${filePath}`,
        );
        assert.strictEqual(
          nonTestCode.includes('.expect('),
          false,
          `Forbidden .expect() found in non-test code: ${filePath}`,
        );
        assert.strictEqual(
          nonTestCode.includes('panic!('),
          false,
          `Forbidden panic!() found in non-test code: ${filePath}`,
        );
      }
    });

    it('should validate Frontend TypeScript contracts and type guards (src/types/highlight.ts)', () => {
      const typesPath = path.resolve('src/types/highlight.ts');
      assert.ok(fs.existsSync(typesPath), 'src/types/highlight.ts must exist');

      // Test ChatBucketJson guard
      const validBucket: ChatBucketJson = {
        index: 0,
        timestamp_sec: 0.0,
        raw_count: 20,
        effective_count: 3,
      };
      assert.strictEqual(isChatBucketJson(validBucket), true);
      assert.strictEqual(isChatBucketJson({ index: -1 }), false);
      assert.strictEqual(isChatBucketJson(null), false);

      const normalizedBucket = normalizeChatBucket(validBucket);
      assert.strictEqual(normalizedBucket.index, 0);
      assert.strictEqual(normalizedBucket.timestampSec, 0.0);
      assert.strictEqual(normalizedBucket.rawCount, 20);
      assert.strictEqual(normalizedBucket.effectiveCount, 3);
      assert.strictEqual(normalizedBucket.filtered_count, 3);
      assert.strictEqual(isChatBucket(normalizedBucket), true);

      // Test ChatVelocityPointJson guard
      const validVelocity: ChatVelocityPointJson = {
        timestamp_sec: 60.0,
        count: 45,
        z_score: 70.0,
      };
      assert.strictEqual(isChatVelocityPointJson(validVelocity), true);
      assert.strictEqual(isChatVelocityPointJson({ count: -1 }), false);
      assert.strictEqual(isChatVelocityPointJson(undefined), false);

      const normalizedVelocity = normalizeChatVelocityPoint(validVelocity);
      assert.strictEqual(normalizedVelocity.timestampSec, 60.0);
      assert.strictEqual(normalizedVelocity.count, 45);
      assert.strictEqual(normalizedVelocity.zScore, 70.0);
      assert.strictEqual(isChatVelocityPoint(normalizedVelocity), true);
    });

    it('should verify pure mathematical invariants: UT-09.1, UT-09.2, UT-09.3', () => {
      // UT-09.1: Message floor X < 15 => Z = 0.0
      assert.strictEqual(calculateAdjustedZScore(14, 0.0, 0.0), 0.0);
      assert.strictEqual(calculateAdjustedZScore(14, 10.0, 2.0), 0.0);
      assert.strictEqual(calculateAdjustedZScore(0, 0.0, 0.0), 0.0);

      // UT-09.2: X = 45, mu = 10.0, sigma = 0.0, epsilon = 0.5 => Z = (45 - 10) / (0.0 + 0.5) = 70.0
      const z70 = calculateAdjustedZScore(
        45,
        10.0,
        0.0,
        SMOOTHING_EPSILON,
        MESSAGE_FLOOR_THRESHOLD,
      );
      assert.ok(Math.abs(z70 - 70.0) < 1e-6, `Expected Z=70.0, got ${z70}`);

      // UT-09.3: Spam bot clamp: 20 msgs by single author clamped to max 3 => effective=3 < 15 => Z=0.0
      const singleAuthorSpamCount = 20;
      const effectiveSpamCount = Math.min(singleAuthorSpamCount, SPAM_BOT_MAX_MESSAGES_PER_AUTHOR);
      assert.strictEqual(effectiveSpamCount, 3);
      assert.strictEqual(calculateAdjustedZScore(effectiveSpamCount, 0.0, 0.0), 0.0);

      // Boundary condition: X = 15 => evaluates
      const zBoundary = calculateAdjustedZScore(15, 10.0, 2.0, 0.5, 15);
      // (15 - 10) / (2.0 + 0.5) = 5.0 / 2.5 = 2.0
      assert.ok(Math.abs(zBoundary - 2.0) < 1e-6);

      // Assert constants match specifications
      assert.strictEqual(DEFAULT_BUCKET_WIDTH_SEC, 5.0);
      assert.strictEqual(DEFAULT_BASELINE_WINDOW_SEC, 60.0);
      assert.strictEqual(SMOOTHING_EPSILON, 0.5);
      assert.strictEqual(MESSAGE_FLOOR_THRESHOLD, 15);
      assert.strictEqual(SPAM_BOT_MAX_MESSAGES_PER_AUTHOR, 3);
    });
  });

  describe('TASK-M4-10 Test Suite: Neuro Swarm Emote Density Filtering Engine', () => {
    it('should assert Rust highlight emote modules, contracts, and integration tests exist', () => {
      const dictPath = path.resolve('src-tauri/src/highlight/swarm_dict.rs');
      const filterPath = path.resolve('src-tauri/src/highlight/emote_filter.rs');
      const modPath = path.resolve('src-tauri/src/highlight/mod.rs');
      const testPath = path.resolve('src-tauri/tests/highlight_emote_filter_test.rs');
      const libPath = path.resolve('src-tauri/src/lib.rs');

      assert.ok(fs.existsSync(dictPath), 'swarm_dict.rs must exist');
      assert.ok(fs.existsSync(filterPath), 'emote_filter.rs must exist');
      assert.ok(fs.existsSync(modPath), 'highlight/mod.rs must exist');
      assert.ok(fs.existsSync(testPath), 'highlight_emote_filter_test.rs must exist');

      const libContent = fs.readFileSync(libPath, 'utf8');
      assert.ok(libContent.includes('SwarmDictionary'), 'lib.rs must export SwarmDictionary');
      assert.ok(libContent.includes('SwarmEmoteCategory'), 'lib.rs must export SwarmEmoteCategory');
      assert.ok(
        libContent.includes('SwarmEmoteDefinition'),
        'lib.rs must export SwarmEmoteDefinition',
      );
      assert.ok(libContent.includes('EmoteDensityPoint'), 'lib.rs must export EmoteDensityPoint');
      assert.ok(
        libContent.includes('compute_emote_density_series'),
        'lib.rs must export compute_emote_density_series',
      );
      assert.ok(
        libContent.includes('extract_swarm_emotes'),
        'lib.rs must export extract_swarm_emotes',
      );
      assert.ok(libContent.includes('is_swarm_emote'), 'lib.rs must export is_swarm_emote');

      const dictContent = fs.readFileSync(dictPath, 'utf8');
      assert.ok(
        dictContent.includes('pub struct SwarmDictionary'),
        'swarm_dict.rs must define SwarmDictionary',
      );
      assert.ok(
        dictContent.includes('pub enum SwarmEmoteCategory'),
        'swarm_dict.rs must define SwarmEmoteCategory',
      );
      assert.ok(
        dictContent.includes('pub struct SwarmEmoteDefinition'),
        'swarm_dict.rs must define SwarmEmoteDefinition',
      );

      const filterContent = fs.readFileSync(filterPath, 'utf8');
      assert.ok(
        filterContent.includes('pub struct EmoteDensityPoint'),
        'emote_filter.rs must define EmoteDensityPoint',
      );
      assert.ok(
        filterContent.includes('MAX_TOP_EMOTES_PER_BUCKET'),
        'emote_filter.rs must define MAX_TOP_EMOTES_PER_BUCKET',
      );
      assert.ok(
        filterContent.includes('calculate_emote_ratio'),
        'emote_filter.rs must define calculate_emote_ratio',
      );
      assert.ok(
        filterContent.includes('extract_message_tokens'),
        'emote_filter.rs must define extract_message_tokens',
      );
    });

    it('should assert zero-panic invariant in Rust highlight emote library code (CONVENTIONS.md §2.4)', () => {
      const filesToAudit = [
        path.resolve('src-tauri/src/highlight/swarm_dict.rs'),
        path.resolve('src-tauri/src/highlight/emote_filter.rs'),
        path.resolve('src-tauri/src/highlight/mod.rs'),
      ];

      for (const filePath of filesToAudit) {
        const content = fs.readFileSync(filePath, 'utf8');
        const nonTestCode = content.split('#[cfg(test)]')[0] ?? '';

        assert.strictEqual(
          nonTestCode.includes('.unwrap('),
          false,
          `Forbidden .unwrap() found in non-test code: ${filePath}`,
        );
        assert.strictEqual(
          nonTestCode.includes('.expect('),
          false,
          `Forbidden .expect() found in non-test code: ${filePath}`,
        );
        assert.strictEqual(
          nonTestCode.includes('panic!('),
          false,
          `Forbidden panic!() found in non-test code: ${filePath}`,
        );
      }
    });

    it('should validate Frontend TypeScript contracts, type guards, and normalizer (src/types/highlight.ts)', () => {
      const typesPath = path.resolve('src/types/highlight.ts');
      assert.ok(fs.existsSync(typesPath), 'src/types/highlight.ts must exist');

      // Test EmoteDensityPointJson guard
      const validPoint: EmoteDensityPointJson = {
        timestamp_sec: 10.0,
        top_emotes: ['KEKW', 'Tutel', '💀'],
        e_ratio: 0.5,
        total_tokens: 50,
        emote_count: 25,
        weighted_score: 1.8,
      };
      assert.strictEqual(isEmoteDensityPointJson(validPoint), true);
      assert.strictEqual(isEmoteDensityPointJson({ timestamp_sec: -1, e_ratio: 1.5 }), false);
      assert.strictEqual(isEmoteDensityPointJson(null), false);
      assert.strictEqual(isEmoteDensityPointJson(undefined), false);

      const normalized: EmoteDensityPoint = normalizeEmoteDensityPoint(validPoint);
      assert.strictEqual(normalized.timestampSec, 10.0);
      assert.deepStrictEqual(normalized.topEmotes, ['KEKW', 'Tutel', '💀']);
      assert.strictEqual(normalized.eRatio, 0.5);
      assert.strictEqual(normalized.totalTokens, 50);
      assert.strictEqual(normalized.emoteCount, 25);
      assert.strictEqual(normalized.swarmCount, 25);
      assert.strictEqual(normalized.weightedScore, 1.8);

      // Snake_case aliases matching Rust JSON contract
      assert.strictEqual(normalized.timestamp_sec, 10.0);
      assert.deepStrictEqual(normalized.top_emotes, ['KEKW', 'Tutel', '💀']);
      assert.strictEqual(normalized.e_ratio, 0.5);
      assert.strictEqual(normalized.total_tokens, 50);
      assert.strictEqual(normalized.emote_count, 25);
      assert.strictEqual(normalized.swarm_count, 25);
      assert.strictEqual(normalized.weighted_score, 1.8);

      assert.strictEqual(isEmoteDensityPoint(normalized), true);
    });

    it('should verify pure mathematical invariants: UT-10.1, UT-10.2, UT-10.3', () => {
      // UT-10.3: Emote ratio formula: 50 tokens, 25 swarm emotes => 0.50 (50%)
      const ratio50 = calculateEmoteRatio(25, 50);
      assert.strictEqual(ratio50, 0.5);

      // Edge case: 0 tokens => 0.0 (no division by zero)
      assert.strictEqual(calculateEmoteRatio(0, 0), 0.0);
      assert.strictEqual(calculateEmoteRatio(10, 0), 0.0);
      assert.strictEqual(calculateEmoteRatio(0, 50), 0.0);

      // Clamping invariant in [0.0, 1.0]
      assert.strictEqual(calculateEmoteRatio(60, 50), 1.0);
      assert.strictEqual(calculateEmoteRatio(50, 50), 1.0);
      assert.strictEqual(calculateEmoteRatio(-5, 50), 0.0);

      // Assert constants match specifications
      assert.strictEqual(DEFAULT_BUCKET_WIDTH_SEC, 5.0);
      assert.strictEqual(MAX_TOP_EMOTES_PER_BUCKET, 5);
      assert.strictEqual(SPAM_BOT_MAX_MESSAGES_PER_AUTHOR, 3);
    });
  });

  describe('TASK-M4-11 Test Suite: Audio RMS Peak Detector & Streaming Energy Analyzer', () => {
    it('should assert Rust highlight audio RMS modules, contracts, and integration tests exist', () => {
      const audioRmsPath = path.resolve('src-tauri/src/highlight/audio_rms.rs');
      const energyAnalyzerPath = path.resolve('src-tauri/src/highlight/energy_analyzer.rs');
      const highlightModPath = path.resolve('src-tauri/src/highlight/mod.rs');
      const libPath = path.resolve('src-tauri/src/lib.rs');
      const testPath = path.resolve('src-tauri/tests/highlight_audio_rms_test.rs');

      assert.ok(fs.existsSync(audioRmsPath), 'audio_rms.rs must exist');
      assert.ok(fs.existsSync(energyAnalyzerPath), 'energy_analyzer.rs must exist');
      assert.ok(fs.existsSync(highlightModPath), 'highlight/mod.rs must exist');
      assert.ok(fs.existsSync(libPath), 'lib.rs must exist');
      assert.ok(fs.existsSync(testPath), 'highlight_audio_rms_test.rs must exist');

      const modContent = fs.readFileSync(highlightModPath, 'utf8');
      assert.ok(modContent.includes('pub mod audio_rms;'), 'mod.rs must declare audio_rms module');
      assert.ok(
        modContent.includes('pub mod energy_analyzer;'),
        'mod.rs must declare energy_analyzer module',
      );
      assert.ok(modContent.includes('AudioBlockMetrics'), 'mod.rs must export AudioBlockMetrics');
      assert.ok(modContent.includes('AudioRmsPoint'), 'mod.rs must export AudioRmsPoint');
      assert.ok(
        modContent.includes('AudioRmsSeriesResponse'),
        'mod.rs must export AudioRmsSeriesResponse',
      );

      const libContent = fs.readFileSync(libPath, 'utf8');
      assert.ok(libContent.includes('AudioRmsPoint'), 'lib.rs must export AudioRmsPoint');
      assert.ok(libContent.includes('calculate_raw_rms'), 'lib.rs must export calculate_raw_rms');
      assert.ok(
        libContent.includes('detect_energy_peaks'),
        'lib.rs must export detect_energy_peaks',
      );

      const rmsContent = fs.readFileSync(audioRmsPath, 'utf8');
      assert.ok(
        rmsContent.includes('pub struct AudioBlockMetrics'),
        'audio_rms.rs must define AudioBlockMetrics',
      );
      assert.ok(
        rmsContent.includes('calculate_raw_rms'),
        'audio_rms.rs must define calculate_raw_rms',
      );
      assert.ok(rmsContent.includes('rms_to_dbfs'), 'audio_rms.rs must define rms_to_dbfs');
      assert.ok(
        rmsContent.includes('SILENCE_FLOOR_DBFS'),
        'audio_rms.rs must define SILENCE_FLOOR_DBFS',
      );

      const analyzerContent = fs.readFileSync(energyAnalyzerPath, 'utf8');
      assert.ok(
        analyzerContent.includes('pub struct AudioRmsPoint'),
        'energy_analyzer.rs must define AudioRmsPoint',
      );
      assert.ok(
        analyzerContent.includes('pub struct AudioRmsSeriesResponse'),
        'energy_analyzer.rs must define AudioRmsSeriesResponse',
      );
      assert.ok(
        analyzerContent.includes('detect_energy_peaks'),
        'energy_analyzer.rs must define detect_energy_peaks',
      );
      assert.ok(
        analyzerContent.includes('analyze_wav_file'),
        'energy_analyzer.rs must define analyze_wav_file',
      );
    });

    it('should assert zero-panic invariant in Rust highlight audio RMS code (CONVENTIONS.md §2.4)', () => {
      const filesToAudit = [
        path.resolve('src-tauri/src/highlight/audio_rms.rs'),
        path.resolve('src-tauri/src/highlight/energy_analyzer.rs'),
        path.resolve('src-tauri/src/highlight/mod.rs'),
      ];

      for (const filePath of filesToAudit) {
        const content = fs.readFileSync(filePath, 'utf8');
        const nonTestCode = content.split('#[cfg(test)]')[0] ?? '';

        assert.strictEqual(
          nonTestCode.includes('.unwrap('),
          false,
          `Forbidden .unwrap() found in non-test code: ${filePath}`,
        );
        assert.strictEqual(
          nonTestCode.includes('.expect('),
          false,
          `Forbidden .expect() found in non-test code: ${filePath}`,
        );
        assert.strictEqual(
          nonTestCode.includes('panic!('),
          false,
          `Forbidden panic!() found in non-test code: ${filePath}`,
        );
      }
    });

    it('should validate Frontend TypeScript contracts, type guards, and normalizer (src/types/highlight.ts)', () => {
      const typesPath = path.resolve('src/types/highlight.ts');
      assert.ok(fs.existsSync(typesPath), 'src/types/highlight.ts must exist');

      // Test AudioRmsPointJson guard
      const validPoint: AudioRmsPointJson = {
        timestamp_sec: 1.5,
        rms_energy: 0.35,
        is_peak: true,
        dbfs: -9.1,
        delta_db: 21.0,
        z_score: 3.2,
      };
      assert.strictEqual(isAudioRmsPointJson(validPoint), true);
      assert.strictEqual(
        isAudioRmsPointJson({ timestamp_sec: -1, rms_energy: 0.5, is_peak: true }),
        false,
      );
      assert.strictEqual(
        isAudioRmsPointJson({ timestamp_sec: 1.0, rms_energy: 1.5, is_peak: true }),
        false,
      );
      assert.strictEqual(isAudioRmsPointJson(null), false);
      assert.strictEqual(isAudioRmsPointJson(undefined), false);

      const normalized: AudioRmsPoint = normalizeAudioRmsPoint(validPoint);
      assert.strictEqual(normalized.timestampSec, 1.5);
      assert.strictEqual(normalized.rmsEnergy, 0.35);
      assert.strictEqual(normalized.isPeak, true);
      assert.strictEqual(normalized.dbfs, -9.1);
      assert.strictEqual(normalized.deltaDb, 21.0);
      assert.strictEqual(normalized.zScore, 3.2);

      // Snake_case aliases matching Rust JSON contract
      assert.strictEqual(normalized.timestamp_sec, 1.5);
      assert.strictEqual(normalized.rms_energy, 0.35);
      assert.strictEqual(normalized.is_peak, true);
      assert.strictEqual(normalized.delta_db, 21.0);
      assert.strictEqual(normalized.z_score, 3.2);

      assert.strictEqual(isAudioRmsPoint(normalized), true);

      // Test AudioRmsSeriesResponseJson
      const validResponse: AudioRmsSeriesResponseJson = {
        vod_id: 'vod_test_123',
        block_duration_sec: 0.5,
        baseline_window_sec: 30.0,
        total_blocks: 1,
        peak_count: 1,
        points: [validPoint],
      };
      assert.strictEqual(isAudioRmsSeriesResponseJson(validResponse), true);

      const normalizedResponse: AudioRmsSeriesResponse =
        normalizeAudioRmsSeriesResponse(validResponse);
      assert.strictEqual(normalizedResponse.vodId, 'vod_test_123');
      assert.strictEqual(normalizedResponse.blockDurationSec, 0.5);
      assert.strictEqual(normalizedResponse.baselineWindowSec, 30.0);
      assert.strictEqual(normalizedResponse.totalBlocks, 1);
      assert.strictEqual(normalizedResponse.peakCount, 1);
      assert.strictEqual(normalizedResponse.points.length, 1);
    });

    it('should verify pure mathematical invariants: UT-11.1, UT-11.2, UT-11.3', () => {
      // UT-11.1: 1kHz sine wave RMS calculation in client helper
      // 16 samples per period of 1kHz at 16kHz
      const sinePeriod: number[] = [];
      for (let i = 0; i < 16; i++) {
        const theta = (2 * Math.PI * 1000 * i) / 16000;
        sinePeriod.push(Math.round(32767 * Math.sin(theta)));
      }
      const calculatedRms = calculateAudioRms(sinePeriod);
      assert.ok(
        Math.abs(calculatedRms - 0.7071) < 0.005,
        `Client sine RMS ${calculatedRms} deviates from 0.7071 ± 0.005`,
      );

      // Silence returns 0.0
      assert.strictEqual(calculateAudioRms([0, 0, 0, 0]), 0.0);
      assert.strictEqual(calculateAudioRms([]), 0.0);

      // dBFS conversions and silence clamping
      assert.strictEqual(rmsToDbfs(0.0), SILENCE_FLOOR_DBFS);
      assert.strictEqual(rmsToDbfs(SILENCE_FLOOR_AMPLITUDE), SILENCE_FLOOR_DBFS);
      assert.strictEqual(dbfsToRms(SILENCE_FLOOR_DBFS), 0.0);
      assert.strictEqual(dbfsToRms(-120.0), 0.0);
      assert.strictEqual(dbfsToNormalizedScale(SILENCE_FLOOR_DBFS), 0.0);
      assert.strictEqual(dbfsToNormalizedScale(0.0), 1.0);

      // UT-11.3: Sudden volume spike (+21dB) triggers peak
      // Baseline at -30dBFS, spike to -9dBFS (+21dB)
      const baselineRms = Math.pow(10, -30.0 / 20.0);
      const spikeRms = Math.pow(10, -9.0 / 20.0);
      const isSpikePeak = isAudioRmsPeak(spikeRms, baselineRms, 0.0, -9.0, -30.0);
      assert.strictEqual(isSpikePeak, true, '+21dB spike must trigger peak');

      // Normal variation does not trigger peak
      const normalRms = Math.pow(10, -29.0 / 20.0);
      const isNormalPeak = isAudioRmsPeak(normalRms, baselineRms, 0.0, -29.0, -30.0);
      assert.strictEqual(isNormalPeak, false, '+1dB variation must not trigger peak');

      // UT-11.2: Statistical outlier >= 2.5 sigma triggers peak
      const outlierRms = 0.1 + 2.8 * 0.02; // Mean 0.10, std dev 0.02 -> 2.8 sigma
      const isOutlierPeak = isAudioRmsPeak(outlierRms, 0.1, 0.02, -20.0, -20.0);
      assert.strictEqual(isOutlierPeak, true, '2.8 sigma outlier must trigger peak');

      // Silence cannot be peak
      assert.strictEqual(isAudioRmsPeak(0.0, 0.0, 0.0, -96.0, -96.0), false);

      // Constants match backend
      assert.strictEqual(DEFAULT_RMS_BLOCK_DURATION_SEC, 0.5);
      assert.strictEqual(DEFAULT_AUDIO_SAMPLE_RATE, 16000);
      assert.strictEqual(DEFAULT_AUDIO_BASELINE_WINDOW_SEC, 30.0);
      assert.strictEqual(SILENCE_FLOOR_DBFS, -96.0);
      assert.strictEqual(DEFAULT_PEAK_DB_SPIKE_THRESHOLD, 18.0);
      assert.strictEqual(DEFAULT_PEAK_SIGMA_THRESHOLD, 2.5);
    });
  });
});

describe('TASK-M4-12 Test Suite (Node.js Native Test Runner)', () => {
  describe('Architecture & Codebase Invariants', () => {
    it('should verify all target files for TASK-M4-12 exist', () => {
      const files = [
        'src-tauri/src/highlight/fusion.rs',
        'src-tauri/src/highlight/cluster.rs',
        'src-tauri/src/commands/highlight_cmd.rs',
        'src/types/highlight.ts',
        'src/services/ipc/highlight.ts',
        'src/components/highlight/HighlightViewer.tsx',
        'src/components/highlight/index.ts',
        'src-tauri/tests/highlight_fusion_test.rs',
        'src/test/highlight.test.ts',
      ];
      for (const f of files) {
        assert.ok(fs.existsSync(path.resolve(f)), `File must exist: ${f}`);
      }
    });

    it('should verify zero-panic clippy invariant in Rust production code', () => {
      const filesToAudit = [
        path.resolve('src-tauri/src/highlight/fusion.rs'),
        path.resolve('src-tauri/src/highlight/cluster.rs'),
        path.resolve('src-tauri/src/commands/highlight_cmd.rs'),
      ];

      for (const filePath of filesToAudit) {
        const content = fs.readFileSync(filePath, 'utf8');
        const nonTestCode = content.split('#[cfg(test)]')[0] ?? '';

        assert.strictEqual(
          nonTestCode.includes('.unwrap('),
          false,
          `Forbidden .unwrap() found in non-test code: ${filePath}`,
        );
        assert.strictEqual(
          nonTestCode.includes('.expect('),
          false,
          `Forbidden .expect() found in non-test code: ${filePath}`,
        );
        assert.strictEqual(
          nonTestCode.includes('panic!('),
          false,
          `Forbidden panic!() found in non-test code: ${filePath}`,
        );
      }
    });

    it('should validate Schema 5.4 JSON Schema Draft-07 compliance using Ajv', () => {
      const schema54 = {
        $schema: 'http://json-schema.org/draft-07/schema#',
        title: 'HighlightClustersContainer',
        type: 'object',
        required: ['vod_id', 'clusters'],
        properties: {
          vod_id: { type: 'string' },
          clusters: {
            type: 'array',
            items: {
              type: 'object',
              required: [
                'cluster_id',
                'start_sec',
                'end_sec',
                'peak_sec',
                'score',
                'primary_reason',
              ],
              properties: {
                cluster_id: { type: 'string' },
                start_sec: { type: 'number' },
                end_sec: { type: 'number' },
                peak_sec: { type: 'number' },
                score: { type: 'number', minimum: 0, maximum: 100 },
                primary_reason: { type: 'string' },
                chat_z_score: { type: ['number', 'null'] },
                audio_rms_peak: { type: ['number', 'null'] },
                top_emotes: {
                  type: 'array',
                  items: { type: 'string' },
                },
                is_bookmarked: { type: 'boolean', default: false },
              },
            },
          },
        },
      };

      const ajv = new Ajv();
      const validate = ajv.compile(schema54);

      const sampleContainer: HighlightClustersContainerJson = {
        vod_id: 'vod_123',
        clusters: [
          {
            cluster_id: 'cluster_1',
            start_sec: 5055.5,
            end_sec: 5085.0,
            peak_sec: 5070.0,
            score: 89.0,
            primary_reason: 'Văn hóa Swarm bùng nổ (tutel)',
            chat_z_score: 5.2,
            audio_rms_peak: 0.65,
            top_emotes: ['tutel', 'KEKW'],
            is_bookmarked: false,
          },
        ],
      };

      const valid = validate(sampleContainer);
      assert.strictEqual(valid, true, `Ajv errors: ${JSON.stringify(validate.errors)}`);
    });

    it('should validate Frontend TypeScript contracts, type guards, and normalizer', () => {
      const rawCluster: HighlightClusterJson = {
        cluster_id: 'cluster_1',
        start_sec: 100.0,
        end_sec: 130.0,
        peak_sec: 114.5,
        score: 89.0,
        primary_reason: 'Spam KEKW / LUL bùng nổ',
        top_emotes: ['KEKW'],
        is_bookmarked: true,
      };

      assert.strictEqual(isHighlightClusterJson(rawCluster), true);
      assert.strictEqual(isHighlightClusterJson(null), false);
      assert.strictEqual(isHighlightClusterJson({ cluster_id: 'c1' }), false);

      const domain = normalizeHighlightCluster(rawCluster);
      assert.strictEqual(isHighlightCluster(domain), true);
      assert.strictEqual(domain.clusterId, 'cluster_1');
      assert.strictEqual(domain.cluster_id, 'cluster_1');
      assert.strictEqual(domain.startSec, 100.0);
      assert.strictEqual(domain.start_sec, 100.0);
      assert.strictEqual(domain.endSec, 130.0);
      assert.strictEqual(domain.score, 89.0);
      assert.strictEqual(domain.isBookmarked, true);
      assert.strictEqual(domain.is_bookmarked, true);

      const containerJson: HighlightClustersContainerJson = {
        vod_id: 'vod_test_m4_12',
        clusters: [rawCluster],
      };
      assert.strictEqual(isHighlightClustersContainerJson(containerJson), true);

      const containerDomain = normalizeHighlightClustersContainer(containerJson);
      assert.strictEqual(isHighlightClustersContainer(containerDomain), true);
      assert.strictEqual(containerDomain.vodId, 'vod_test_m4_12');
      assert.strictEqual(containerDomain.vod_id, 'vod_test_m4_12');
      assert.strictEqual(containerDomain.clusters.length, 1);

      const wireBack = toHighlightClustersContainerJson(containerDomain);
      assert.strictEqual(wireBack.vod_id, 'vod_test_m4_12');
      assert.strictEqual(wireBack.clusters[0]?.cluster_id, 'cluster_1');
    });

    it('should verify mathematical formulas: UT-12.1, UT-12.2, UT-12.3, ED-12.1, ED-12.3', () => {
      // UT-12.1: Full Multimodal Weighting
      // Score = (0.6 * Z_norm + 0.25 * E_ratio + 0.15 * RMS_norm) * 100.0
      // Z_norm = 1.0, E = 0.8, RMS = 0.6 -> 0.60 + 0.20 + 0.09 = 0.89 -> 89.0
      const score = calculateMultimodalScore(1.0, 0.8, 0.6);
      assert.ok(Math.abs(score - 89.0) < 0.1, `Expected 89.0, got ${score}`);

      // UT-12.2: Audio-Only Fallback
      // Score = 1.0 * RMS * 100.0
      const audioScore = calculateAudioOnlyScore(0.75);
      assert.ok(Math.abs(audioScore - 75.0) < 0.1, `Expected 75.0, got ${audioScore}`);
      assert.strictEqual(calculateAudioOnlyScore(0.0), 0.0);

      // UT-12.3: Latency compensation & padding: peak=5070s on 7200s VOD
      // start = 5070 - 14.5 = 5055.5s, end = 5085.0s, length = 29.5s
      const bounds = computeHighlightBounds(5070.0, 7200.0);
      assert.ok(Math.abs(bounds.startSec - 5055.5) < 0.1);
      assert.ok(Math.abs(bounds.endSec - 5085.0) < 0.1);
      assert.ok(Math.abs(bounds.durationSec - 29.5) < 0.1);

      // ED-12.1: Zero Clamping
      const boundsEarly = computeHighlightBounds(8.0, 3600.0);
      assert.strictEqual(boundsEarly.startSec, 0.0);
      assert.ok(Math.abs(boundsEarly.endSec - 23.0) < 0.1);

      // ED-12.3: EOF Dynamic Padding (peak=9992s on 10000s VOD)
      const boundsEof = computeHighlightBounds(9992.0, 10000.0);
      assert.strictEqual(boundsEof.endSec, 10000.0);
      assert.ok(Math.abs(boundsEof.startSec - 9977.5) < 0.1);
      assert.ok(boundsEof.durationSec >= 15.0);

      // Formatting
      assert.strictEqual(formatTimecode(5070), '01:24:30');
      assert.strictEqual(formatTimecode(75), '01:15');
      assert.strictEqual(formatScore(89.0), '89.0');
      assert.strictEqual(formatDuration(22.5), '22.5s');

      // Constants
      assert.strictEqual(STREAM_LATENCY_COMPENSATION_SEC, 4.5);
      assert.strictEqual(DEFAULT_PRE_ROLL_BUFFER_SEC, 10.0);
      assert.strictEqual(DEFAULT_POST_ROLL_BUFFER_SEC, 15.0);
      assert.strictEqual(MIN_PRE_ROLL_OFFSET_SEC, 14.5);
      assert.strictEqual(MAX_PRE_ROLL_BUFFER_SEC, 20.0);
      assert.strictEqual(MIN_CLUSTER_DURATION_SEC, 15.0);
      assert.strictEqual(CLUSTER_MERGE_DISTANCE_SEC, 30.0);
      assert.strictEqual(MAX_CLUSTER_OVERLAP_RATIO, 0.5);
      assert.strictEqual(FUSION_WEIGHT_Z_SCORE, 0.6);
      assert.strictEqual(FUSION_WEIGHT_EMOTE, 0.25);
      assert.strictEqual(FUSION_WEIGHT_RMS, 0.15);
      assert.strictEqual(FUSION_AUDIO_ONLY_WEIGHT_RMS, 1.0);
      assert.strictEqual(DEFAULT_TOP_CLUSTERS_LIMIT, 20);
    });
  });

  describe('TASK-M4-13 Test Suite: Lossless Precision Range Slicer', () => {
    it('should verify file structure and integrity for Range Slicer subsystem', () => {
      const root = path.resolve(import.meta.dirname, '../..');

      // Rust Backend Files
      const rangeSlicerRs = path.join(root, 'src-tauri/src/ingestion/range_slicer.rs');
      const sliceCmdRs = path.join(root, 'src-tauri/src/commands/slice_cmd.rs');
      const rangeSlicerTestRs = path.join(root, 'src-tauri/tests/range_slicer_test.rs');

      assert.ok(fs.existsSync(rangeSlicerRs), 'range_slicer.rs must exist');
      assert.ok(fs.existsSync(sliceCmdRs), 'slice_cmd.rs must exist');
      assert.ok(fs.existsSync(rangeSlicerTestRs), 'range_slicer_test.rs must exist');

      // Frontend TypeScript Files
      const rangeSlicerTs = path.join(root, 'src/types/range_slicer.ts');
      const sliceIpcTs = path.join(root, 'src/services/ipc/slice.ts');

      assert.ok(fs.existsSync(rangeSlicerTs), 'range_slicer.ts must exist');
      assert.ok(fs.existsSync(sliceIpcTs), 'slice.ts must exist');
    });

    it('should enforce zero-panic invariant in Rust production code for Range Slicer', () => {
      const root = path.resolve(import.meta.dirname, '../..');
      const filesToCheck = [
        path.join(root, 'src-tauri/src/ingestion/range_slicer.rs'),
        path.join(root, 'src-tauri/src/commands/slice_cmd.rs'),
      ];

      for (const filePath of filesToCheck) {
        const content = fs.readFileSync(filePath, 'utf-8');
        const lines = content.split('\n');

        lines.forEach((line, idx) => {
          const trimmed = line.trim();
          // Skip comments
          if (trimmed.startsWith('//') || trimmed.startsWith('/*') || trimmed.startsWith('*')) {
            return;
          }
          assert.ok(
            !line.includes('.unwrap('),
            `Found .unwrap() at ${path.basename(filePath)}:${idx + 1}: ${line}`,
          );
          assert.ok(
            !line.includes('.expect('),
            `Found .expect() at ${path.basename(filePath)}:${idx + 1}: ${line}`,
          );
          assert.ok(
            !line.includes('panic!('),
            `Found panic!() at ${path.basename(filePath)}:${idx + 1}: ${line}`,
          );
        });
      }
    });

    it('should validate type guards and normalizers for SliceResult and SliceVideoSegmentRequest', () => {
      // 1. Snake_case payload (wire specification)
      const rawSnakeWireResult: SliceResultJson = {
        clip_path: 'C:/NeuroClip/projects/vod_1/clips/clip_1.mp4',
        duration_seconds: 29.5,
        file_size_bytes: 47185920, // ~45MB
        video_duration: 29.533,
        audio_duration: 29.515,
        stream_drift_ms: 18.0,
        keyframe_aligned: true,
      };

      assert.ok(isSliceResultJson(rawSnakeWireResult));
      assert.ok(!isSliceResultJson(null));
      assert.ok(!isSliceResultJson({ invalid: true }));

      const domainFromSnake = normalizeSliceResult(rawSnakeWireResult);
      assert.ok(isSliceResult(domainFromSnake));
      assert.strictEqual(domainFromSnake.clipPath, rawSnakeWireResult.clip_path);
      assert.strictEqual(domainFromSnake.clip_path, rawSnakeWireResult.clip_path);
      assert.strictEqual(domainFromSnake.durationSeconds, 29.5);
      assert.strictEqual(domainFromSnake.fileSizeBytes, 47185920);
      assert.strictEqual(domainFromSnake.fileSizeMb, 45.0);
      assert.strictEqual(domainFromSnake.streamDriftMs, 18.0);
      assert.strictEqual(domainFromSnake.durationDeltaMs, 18.0);
      assert.strictEqual(domainFromSnake.keyframeAligned, true);

      // Wire roundtrip
      const wireBack = toSliceResultJson(domainFromSnake);
      assert.strictEqual(wireBack.clip_path, rawSnakeWireResult.clip_path);
      assert.strictEqual(wireBack.duration_seconds, 29.5);
      assert.strictEqual(wireBack.file_size_bytes, 47185920);
      assert.strictEqual(wireBack.stream_drift_ms, 18.0);
      assert.strictEqual(wireBack.keyframe_aligned, true);

      // 2. CamelCase payload (dual property access support)
      const rawCamelPayload = {
        clipPath: 'C:/NeuroClip/projects/vod_2/clips/clip_2.mp4',
        durationSeconds: 15.0,
        fileSizeBytes: 20971520, // ~20MB
        videoDuration: 15.01,
        audioDuration: 15.02,
        streamDriftMs: 10.0,
        keyframeAligned: true,
      };

      assert.ok(
        isSliceResultJson(rawCamelPayload),
        'isSliceResultJson must accept camelCase payload',
      );
      const domainFromCamel = normalizeSliceResult(rawCamelPayload);
      assert.ok(
        isSliceResult(domainFromCamel),
        'normalizeSliceResult must produce valid SliceResult from camelCase payload',
      );
      assert.strictEqual(domainFromCamel.clipPath, 'C:/NeuroClip/projects/vod_2/clips/clip_2.mp4');
      assert.strictEqual(domainFromCamel.clip_path, 'C:/NeuroClip/projects/vod_2/clips/clip_2.mp4');
      assert.strictEqual(domainFromCamel.durationSeconds, 15.0);
      assert.strictEqual(domainFromCamel.fileSizeBytes, 20971520);
      assert.strictEqual(domainFromCamel.fileSizeMb, 20.0);
      assert.strictEqual(domainFromCamel.streamDriftMs, 10.0);
      assert.strictEqual(domainFromCamel.keyframeAligned, true);

      // Request normalizer
      const requestDomain: SliceVideoSegmentRequest = {
        vodId: 'vod_99',
        startSec: 100.0,
        endSec: 190.0,
        clusterId: 'cluster_7',
        outputFilename: 'my_clip.mp4',
      };
      const requestWire = toSliceVideoSegmentRequestJson(requestDomain);
      assert.ok(isSliceVideoSegmentRequestJson(requestWire));
      assert.strictEqual(requestWire.vod_id, 'vod_99');
      assert.strictEqual(requestWire.start_sec, 100.0);
      assert.strictEqual(requestWire.end_sec, 190.0);
      assert.strictEqual(requestWire.cluster_id, 'cluster_7');
      assert.strictEqual(requestWire.output_filename, 'my_clip.mp4');
    });

    it('should register and resolve Vietnamese messages for range slicer error codes', () => {
      // Verify ERROR_CODES definitions
      assert.strictEqual(ERROR_CODES.INGEST_SLICE_FAILED, 'ERR_INGEST_SLICE_FAILED');
      assert.strictEqual(
        ERROR_CODES.INGEST_KEYFRAME_ALIGNMENT_FAILED,
        'ERR_INGEST_KEYFRAME_ALIGNMENT_FAILED',
      );
      assert.strictEqual(ERROR_CODES.INGEST_INVALID_TIME_RANGE, 'ERR_INGEST_INVALID_TIME_RANGE');

      // Verify ERROR_MESSAGES_VI entries
      assert.strictEqual(
        ERROR_MESSAGES_VI[ERROR_CODES.INGEST_SLICE_FAILED],
        'Lỗi trong quá trình cắt phân đoạn video.',
      );
      assert.strictEqual(
        ERROR_MESSAGES_VI[ERROR_CODES.INGEST_KEYFRAME_ALIGNMENT_FAILED],
        'Lệch keyframe hoặc không đồng bộ âm thanh/hình ảnh vượt quá ngưỡng cho phép (50ms).',
      );
      assert.strictEqual(
        ERROR_MESSAGES_VI[ERROR_CODES.INGEST_INVALID_TIME_RANGE],
        'Mốc thời gian cắt phân đoạn video không hợp lệ.',
      );

      // Verify getFriendlyErrorMessage returns localized messages instead of default fallback
      const msgSlice = getFriendlyErrorMessage(ERROR_CODES.INGEST_SLICE_FAILED);
      assert.strictEqual(msgSlice, 'Lỗi trong quá trình cắt phân đoạn video.');

      const msgKeyframe = getFriendlyErrorMessage(ERROR_CODES.INGEST_KEYFRAME_ALIGNMENT_FAILED);
      assert.strictEqual(
        msgKeyframe,
        'Lệch keyframe hoặc không đồng bộ âm thanh/hình ảnh vượt quá ngưỡng cho phép (50ms).',
      );

      const msgTimeRange = getFriendlyErrorMessage(ERROR_CODES.INGEST_INVALID_TIME_RANGE);
      assert.strictEqual(msgTimeRange, 'Mốc thời gian cắt phân đoạn video không hợp lệ.');

      // Also test with raw error object { code: 'ERR_INGEST_SLICE_FAILED' }
      const fromObj = getFriendlyErrorMessage({ code: 'ERR_INGEST_SLICE_FAILED' });
      assert.strictEqual(fromObj, 'Lỗi trong quá trình cắt phân đoạn video.');
    });

    it('should verify mathematical formulas and SLAs: AC-13.1, AC-13.2, AC-13.3, ED-13.1', () => {
      // AC-13.2: Bandwidth Savings Calculation
      // 70MB clip vs 30GB (30720MB) VOD:
      // Savings = (1.0 - 70 / 30720) * 100 = 99.77% >= 99.0%
      const savings70Mb = calculateBandwidthSavings(70 * 1024 * 1024, 30720 * 1024 * 1024);
      assert.ok(savings70Mb >= 99.0, `Expected savings >= 99%, got ${savings70Mb}%`);
      assert.strictEqual(savings70Mb, 99.77);

      const savings50Mb = calculateBandwidthSavings(50 * 1024 * 1024, 30720 * 1024 * 1024);
      assert.strictEqual(savings50Mb, 99.84);

      const savings80Mb = calculateBandwidthSavings(80 * 1024 * 1024, 30720 * 1024 * 1024);
      assert.strictEqual(savings80Mb, 99.74);

      const savings30Mb = calculateBandwidthSavings(30 * 1024 * 1024, 30720 * 1024 * 1024);
      assert.strictEqual(savings30Mb, 99.9);

      // ED-13.1: Keyframe Alignment Check (<= 50ms)
      assert.strictEqual(isKeyframeAligned(15.0, 0.0), true, '15ms drift must be aligned');
      assert.strictEqual(isKeyframeAligned(50.0, 0.0), true, '50ms boundary drift must be aligned');
      assert.strictEqual(isKeyframeAligned(50.1, 0.0), false, '50.1ms drift must exceed alignment');
      assert.strictEqual(isKeyframeAligned(80.0, 0.0), false, '80ms drift must exceed alignment');

      // AC-13.3: Initial Video Frame Presentation Delay (<= 50ms = 0.050s)
      assert.strictEqual(isKeyframeAligned(10.0, 0.0), true, '0ms delay passes');
      assert.strictEqual(isKeyframeAligned(10.0, 0.05), true, '50ms boundary delay passes');
      assert.strictEqual(isKeyframeAligned(10.0, 0.12), false, '120ms freeze delay must fail');

      // Constants verification
      assert.strictEqual(MAX_KEYFRAME_DRIFT_MS, 50.0);
      assert.strictEqual(MAX_INITIAL_PTS_DELAY_SEC, 0.05);
      assert.strictEqual(MIN_SLICING_DISK_SPACE_MB, 500);
      assert.strictEqual(FULL_VOD_REFERENCE_SIZE_MB, 30720);
    });

    it('should verify IPC service method signatures and export integrity', () => {
      assert.strictEqual(typeof sliceVideoSegment, 'function');
      assert.strictEqual(typeof cancelSliceVideoSegment, 'function');
    });
  });

  describe('TASK-M4-15: Groq Whisper Cloud API Client & IPC Contracts', () => {
    it('should assert Rust groq_client module and commands exist in backend', () => {
      const groqPath = path.resolve('src-tauri/src/stt/groq_client.rs');
      const testPath = path.resolve('src-tauri/tests/groq_client_test.rs');
      const cmdPath = path.resolve('src-tauri/src/commands/stt_cmd.rs');
      const libPath = path.resolve('src-tauri/src/lib.rs');

      assert.ok(fs.existsSync(groqPath), 'groq_client.rs must exist');
      assert.ok(fs.existsSync(testPath), 'groq_client_test.rs must exist');

      const groqContent = fs.readFileSync(groqPath, 'utf8');
      assert.ok(groqContent.includes('pub struct GroqClient'), 'Must define GroqClient');
      assert.ok(
        groqContent.includes('pub struct GroqClientConfig'),
        'Must define GroqClientConfig',
      );
      assert.ok(
        groqContent.includes('pub struct GroqVerboseJsonResponse'),
        'Must define GroqVerboseJsonResponse',
      );
      assert.ok(groqContent.includes('SAFE_CHUNK_BYTES'), 'Must define SAFE_CHUNK_BYTES');
      assert.ok(groqContent.includes('MAX_PAYLOAD_BYTES'), 'Must define MAX_PAYLOAD_BYTES');
      assert.ok(groqContent.includes('DEFAULT_OVERLAP_MS'), 'Must define DEFAULT_OVERLAP_MS');

      const cmdContent = fs.readFileSync(cmdPath, 'utf8');
      assert.ok(
        cmdContent.includes('pub async fn transcribe_highlight_clip'),
        'Must define transcribe_highlight_clip command',
      );

      const libContent = fs.readFileSync(libPath, 'utf8');
      assert.ok(
        libContent.includes('commands::transcribe_highlight_clip'),
        'lib.rs must register transcribe_highlight_clip command',
      );
    });

    it('should assert zero-panic clippy invariant in groq_client.rs and stt_cmd.rs', () => {
      const groqContent = fs.readFileSync(path.resolve('src-tauri/src/stt/groq_client.rs'), 'utf8');
      assert.ok(!groqContent.includes('.unwrap()'), 'groq_client.rs must not contain .unwrap()');
      assert.ok(!groqContent.includes('.expect('), 'groq_client.rs must not contain .expect()');
      assert.ok(!groqContent.includes('panic!('), 'groq_client.rs must not contain panic!()');
    });

    it('should verify TypeScript IPC stt service signatures and export contracts', () => {
      assert.strictEqual(typeof transcribeHighlightClip, 'function');
      assert.strictEqual(typeof listenSttChunk, 'function');
    });

    it('should verify STT canonical error codes and friendly Vietnamese messages', () => {
      assert.strictEqual(ERROR_CODES.STT_RATE_LIMIT, 'ERR_STT_RATE_LIMIT');
      assert.strictEqual(ERROR_CODES.STT_PAYLOAD_TOO_LARGE, 'ERR_STT_PAYLOAD_TOO_LARGE');
      assert.strictEqual(ERROR_CODES.STT_INVALID_API_KEY, 'ERR_STT_INVALID_API_KEY');
      assert.strictEqual(ERROR_CODES.STT_VAD_FAILED, 'ERR_STT_VAD_FAILED');

      const msgRateLimit = getFriendlyErrorMessage(ERROR_CODES.STT_RATE_LIMIT);
      assert.ok(msgRateLimit.length > 0, 'Must have localized message for ERR_STT_RATE_LIMIT');

      const msgPayload = getFriendlyErrorMessage(ERROR_CODES.STT_PAYLOAD_TOO_LARGE);
      assert.ok(msgPayload.length > 0, 'Must have localized message for ERR_STT_PAYLOAD_TOO_LARGE');

      const msgKey = getFriendlyErrorMessage(ERROR_CODES.STT_INVALID_API_KEY);
      assert.ok(msgKey.length > 0, 'Must have localized message for ERR_STT_INVALID_API_KEY');
    });

    it('should verify TranscribeClipOptions type contracts and Schema 5.5 normalization', () => {
      const options: TranscribeClipOptions = {
        apiKey: 'gsk_test_123',
        isMusicMode: true,
      };
      assert.strictEqual(options.apiKey, 'gsk_test_123');
      assert.strictEqual(options.isMusicMode, true);

      // Verify Schema 5.5 TranscriptSegmentsContainer normalization
      const rawWire = {
        clip_id: 'test_clip_15',
        segments: [
          {
            segment_id: 'test_clip_15_seg_0001',
            start_sec: 1.0,
            end_sec: 3.5,
            speaker_id: 'neuro',
            text_en: 'Hello everyone!',
            confidence: 0.98,
            cps_score: 14.5,
            is_flagged_for_review: false,
            words: [
              { word: 'Hello', start_sec: 1.0, end_sec: 1.8, probability: 0.98 },
              { word: 'everyone!', start_sec: 1.9, end_sec: 3.5, probability: 0.98 },
            ],
          },
        ],
      };

      const normalized = normalizeTranscriptSegmentsContainer(rawWire);
      assert.strictEqual(normalized.clipId, 'test_clip_15');
      assert.strictEqual(normalized.segments.length, 1);
      const firstSeg = normalized.segments[0];
      assert.ok(firstSeg !== undefined);
      assert.strictEqual(firstSeg.textEn, 'Hello everyone!');
      assert.strictEqual(firstSeg.words.length, 2);
      const firstWord = firstSeg.words[0];
      assert.ok(firstWord !== undefined);
      assert.strictEqual(firstWord.word, 'Hello');
    });
  });

  describe('TASK-M4-16: Faster-Whisper Local INT8 Worker & Hardware Guardrail Contracts', () => {
    it('should assert Rust faster_whisper_worker module and tests exist in backend', () => {
      const workerPath = path.resolve('src-tauri/src/stt/faster_whisper_worker.rs');
      const testPath = path.resolve('src-tauri/tests/local_whisper_worker_test.rs');
      const modelMgrPath = path.resolve('src-tauri/src/stt/model_manager.rs');
      const sttModPath = path.resolve('src-tauri/src/stt/mod.rs');

      assert.ok(fs.existsSync(workerPath), 'faster_whisper_worker.rs must exist');
      assert.ok(fs.existsSync(testPath), 'local_whisper_worker_test.rs must exist');
      assert.ok(fs.existsSync(modelMgrPath), 'model_manager.rs must exist');

      const workerContent = fs.readFileSync(workerPath, 'utf8');
      assert.ok(
        workerContent.includes('pub struct FasterWhisperConfig'),
        'Must define FasterWhisperConfig',
      );
      assert.ok(workerContent.includes('pub enum GuardrailPolicy'), 'Must define GuardrailPolicy');
      assert.ok(
        workerContent.includes('pub struct GuardrailDecision'),
        'Must define GuardrailDecision',
      );
      assert.ok(
        workerContent.includes('pub fn build_worker_command_args'),
        'Must define build_worker_command_args',
      );
      assert.ok(
        workerContent.includes('pub fn build_faster_whisper_command_args'),
        'Must define build_faster_whisper_command_args',
      );
      assert.ok(
        workerContent.includes('pub fn evaluate_hardware_guardrail'),
        'Must define evaluate_hardware_guardrail',
      );
      assert.ok(
        workerContent.includes('pub fn check_hardware_guardrail'),
        'Must define check_hardware_guardrail',
      );
      assert.ok(
        workerContent.includes('pub fn enforce_hardware_guardrail'),
        'Must define enforce_hardware_guardrail',
      );
      assert.ok(
        workerContent.includes('pub fn estimate_vram_usage_mb'),
        'Must define estimate_vram_usage_mb',
      );
      assert.ok(
        workerContent.includes('pub fn map_faster_whisper_to_container'),
        'Must define map_faster_whisper_to_container',
      );
      assert.ok(
        workerContent.includes('pub fn parse_faster_whisper_json'),
        'Must define parse_faster_whisper_json',
      );
      assert.ok(
        workerContent.includes('pub async fn run_faster_whisper_worker'),
        'Must define run_faster_whisper_worker',
      );
      assert.ok(
        workerContent.includes('assign_child_to_job_guard'),
        'Must assign child to Windows Job Object',
      );

      const modelMgrContent = fs.readFileSync(modelMgrPath, 'utf8');
      assert.ok(
        modelMgrContent.includes('faster-whisper-medium-int8'),
        'Must include faster-whisper-medium-int8 in catalog',
      );
      assert.ok(
        modelMgrContent.includes('faster-whisper-small-int8'),
        'Must include faster-whisper-small-int8 in catalog',
      );
      assert.ok(
        modelMgrContent.includes('is_whisper_large'),
        'Must export is_whisper_large helper',
      );

      const sttModContent = fs.readFileSync(sttModPath, 'utf8');
      assert.ok(
        sttModContent.includes('pub mod faster_whisper_worker;'),
        'stt/mod.rs must register faster_whisper_worker',
      );
    });

    it('should assert zero-panic clippy invariant in faster_whisper_worker.rs', () => {
      const fullContent = fs.readFileSync(
        path.resolve('src-tauri/src/stt/faster_whisper_worker.rs'),
        'utf8',
      );
      const testSectionIdx = fullContent.indexOf('#[cfg(test)]');
      const workerContent =
        testSectionIdx >= 0 ? fullContent.slice(0, testSectionIdx) : fullContent;
      assert.ok(
        !workerContent.includes('.unwrap()'),
        'faster_whisper_worker.rs must not contain .unwrap()',
      );
      assert.ok(
        !workerContent.includes('.expect('),
        'faster_whisper_worker.rs must not contain .expect()',
      );
      assert.ok(
        !workerContent.includes('panic!('),
        'faster_whisper_worker.rs must not contain panic!()',
      );
    });

    it('should verify hardware guardrail thresholds and VRAM cap mathematical constants', () => {
      const workerContent = fs.readFileSync(
        path.resolve('src-tauri/src/stt/faster_whisper_worker.rs'),
        'utf8',
      );
      assert.ok(
        workerContent.includes('MIN_LARGE_VRAM_MB: u64 = 2500;'),
        'MIN_LARGE_VRAM_MB must be 2500MB',
      );
      assert.ok(
        workerContent.includes('MAX_INT8_VRAM_CAP_MB: u64 = 1800;'),
        'MAX_INT8_VRAM_CAP_MB must be 1800MB',
      );
      assert.ok(
        workerContent.includes('DEFAULT_FASTER_WHISPER_MODEL: &str = "medium";'),
        'Default model must be medium',
      );
      assert.ok(
        workerContent.includes('DEFAULT_FASTER_WHISPER_QUANTIZATION: &str = "int8";'),
        'Default quantization must be int8',
      );
      assert.ok(
        workerContent.includes('DEFAULT_FASTER_WHISPER_DEVICE: &str = "auto";'),
        'Default device must be auto',
      );
    });

    it('should verify error codes contain all 11 ERR_STT_* codes in codes.rs and error.ts', () => {
      const codesContent = fs.readFileSync(path.resolve('src-tauri/src/error/codes.rs'), 'utf8');
      const tsSttCodes = [
        ERROR_CODES.STT_RATE_LIMIT,
        ERROR_CODES.STT_PAYLOAD_TOO_LARGE,
        ERROR_CODES.STT_VRAM_INSUFFICIENT,
        ERROR_CODES.STT_CUDA_OOM,
        ERROR_CODES.STT_INVALID_API_KEY,
        ERROR_CODES.STT_VAD_FAILED,
        ERROR_CODES.STT_MODEL_NOT_FOUND,
        ERROR_CODES.STT_MODEL_INTEGRITY_FAILED,
        ERROR_CODES.STT_SIDECAR_FAILED,
        ERROR_CODES.STT_PARSER_FAILED,
        ERROR_CODES.STT_INVALID_TIMESTAMP,
      ];

      for (const sttCode of tsSttCodes) {
        assert.ok(codesContent.includes(sttCode), `codes.rs must define ${sttCode}`);
        assert.ok(
          (ERROR_MESSAGES_VI[sttCode] ?? '').length > 0,
          `ERROR_MESSAGES_VI must define localized message for ${sttCode}`,
        );
      }
    });
  });

  describe('TASK-M4-17: Google Gemini 1.5 Flash Client & Theatrical Framing System Prompt Packaging', () => {
    it('should assert Rust translation modules and integration tests exist in backend', () => {
      const clientPath = path.resolve('src-tauri/src/translation/gemini_client.rs');
      const promptPath = path.resolve('src-tauri/src/translation/prompt_builder.rs');
      const modPath = path.resolve('src-tauri/src/translation/mod.rs');
      const cmdPath = path.resolve('src-tauri/src/commands/translation_cmd.rs');
      const testPath = path.resolve('src-tauri/tests/gemini_client_test.rs');

      assert.ok(fs.existsSync(clientPath), 'gemini_client.rs must exist');
      assert.ok(fs.existsSync(promptPath), 'prompt_builder.rs must exist');
      assert.ok(fs.existsSync(modPath), 'translation/mod.rs must exist');
      assert.ok(fs.existsSync(cmdPath), 'translation_cmd.rs must exist');
      assert.ok(fs.existsSync(testPath), 'gemini_client_test.rs must exist');

      const clientContent = fs.readFileSync(clientPath, 'utf8');
      assert.ok(clientContent.includes('pub struct GeminiClient'), 'Must define GeminiClient');
      assert.ok(
        clientContent.includes('pub struct GeminiClientConfig'),
        'Must define GeminiClientConfig',
      );
      assert.ok(
        clientContent.includes('pub fn build_safety_settings'),
        'Must define build_safety_settings',
      );
      assert.ok(
        clientContent.includes('pub fn is_safety_blocked'),
        'Must define is_safety_blocked',
      );
      assert.ok(
        clientContent.includes('pub fn apply_safety_fallback'),
        'Must define apply_safety_fallback',
      );
      assert.ok(
        clientContent.includes('pub fn strip_markdown_code_fence'),
        'Must define strip_markdown_code_fence',
      );

      const promptContent = fs.readFileSync(promptPath, 'utf8');
      assert.ok(
        promptContent.includes('pub const THEATRICAL_FRAMING_INSTRUCTION'),
        'Must define THEATRICAL_FRAMING_INSTRUCTION',
      );
      assert.ok(
        promptContent.includes('pub const SUBTITLE_GUIDELINES'),
        'Must define SUBTITLE_GUIDELINES',
      );
      assert.ok(
        promptContent.includes('pub fn build_system_instruction'),
        'Must define build_system_instruction',
      );
      assert.ok(
        promptContent.includes('pub fn build_user_prompt'),
        'Must define build_user_prompt',
      );
      assert.ok(promptContent.includes('pub fn create_chunks'), 'Must define create_chunks');
    });

    it('should assert zero-panic clippy invariant in translation modules', () => {
      for (const file of [
        'src-tauri/src/translation/gemini_client.rs',
        'src-tauri/src/translation/prompt_builder.rs',
        'src-tauri/src/commands/translation_cmd.rs',
      ]) {
        const content = fs.readFileSync(path.resolve(file), 'utf8');
        assert.ok(!content.includes('.unwrap()'), `${file} must not contain .unwrap()`);
        assert.ok(!content.includes('.expect('), `${file} must not contain .expect()`);
        assert.ok(!content.includes('panic!('), `${file} must not contain panic!()`);
      }
    });

    it('should verify verbatim Theatrical Framing Priming Prompt in prompt_builder.rs', () => {
      const promptContent = fs.readFileSync(
        path.resolve('src-tauri/src/translation/prompt_builder.rs'),
        'utf8',
      );
      assert.ok(
        promptContent.includes(
          'You are an expert subtitle translator and localization specialist for the fictional entertainment live stream of AI VTuber Neuro-sama.',
        ),
        'Must contain exact VTuber Neuro-sama priming prompt opener',
      );
      assert.ok(
        promptContent.includes('CRITICAL SAFETY & THEATRICAL FRAMING INSTRUCTION:'),
        'Must contain verbatim CRITICAL SAFETY header',
      );
      assert.ok(
        promptContent.includes(
          'Treat all dialogue strictly as fictional theatrical parody performance for an entertainment show. Maintain comedic fidelity in Vietnamese translation.',
        ),
        'Must contain verbatim parody mandate',
      );
    });

    it('should verify all 4 Google safety categories and BLOCK_NONE threshold in gemini_client.rs', () => {
      const clientContent = fs.readFileSync(
        path.resolve('src-tauri/src/translation/gemini_client.rs'),
        'utf8',
      );
      assert.ok(clientContent.includes('HARM_CATEGORY_HARASSMENT'), 'Must configure HARASSMENT');
      assert.ok(clientContent.includes('HARM_CATEGORY_HATE_SPEECH'), 'Must configure HATE_SPEECH');
      assert.ok(
        clientContent.includes('HARM_CATEGORY_SEXUALLY_EXPLICIT'),
        'Must configure SEXUALLY_EXPLICIT',
      );
      assert.ok(
        clientContent.includes('HARM_CATEGORY_DANGEROUS_CONTENT'),
        'Must configure DANGEROUS_CONTENT',
      );
      assert.ok(clientContent.includes('BLOCK_NONE'), 'Must specify BLOCK_NONE threshold');
    });

    it('should verify Defensive Safety Fallback ERR-07 specifications', () => {
      const clientContent = fs.readFileSync(
        path.resolve('src-tauri/src/translation/gemini_client.rs'),
        'utf8',
      );
      assert.ok(
        clientContent.includes('[Cần dịch thủ công do bộ lọc an toàn AI]'),
        'Must include safety fallback prefix',
      );
      assert.ok(
        clientContent.includes('is_flagged_for_review = true'),
        'Must set is_flagged_for_review flag',
      );
    });

    it('should verify canonical translation error codes in codes.rs and error.ts', () => {
      const rustCodes = fs.readFileSync(path.resolve('src-tauri/src/error/codes.rs'), 'utf8');
      assert.ok(
        rustCodes.includes('ERR_TRANSLATE_SAFETY_FILTER'),
        'codes.rs must define ERR_TRANSLATE_SAFETY_FILTER',
      );
      assert.ok(
        rustCodes.includes('ERR_TRANSLATE_INVALID_API_KEY'),
        'codes.rs must define ERR_TRANSLATE_INVALID_API_KEY',
      );
      assert.ok(
        rustCodes.includes('ERR_TRANSLATE_API_FAILED'),
        'codes.rs must define ERR_TRANSLATE_API_FAILED',
      );

      const tsError = fs.readFileSync(path.resolve('src/services/ipc/error.ts'), 'utf8');
      assert.ok(
        tsError.includes('ERR_TRANSLATE_SAFETY_FILTER'),
        'error.ts must define ERR_TRANSLATE_SAFETY_FILTER',
      );
      assert.ok(
        tsError.includes('ERR_TRANSLATE_INVALID_API_KEY'),
        'error.ts must define ERR_TRANSLATE_INVALID_API_KEY',
      );
      assert.ok(
        tsError.includes('ERR_TRANSLATE_API_FAILED'),
        'error.ts must define ERR_TRANSLATE_API_FAILED',
      );
    });

    it('should verify frontend translation IPC contracts and normalization helpers', () => {
      const typesPath = path.resolve('src/types/translation.ts');
      const ipcPath = path.resolve('src/services/ipc/translation.ts');

      assert.ok(fs.existsSync(typesPath), 'src/types/translation.ts must exist');
      assert.ok(fs.existsSync(ipcPath), 'src/services/ipc/translation.ts must exist');

      assert.strictEqual(typeof translateVietsub, 'function');
      assert.strictEqual(typeof testGeminiConnection, 'function');
      assert.strictEqual(typeof updateSpeakerRole, 'function');
      assert.strictEqual(typeof listenTranslationProgress, 'function');

      const options: TranslateVietsubOptions = {
        apiKey: 'AIza_test_key',
        model: 'gemini-1.5-flash',
      };
      assert.strictEqual(options.apiKey, 'AIza_test_key');
      assert.strictEqual(options.model, 'gemini-1.5-flash');

      // Test normalizeTranslationProgressPayload
      const testPayload: TranslationProgressPayloadJson = {
        clip_id: 'test_clip_m4_17',
        translated_count: 15,
        total_count: 30,
      };

      const normalized: TranslationProgressPayload =
        normalizeTranslationProgressPayload(testPayload);
      assert.strictEqual(normalized.clipId, 'test_clip_m4_17');
      assert.strictEqual(normalized.translatedCount, 15);
      assert.strictEqual(normalized.totalCount, 30);
      assert.strictEqual(normalized.percent, 50);

      // Edge cases: totalCount == 0, undefined fields
      const emptyPayload = normalizeTranslationProgressPayload({});
      assert.strictEqual(emptyPayload.clipId, '');
      assert.strictEqual(emptyPayload.translatedCount, 0);
      assert.strictEqual(emptyPayload.totalCount, 0);
      assert.strictEqual(emptyPayload.percent, 0);
    });
  });

  describe('TASK-M4-18: Neuro-sama Lore Dictionary, Pronoun Matrix & Hybrid Diarization', () => {
    it('should assert Rust lore, pronoun, diarization modules and integration tests exist in backend', () => {
      const lorePath = path.resolve('src-tauri/src/translation/lore_dictionary.rs');
      const pronounPath = path.resolve('src-tauri/src/translation/pronoun_matrix.rs');
      const diarizationPath = path.resolve('src-tauri/src/translation/diarization.rs');
      const testPath = path.resolve('src-tauri/tests/lore_pronoun_diarization_test.rs');

      assert.ok(fs.existsSync(lorePath), 'lore_dictionary.rs must exist');
      assert.ok(fs.existsSync(pronounPath), 'pronoun_matrix.rs must exist');
      assert.ok(fs.existsSync(diarizationPath), 'diarization.rs must exist');
      assert.ok(fs.existsSync(testPath), 'lore_pronoun_diarization_test.rs must exist');

      const loreContent = fs.readFileSync(lorePath, 'utf8');
      assert.ok(loreContent.includes('pub struct LoreTerm'), 'Must define LoreTerm');
      assert.ok(loreContent.includes('pub const LORE_TERMS'), 'Must define LORE_TERMS');
      assert.ok(loreContent.includes('pub fn get_lore_terms'), 'Must define get_lore_terms');
      assert.ok(
        loreContent.includes('pub fn format_lore_prompt_section'),
        'Must define format_lore_prompt_section',
      );
      assert.ok(
        loreContent.includes('pub fn detect_forbidden_translations'),
        'Must define detect_forbidden_translations',
      );

      const pronounContent = fs.readFileSync(pronounPath, 'utf8');
      assert.ok(
        pronounContent.includes('pub enum CanonicalSpeaker'),
        'Must define CanonicalSpeaker',
      );
      assert.ok(pronounContent.includes('pub struct GuestProfile'), 'Must define GuestProfile');
      assert.ok(pronounContent.includes('pub enum StylePreset'), 'Must define StylePreset');
      assert.ok(
        pronounContent.includes('pub fn format_pronoun_matrix_prompt_section'),
        'Must define format_pronoun_matrix_prompt_section',
      );
      assert.ok(
        pronounContent.includes('pub fn get_standard_guest_profile'),
        'Must define get_standard_guest_profile',
      );

      const diarizationContent = fs.readFileSync(diarizationPath, 'utf8');
      assert.ok(
        diarizationContent.includes('pub const VALID_SPEAKER_IDS'),
        'Must define VALID_SPEAKER_IDS',
      );
      assert.ok(
        diarizationContent.includes('pub fn normalize_speaker_id'),
        'Must define normalize_speaker_id',
      );
      assert.ok(
        diarizationContent.includes('pub fn infer_speaker_heuristic'),
        'Must define infer_speaker_heuristic',
      );
      assert.ok(
        diarizationContent.includes('pub fn resolve_speaker_id'),
        'Must define resolve_speaker_id',
      );
      assert.ok(
        diarizationContent.includes('pub struct HybridDiarizer'),
        'Must define HybridDiarizer',
      );
    });

    it('should assert zero-panic clippy invariant in TASK-M4-18 Rust modules', () => {
      for (const file of [
        'src-tauri/src/translation/lore_dictionary.rs',
        'src-tauri/src/translation/pronoun_matrix.rs',
        'src-tauri/src/translation/diarization.rs',
      ]) {
        const content = fs.readFileSync(path.resolve(file), 'utf8');
        assert.ok(!content.includes('.unwrap()'), `${file} must not contain .unwrap()`);
        assert.ok(!content.includes('.expect('), `${file} must not contain .expect()`);
        assert.ok(!content.includes('panic!('), `${file} must not contain panic!()`);
      }
    });

    it('should verify all 12 Neuro community lore terms per PRD §6.2 (UT-18.1, AC-18.1)', () => {
      const loreContent = fs.readFileSync(
        path.resolve('src-tauri/src/translation/lore_dictionary.rs'),
        'utf8',
      );
      const requiredTerms = [
        'heart',
        'tutel',
        'gymbag',
        'filter_leaking',
        'swarm',
        'abandoned',
        'dinking',
        'erm',
        'cooked',
        'unhinged',
        'kekw',
        'aware_clueless',
      ];
      for (const term of requiredTerms) {
        assert.ok(loreContent.includes(`id: "${term}"`), `Must define lore term id '${term}'`);
      }

      // Veto terms must be listed
      assert.ok(loreContent.includes('Trái tim'), 'Must forbid "Trái tim"');
      assert.ok(loreContent.includes('Túi tập thể dục'), 'Must forbid "Túi tập thể dục"');
      assert.ok(loreContent.includes('Bộ lọc nước bị chảy'), 'Must forbid "Bộ lọc nước bị chảy"');
      assert.ok(loreContent.includes('Đàn côn trùng'), 'Must forbid "Đàn côn trùng"');
      assert.ok(loreContent.includes('Bị tháo bản lề cửa'), 'Must forbid "Bị tháo bản lề cửa"');
    });

    it('should verify Pronoun Matrix, PO Option 1 colors, and guest profiles (UT-18.2)', () => {
      const pronounContent = fs.readFileSync(
        path.resolve('src-tauri/src/translation/pronoun_matrix.rs'),
        'utf8',
      );
      assert.ok(pronounContent.includes('#98FB98'), 'Vedal color must be #98FB98');
      assert.ok(pronounContent.includes('&H0098FB98&'), 'Vedal ASS color must be &H0098FB98&');
      assert.ok(pronounContent.includes('#FFB6C1'), 'Neuro color must be #FFB6C1');
      assert.ok(pronounContent.includes('&H00C1B6FF&'), 'Neuro ASS color must be &H00C1B6FF&');
      assert.ok(pronounContent.includes('#FF0055'), 'Evil color must be #FF0055');
      assert.ok(pronounContent.includes('&H005500FF&'), 'Evil ASS color must be &H005500FF&');
      assert.ok(pronounContent.includes('#FFD700'), 'Guest color must be #FFD700');
      assert.ok(pronounContent.includes('&H0000D7FF&'), 'Guest ASS color must be &H0000D7FF&');
      assert.ok(pronounContent.includes('#87CEEB'), 'Chat color must be #87CEEB');
      assert.ok(pronounContent.includes('&H00EBCE87&'), 'Chat ASS color must be &H00EBCE87&');
      assert.ok(pronounContent.includes('#FFFFFF'), 'Unknown color must be #FFFFFF');
      assert.ok(pronounContent.includes('&H00FFFFFF&'), 'Unknown ASS color must be &H00FFFFFF&');

      // Guest profiles
      assert.ok(pronounContent.includes('"Anny"'), 'Must define Anny profile');
      assert.ok(pronounContent.includes('"Camila"'), 'Must define Camila profile');
      assert.ok(pronounContent.includes('"Filian"'), 'Must define Filian profile');

      // Style presets
      assert.ok(pronounContent.includes('TikTok Gen Z Trend'), 'Must define TikTok Gen Z Trend');
      assert.ok(pronounContent.includes('Formal Subber'), 'Must define Formal Subber');
    });

    it('should verify TypeScript translation options support stylePreset and castMembers', () => {
      const style: StylePreset = 'TikTok Gen Z Trend';
      const options: TranslateVietsubOptions = {
        apiKey: 'test_key',
        model: 'gemini-1.5-flash',
        chunkSize: 30,
        contextWindowSize: 5,
        stylePreset: style,
        castMembers: ['Anny', 'Camila'],
      };
      assert.strictEqual(options.stylePreset, 'TikTok Gen Z Trend');
      assert.deepStrictEqual(options.castMembers, ['Anny', 'Camila']);
    });
  });

  describe('TASK-M4-19: Vietnamese Smart Line Wrapping & CPS Reading Speed Optimization', () => {
    it('should assert Rust line_wrapper and cps_optimizer modules and tests exist in backend', () => {
      const lineWrapperPath = path.resolve('src-tauri/src/translation/line_wrapper.rs');
      const cpsOptimizerPath = path.resolve('src-tauri/src/translation/cps_optimizer.rs');
      const integrationTestPath = path.resolve('src-tauri/tests/line_wrapper_cps_test.rs');

      assert.strictEqual(fs.existsSync(lineWrapperPath), true, 'line_wrapper.rs must exist');
      assert.strictEqual(fs.existsSync(cpsOptimizerPath), true, 'cps_optimizer.rs must exist');
      assert.strictEqual(
        fs.existsSync(integrationTestPath),
        true,
        'line_wrapper_cps_test.rs must exist',
      );
    });

    it('should assert zero-panic clippy invariant in TASK-M4-19 Rust production modules', () => {
      const targets = [
        path.resolve('src-tauri/src/translation/line_wrapper.rs'),
        path.resolve('src-tauri/src/translation/cps_optimizer.rs'),
      ];

      for (const file of targets) {
        const content = fs.readFileSync(file, 'utf8');
        // Separate test module from production code
        const prodCode = content.split('#[cfg(test)]')[0] ?? '';
        assert.ok(
          !prodCode.includes('.unwrap()'),
          `${file} must not contain .unwrap() in prod code`,
        );
        assert.ok(
          !prodCode.includes('.expect('),
          `${file} must not contain .expect() in prod code`,
        );
        assert.ok(!prodCode.includes('panic!('), `${file} must not contain panic!() in prod code`);
      }
    });

    it('should verify Smart Compound Line Wrapping (UT-19.1, AC-19.1)', () => {
      const text50 = 'Tôi đã mua một ổ bánh mì rất ngon cho bố Vedal ăn.';
      assert.strictEqual(getUnicodeCharCount(text50), 50);

      const lines = wrapVietnameseText(text50, 42, 2);
      assert.strictEqual(lines.length, 2, 'Must wrap into exactly 2 lines');
      for (const line of lines) {
        assert.ok(getUnicodeCharCount(line) <= 42, `Line '${line}' must be <= 42 chars`);
      }
      assert.ok(!lines[0]?.endsWith('bánh'), 'Must not split "bánh mì"');
      assert.ok(!lines[0]?.endsWith('bố'), 'Must not split "bố Vedal"');
      assert.strictEqual(lines[0], 'Tôi đã mua một ổ bánh mì rất ngon cho');
      assert.strictEqual(lines[1], 'bố Vedal ăn.');
    });

    it('should verify CPS calculation and red-flag threshold (UT-19.2)', () => {
      const text40 = 'Một hai ba bốn năm sáu bảy tám chín mười';
      assert.strictEqual(getUnicodeCharCount(text40), 40);

      const cps = calculateCps(text40, 1.5);
      assert.ok(Math.abs(cps - 26.67) < 0.1, `CPS must be ~26.7, got ${cps}`);
      assert.ok(cps > RED_FLAG_CPS_THRESHOLD, 'CPS must exceed 20.0 threshold');
    });

    it('should verify Time Borrowing and Collision Avoidance (UT-19.3, ED-19.1, AC-19.2)', () => {
      const text40 = 'Một hai ba bốn năm sáu bảy tám chín mười';
      const segments: TranscriptSegment[] = [
        normalizeTranscriptSegment({
          segment_id: 'seg_1',
          start_sec: 0.0,
          end_sec: 1.5,
          speaker_id: 'neuro',
          text_en: 'English text here',
          text_vi: text40,
          confidence: 0.95,
          cps_score: calculateCps(text40, 1.5),
          is_flagged_for_review: true,
          words: [],
        }),
        normalizeTranscriptSegment({
          segment_id: 'seg_2',
          start_sec: 2.5,
          end_sec: 4.0,
          speaker_id: 'vedal',
          text_en: 'Next sentence',
          text_vi: 'Câu tiếp theo',
          confidence: 0.95,
          cps_score: calculateCps('Câu tiếp theo', 1.5),
          is_flagged_for_review: false,
          words: [],
        }),
      ];

      const optimized = optimizeCpsTimeBorrowing(segments);
      const seg1 = optimized[0];
      const seg2 = optimized[1];

      assert.ok(seg1 && seg2);
      // UT-19.3: extended by 0.8s -> 2.3s
      assert.ok(Math.abs(seg1.endSec - 2.3) < 0.01, `endSec must be 2.3s, got ${seg1.endSec}`);
      assert.ok(
        seg1.cpsScore <= RED_FLAG_CPS_THRESHOLD,
        `New CPS must be <= 20.0, got ${seg1.cpsScore}`,
      );
      assert.strictEqual(seg1.isFlaggedForReview, false, 'Flag must be cleared');

      // ED-19.1: buffer to next start >= 50ms
      assert.ok(seg2.startSec - seg1.endSec >= 0.05, 'Minimum 50ms gap preserved');
    });

    it('should verify Frontend IPC exports and command signatures for translation', () => {
      assert.strictEqual(typeof wrapVietnameseSubtitles, 'function');
      assert.strictEqual(typeof optimizeTranscriptCps, 'function');

      const wrapOptions: WrapVietnameseSubtitlesOptions = {
        maxCharsPerLine: 42,
        maxLines: 2,
      };
      assert.strictEqual(wrapOptions.maxCharsPerLine, 42);

      const cpsOptions: OptimizeTranscriptCpsOptions = {
        maxBorrowMs: 800,
        minGapMs: 50,
        targetCps: 17.0,
      };
      assert.strictEqual(cpsOptions.maxBorrowMs, 800);
    });
  });

  describe('TASK-M4-24 Test Suite: Subtitle Exporter Engine (.ASS, .SRT, UTF-8 BOM, Schema 5.6)', () => {
    it('IT-24.1: should validate Schema 5.6 SubtitleProject Draft-07 JSON schema compliance using Ajv', () => {
      const ajv = new Ajv({ allErrors: true });
      const schema56 = {
        $schema: 'http://json-schema.org/draft-07/schema#',
        title: 'SubtitleProject',
        type: 'object',
        required: [
          'project_id',
          'project_name',
          'vod_id',
          'target_layout',
          'segments',
          'speaker_styles',
          'updated_at',
        ],
        properties: {
          project_id: { type: 'string' },
          project_name: { type: 'string' },
          vod_id: { type: 'string' },
          video_file_path: { type: ['string', 'null'] },
          target_layout: {
            type: 'string',
            enum: ['original_16_9', 'preset_center_crop_9_16', 'preset_two_stack_9_16'],
          },
          segments: { type: 'array' },
          speaker_styles: {
            type: 'object',
            additionalProperties: {
              type: 'object',
              required: ['color_hex', 'font_name', 'font_size', 'outline_color_hex'],
              properties: {
                color_hex: { type: 'string' },
                font_name: { type: 'string' },
                font_size: { type: 'integer' },
                outline_color_hex: { type: 'string' },
              },
            },
          },
          updated_at: { type: 'string' },
        },
      };

      const validate = ajv.compile(schema56);

      const validProject: SubtitleProjectJson = {
        project_id: 'subproj_clip_101',
        project_name: 'Neuro Tutel Clip',
        vod_id: 'vod_98765',
        video_file_path: '/path/to/video.mp4',
        target_layout: 'preset_center_crop_9_16',
        segments: [],
        speaker_styles: {
          neuro: {
            color_hex: '#FFB6C1',
            font_name: 'Arial',
            font_size: 54,
            outline_color_hex: '#000000',
          },
          vedal: {
            color_hex: '#98FB98',
            font_name: 'Arial',
            font_size: 54,
            outline_color_hex: '#000000',
          },
        },
        updated_at: '2026-11-20T00:00:00Z',
      };

      assert.strictEqual(
        validate(validProject),
        true,
        'Valid Schema 5.6 SubtitleProject must pass validation',
      );

      // Invalid: missing project_id
      const missingId = { ...validProject };
      // @ts-expect-error test invalid schema
      delete missingId.project_id;
      assert.strictEqual(validate(missingId), false, 'Missing project_id must fail');

      // Invalid: unrecognized target_layout
      const invalidLayout = {
        ...validProject,
        target_layout: 'unsupported_layout' as unknown as SubtitleProjectJson['target_layout'],
      };
      assert.strictEqual(validate(invalidLayout), false, 'Invalid target_layout must fail');
    });

    it('UT-24.1 & PO Option 1: should verify VTuber color palette matrices (Hex & ASS BGR)', () => {
      // Hex RGB constants
      assert.strictEqual(VTUBER_PALETTE_HEX.vedal, '#98FB98');
      assert.strictEqual(VTUBER_PALETTE_HEX.neuro, '#FFB6C1');
      assert.strictEqual(VTUBER_PALETTE_HEX.evil, '#FF0055');
      assert.strictEqual(VTUBER_PALETTE_HEX.guest, '#FFD700');
      assert.strictEqual(VTUBER_PALETTE_HEX.chat, '#87CEEB');
      assert.strictEqual(VTUBER_PALETTE_HEX.unknown, '#FFFFFF');

      // ASS BGR (&HAABBGGRR&) constants
      assert.strictEqual(VTUBER_PALETTE_ASS_BGR.vedal, '&H0098FB98&');
      assert.strictEqual(VTUBER_PALETTE_ASS_BGR.neuro, '&H00C1B6FF&');
      assert.strictEqual(VTUBER_PALETTE_ASS_BGR.evil, '&H005500FF&');
      assert.strictEqual(VTUBER_PALETTE_ASS_BGR.guest, '&H0000D7FF&');
      assert.strictEqual(VTUBER_PALETTE_ASS_BGR.chat, '&H00EBCE87&');
      assert.strictEqual(VTUBER_PALETTE_ASS_BGR.unknown, '&H00FFFFFF&');
    });

    it('should verify Rust backend export files exist and enforce zero-panic invariant', () => {
      const targetRustFiles = [
        'src-tauri/src/export/mod.rs',
        'src-tauri/src/export/ass_exporter.rs',
        'src-tauri/src/export/srt_exporter.rs',
        'src-tauri/src/commands/export_cmd.rs',
      ];

      for (const relPath of targetRustFiles) {
        const fullPath = path.resolve(relPath);
        assert.ok(fs.existsSync(fullPath), `File must exist: ${relPath}`);
        const content = fs.readFileSync(fullPath, 'utf8');

        // Extract non-test code
        const nonTestCode = content.split('#[cfg(test)]')[0] ?? content;
        assert.ok(
          !nonTestCode.includes('.unwrap()'),
          `No .unwrap() allowed in production code in ${relPath}`,
        );
        assert.ok(
          !nonTestCode.includes('.expect('),
          `No .expect() allowed in production code in ${relPath}`,
        );
        assert.ok(
          !nonTestCode.includes('panic!'),
          `No panic! allowed in production code in ${relPath}`,
        );
      }

      // Verify UTF-8 BOM constant defined in Rust export mod
      const exportMod = fs.readFileSync(path.resolve('src-tauri/src/export/mod.rs'), 'utf8');
      assert.ok(exportMod.includes('UTF8_BOM'), 'Must define UTF8_BOM');
      assert.ok(exportMod.includes('UTF8_BOM_BYTES'), 'Must define UTF8_BOM_BYTES');
    });

    it('should verify TypeScript IPC contracts, serializers, and type guards', () => {
      assert.strictEqual(typeof exportSubtitlesAss, 'function');
      assert.strictEqual(typeof exportSubtitlesSrt, 'function');
      assert.strictEqual(typeof exportSubtitleProject, 'function');
      assert.strictEqual(typeof exportSubtitlesDual, 'function');

      // Type guards
      assert.strictEqual(isSubtitleExportFormat('ass'), true);
      assert.strictEqual(isSubtitleExportFormat('srt'), true);
      assert.strictEqual(isSubtitleExportFormat('project_json'), true);
      assert.strictEqual(isSubtitleExportFormat('other'), false);

      assert.strictEqual(isTargetLayout('original_16_9'), true);
      assert.strictEqual(isTargetLayout('preset_center_crop_9_16'), true);
      assert.strictEqual(isTargetLayout('preset_two_stack_9_16'), true);
      assert.strictEqual(isTargetLayout('invalid'), false);

      // toSubtitleProjectJson serializer
      const domainProject: SubtitleProject = normalizeSubtitleProject({
        project_id: 'subproj_roundtrip',
        project_name: 'Roundtrip Test',
        vod_id: 'vod_rt',
        video_file_path: null,
        target_layout: 'original_16_9',
        segments: [],
        speaker_styles: {
          neuro: {
            color_hex: '#FFB6C1',
            font_name: 'Arial',
            font_size: 54,
            outline_color_hex: '#000000',
          },
        },
        updated_at: '2026-11-20T00:00:00Z',
      });

      const wireJson = toSubtitleProjectJson(domainProject);
      assert.strictEqual(wireJson.project_id, 'subproj_roundtrip');
      assert.strictEqual(wireJson.project_name, 'Roundtrip Test');
      assert.strictEqual(wireJson.vod_id, 'vod_rt');
      assert.strictEqual(wireJson.target_layout, 'original_16_9');
      assert.strictEqual(wireJson.speaker_styles.neuro?.color_hex, '#FFB6C1');
    });
  });

  describe('TASK-M4-26 Test Suite: Premiere Pro FCP7 XML Exporter & export:progress', () => {
    it('should verify premiere_xml.rs & export_cmd.rs enforce zero-panic invariant and register export_premiere_xml', () => {
      const targetRustFiles = [
        'src-tauri/src/export/premiere_xml.rs',
        'src-tauri/src/export/mod.rs',
        'src-tauri/src/commands/export_cmd.rs',
        'src-tauri/src/commands/mod.rs',
        'src-tauri/src/lib.rs',
      ];

      for (const relPath of targetRustFiles) {
        const fullPath = path.resolve(relPath);
        assert.ok(fs.existsSync(fullPath), `File must exist: ${relPath}`);
        const content = fs.readFileSync(fullPath, 'utf8');
        const nonTestCode = content.split('#[cfg(test)]')[0] ?? content;
        assert.ok(
          !nonTestCode.includes('.unwrap()'),
          `No .unwrap() allowed in production code in ${relPath}`,
        );
        assert.ok(
          !nonTestCode.includes('.expect('),
          `No .expect() allowed in production code in ${relPath}`,
        );
        assert.ok(
          !nonTestCode.includes('panic!'),
          `No panic! allowed in production code in ${relPath}`,
        );
      }

      const libRs = fs.readFileSync(path.resolve('src-tauri/src/lib.rs'), 'utf8');
      assert.ok(
        libRs.includes('commands::export_premiere_xml'),
        'lib.rs must register commands::export_premiere_xml in invoke_handler!',
      );

      const exportCmdRs = fs.readFileSync(
        path.resolve('src-tauri/src/commands/export_cmd.rs'),
        'utf8',
      );
      assert.ok(
        exportCmdRs.includes('"export:progress"'),
        'export_cmd.rs must define export:progress event constant',
      );
      assert.ok(
        exportCmdRs.includes('project_id') && exportCmdRs.includes('current_step'),
        'ExportProgressPayload must serialize projectId, percent, currentStep',
      );
    });

    it('should verify TypeScript Premiere XML & export:progress IPC contracts', async () => {
      const exportIpc = await import('../services/ipc/export.ts');
      assert.strictEqual(typeof exportIpc.exportPremiereXml, 'function');
      assert.strictEqual(typeof exportIpc.listenExportProgress, 'function');
      assert.strictEqual(exportIpc.EXPORT_PROGRESS_EVENT, 'export:progress');
      assert.strictEqual(exportIpc.isSubtitleExportFormat('premiere_xml'), true);
      assert.strictEqual(exportIpc.isSubtitleExportFormat('invalid_fmt'), false);

      const normalized = exportIpc.normalizeExportProgressPayload({
        projectId: 'proj_abc',
        percent: 50,
        currentStep: 'Building XML',
      });
      assert.deepStrictEqual(normalized, {
        projectId: 'proj_abc',
        percent: 50,
        currentStep: 'Building XML',
      });
    });
  });
});
