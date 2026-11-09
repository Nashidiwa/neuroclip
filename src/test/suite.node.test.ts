import { describe, it } from 'node:test';
import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';
import { loadTheme, resolveBackdropConfig, THEME_PALETTE, WINDOW_GEOMETRY } from '../styles/theme.ts';
import { ERROR_CODES, ERROR_MESSAGES_VI, isIpcError, parseIpcError } from '../services/ipc/error.ts';

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

