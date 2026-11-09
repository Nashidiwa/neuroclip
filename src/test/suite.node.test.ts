import { describe, it } from 'node:test';
import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';
import { loadTheme, resolveBackdropConfig, THEME_PALETTE, WINDOW_GEOMETRY } from '../styles/theme.ts';

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
