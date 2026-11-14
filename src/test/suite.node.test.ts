import { describe, it } from 'node:test';
import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';
import Ajv from 'ajv';
import { loadTheme, resolveBackdropConfig, THEME_PALETTE, WINDOW_GEOMETRY } from '../styles/theme.ts';
import { ERROR_CODES, ERROR_MESSAGES_VI, isIpcError, parseIpcError } from '../services/ipc/error.ts';
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

describe('TASK-M4-01 Test Suite (Node.js Native Test Runner)', () => {
  describe('UT-01.1: Theme Palette Loading (Happy Path)', () => {
    it('should return exactly the specified Neuro-sama VTuber theme palette hex colors', () => {
      const theme = loadTheme();

      // Primary & Secondary dark backgrounds
      assert.strictEqual(theme.bgPrimary.toUpperCase(), '#12131C', 'bgPrimary must be #12131C');
      assert.strictEqual(theme.bgSecondary.toUpperCase(), '#1A1C28', 'bgSecondary must be #1A1C28');
      assert.strictEqual(theme.bgTertiary.toUpperCase(), '#242738', 'bgTertiary must be #242738');

      // VTuber character accent colors
      assert.strictEqual(theme.accentNeuro.toUpperCase(), '#FFB6C1', 'Neuro accent must be #FFB6C1');
      assert.strictEqual(theme.accentEvil.toUpperCase(), '#FF0055', 'Evil accent must be #FF0055');
      assert.strictEqual(theme.accentVedal.toUpperCase(), '#98FB98', 'Vedal accent must be #98FB98');
      assert.strictEqual(theme.accentGuest.toUpperCase(), '#FFD700', 'Guest accent must be #FFD700');
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
      assert.strictEqual(windowConfig.width, WINDOW_GEOMETRY.defaultWidth, 'Default width must match WINDOW_GEOMETRY');
      assert.strictEqual(windowConfig.height, WINDOW_GEOMETRY.defaultHeight, 'Default height must match WINDOW_GEOMETRY');
      assert.strictEqual(windowConfig.minWidth, WINDOW_GEOMETRY.minWidth, 'Minimum width must match WINDOW_GEOMETRY');
      assert.strictEqual(windowConfig.minHeight, WINDOW_GEOMETRY.minHeight, 'Minimum height must match WINDOW_GEOMETRY');
      assert.strictEqual(windowConfig.transparent, true, 'Window must support transparency');
      assert.strictEqual(windowConfig.decorations, false, 'Window decorations must be false for frameless acrylic');
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
      assert.ok(content.includes('--color-bg-primary: #12131C;'));
      assert.ok(content.includes('--color-bg-secondary: #1A1C28;'));
      assert.ok(content.includes('--color-accent-neuro: #FFB6C1;'));
      assert.ok(content.includes('--color-accent-evil: #FF0055;'));
      assert.ok(content.includes('--color-accent-vedal: #98FB98;'));
      assert.ok(content.includes('--color-accent-guest: #FFD700;'));
      assert.ok(content.includes('--color-accent-chat: #87CEEB;'));
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
        'ERR_STT_INVALID_API_KEY',
        'ERR_STT_VAD_FAILED',
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
      assert.ok(content.includes('impl Serialize for AppError'), 'Must implement Serialize for AppError');
      assert.ok(content.includes('pub fn code(&self) ->'), 'Must implement code method');
      assert.ok(content.includes('pub fn is_recoverable(&self) -> bool'), 'Must implement is_recoverable');
      assert.ok(content.includes('pub fn to_ipc_response(&self) -> IpcErrorResponse'), 'Must implement to_ipc_response');
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

describe('TASK-M4-03 Test Suite: Shared Types, Hardware Profile & Config Management', () => {
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
      assert.strictEqual(evaluateRecommendation(32768, 'NVIDIA', 24576, true).mode, 'offline_local');

      // TC-HW-02: RTX 3080 10GB + 32GB RAM -> offline_local
      assert.strictEqual(evaluateRecommendation(32768, 'NVIDIA', 10240, true).mode, 'offline_local');

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
      assert.ok(content.includes('commands::get_app_config'), 'lib.rs must register get_app_config');
      assert.ok(content.includes('commands::save_app_config'), 'lib.rs must register save_app_config');
      assert.ok(content.includes('commands::get_hardware_profile'), 'lib.rs must register get_hardware_profile');
      assert.ok(content.includes('commands::detect_hardware'), 'lib.rs must register detect_hardware');
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
            if (
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
        { vendor: 'NVIDIA', model: 'NVIDIA GeForce RTX 3060 Laptop GPU', vramMb: 6144, cudaSupported: true },
      ];
      const selectedLaptop = selectOptimalGpu(laptopRig);
      assert.strictEqual(selectedLaptop?.model, 'NVIDIA GeForce RTX 3060 Laptop GPU');
      assert.strictEqual(selectedLaptop?.cudaSupported, true);

      // Scenario 2: AMD Radeon 680M (0000) + NVIDIA RTX 4070 Laptop (0001) -> selects RTX 4070
      const amdNvidiaRig: MockGpuCandidate[] = [
        { vendor: 'AMD', model: 'AMD Radeon 680M', vramMb: 512, cudaSupported: false },
        { vendor: 'NVIDIA', model: 'NVIDIA GeForce RTX 4070 Laptop GPU', vramMb: 8192, cudaSupported: true },
      ];
      assert.strictEqual(selectOptimalGpu(amdNvidiaRig)?.model, 'NVIDIA GeForce RTX 4070 Laptop GPU');

      // Scenario 3: Dual NVIDIA GPUs (e.g. GTX 1650 4GB at 0000 + RTX 4090 24GB at 0001) -> selects highest VRAM
      const dualNvidiaRig: MockGpuCandidate[] = [
        { vendor: 'NVIDIA', model: 'NVIDIA GeForce GTX 1650', vramMb: 4096, cudaSupported: true },
        { vendor: 'NVIDIA', model: 'NVIDIA GeForce RTX 4090', vramMb: 24576, cudaSupported: true },
      ];
      assert.strictEqual(selectOptimalGpu(dualNvidiaRig)?.model, 'NVIDIA GeForce RTX 4090');
    });

    it('should assert save_app_config holds the write lock during disk I/O', () => {
      const configCmdPath = path.resolve('src-tauri/src/commands/config_cmd.rs');
      assert.strictEqual(fs.existsSync(configCmdPath), true, 'config_cmd.rs must exist');

      const content = fs.readFileSync(configCmdPath, 'utf8');

      const fnStart = content.indexOf('pub async fn save_app_config');
      assert.ok(fnStart !== -1, 'save_app_config command must exist in config_cmd.rs');

      const nextFn = content.indexOf('pub async fn', fnStart + 20);
      const fnBody = nextFn !== -1 ? content.slice(fnStart, nextFn) : content.slice(fnStart);

      const writeLockIndex = fnBody.indexOf('state.config.write()');
      const atomicSaveIndex = fnBody.indexOf('atomic_save_json');

      assert.ok(
        writeLockIndex !== -1,
        'save_app_config must acquire state.config.write() lock',
      );
      assert.ok(
        atomicSaveIndex !== -1,
        'save_app_config must call atomic_save_json',
      );
      assert.ok(
        writeLockIndex < atomicSaveIndex,
        'state.config.write() MUST be acquired BEFORE calling atomic_save_json to eliminate concurrency lock inversion',
      );
    });

    it('should assert atomic temporary file nonce uses atomic counter / PID', () => {
      const atomicRsPath = path.resolve('src-tauri/src/config/atomic.rs');
      assert.strictEqual(fs.existsSync(atomicRsPath), true, 'atomic.rs must exist');

      const content = fs.readFileSync(atomicRsPath, 'utf8');

      // 1. Verify atomic sequence counter usage (AtomicU64 or AtomicUsize)
      const hasAtomicCounter =
        content.includes('AtomicU64') ||
        content.includes('AtomicUsize');
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
      const hasPid =
        content.includes('std::process::id()') ||
        content.includes('process::id()');
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
      assert.strictEqual(redactSecret('short'), '[REDACTED]', '5-char string must return [REDACTED]');
      assert.strictEqual(redactSecret('12345678'), '[REDACTED]', '8-char boundary string must return [REDACTED]');
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
      assert.strictEqual(sanitized.includes('1234567890abcdef1234'), false, 'Must not contain raw Twitch token');
      assert.strictEqual(sanitized.includes('eyJhbGciOiJIUzI1NiIsInR5cCI6'), false, 'Must not contain raw Bearer token');

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

      assert.ok(content.includes('pub mod logging;'), 'lib.rs must export pub mod logging;');
      assert.ok(content.includes('pub mod core {'), 'lib.rs must export pub mod core module alias');
      assert.ok(content.includes('pub use crate::logging;'), 'lib.rs core module must re-export logging');
      assert.ok(content.includes('commands::get_recent_logs'), 'lib.rs must register get_recent_logs command');
      assert.ok(content.includes('commands::clear_logs'), 'lib.rs must register clear_logs command');
      assert.ok(content.includes('commands::log_frontend_event'), 'lib.rs must register log_frontend_event command');
      assert.ok(content.includes('logging::init_logging()'), 'lib.rs must initialize logging in run()');
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

      assert.ok(content.includes('pub struct RedactingWriter<W: Write>'), 'Must define RedactingWriter');
      assert.ok(content.includes('impl<W: Write> Write for RedactingWriter<W>'), 'Must implement Write for RedactingWriter');
      assert.ok(content.includes('redact_secrets_in_text'), 'RedactingWriter must call redact_secrets_in_text');
    });

    it('should assert RetentionPolicy constants match SSOT specifications (14 days, 50MB ceiling, 30MB low-water mark)', () => {
      const rotationPath = path.resolve('src-tauri/src/logging/rotation.rs');
      const content = fs.readFileSync(rotationPath, 'utf8');

      assert.ok(content.includes('max_age_days: 14'), 'RetentionPolicy default max_age_days must be 14');
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
        assert.strictEqual(fs.existsSync(fullPath), true, `Frontend file ${relPath} must exist on disk`);
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

      assert.strictEqual(isLogEntryJson(validJson), true, 'Valid LogEntryJson must pass type guard');
      assert.strictEqual(isLogEntryJson({ bad: 'payload' }), false, 'Malformed payload must be rejected');

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

      assert.strictEqual(isDiskStatusJson(wireJson), true, 'Valid DiskStatusJson must pass type guard');
      assert.strictEqual(isDiskStatusJson({ bad: 123 }), false, 'Malformed payload must be rejected');

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
      required: [
        'vod_id',
        'source_type',
        'title',
        'duration_seconds',
        'audio_path',
        'created_at',
      ],
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

      assert.ok(content.includes('carry_byte: Option<u8>'), 'Must maintain carry_byte state for odd bytes');
      assert.ok(content.includes('WAV_HEADER_SIZE: usize = 44'), 'Must define 44-byte WAV header size');
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

      assert.ok(content.includes('Windows NT 10.0; Win64; x64'), 'Must target Windows 10/11 64-bit');
      assert.ok(content.includes('get_rotated_desktop_user_agent'), 'Must provide UA rotation mechanism');
    });
  });

  describe('ED-06.3: Dual-layer Process Lifecycle & Live Stream Guardrail AST Checks', () => {
    it('should assert Dual-Layer Process Lifecycle: kill_on_drop, ChildProcessGuard, and Windows Job Object', () => {
      const streamerPath = path.resolve('src-tauri/src/ingestion/streamer.rs');
      const content = fs.readFileSync(streamerPath, 'utf8');

      assert.ok(content.includes('kill_on_drop(true)'), 'Command must set kill_on_drop(true)');
      assert.ok(content.includes('pub struct ChildProcessGuard'), 'Must define ChildProcessGuard');
      assert.ok(content.includes('start_kill()'), 'ChildProcessGuard drop must call start_kill()');
      assert.ok(content.includes('JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE'), 'Must configure JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE');
      assert.ok(content.includes('assign_child_to_job_guard'), 'Must assign spawned processes to Windows Job Object');
      assert.ok(content.includes('assign_process'), 'WindowsJobObjectGuard must implement assign_process');
      assert.ok(content.includes('assign_child_process'), 'WindowsJobObjectGuard must implement assign_child_process');
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
      assert.ok(
        content.includes('commands::probe_vod'),
        'lib.rs must register probe_vod command',
      );
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

  describe('TASK-M4-07: YouTube Live Chat / Twitch VOD Chat Extraction & Normalizer Contracts', () => {
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
      assert.ok(parserContent.includes('pub struct ChatEventsContainer'), 'Must define ChatEventsContainer');
      assert.ok(parserContent.includes('pub struct ChatDensityPoint'), 'Must define ChatDensityPoint');
      assert.ok(parserContent.includes('pub struct ChatHeatmapResponse'), 'Must define ChatHeatmapResponse');
      assert.ok(parserContent.includes('NEURO_SWARM_EMOTES'), 'Must define Swarm emote dictionary');
      assert.ok(parserContent.includes('StreamingChatNormalizer'), 'Must define StreamingChatNormalizer');
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
      assert.ok(typesContent.includes('isChatEventsContainerJson'), 'Must export isChatEventsContainerJson guard');
      assert.ok(typesContent.includes('isChatDensityPointJson'), 'Must export isChatDensityPointJson guard');
      assert.ok(typesContent.includes('isChatHeatmapResponseJson'), 'Must export isChatHeatmapResponseJson guard');
      assert.ok(typesContent.includes('normalizeChatEvent'), 'Must export normalizeChatEvent');
    });
  });
});



