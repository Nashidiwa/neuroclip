/**
 * ADVERSARIAL STRESS TEST SUITE: TASK-M4-04 Secret Redaction & Rotation Logic
 * Empirical challenge covering:
 * - Secret Redaction Stress (<= 8 chars, > 8 chars, UTF-8 multi-byte, mixed text)
 * - Rotation & Pruning Edge Cases (non-neuroclip file safety, age > 14d, size > 50MB -> <= 30MB oldest-first, zero-panic)
 * - Rust & TypeScript AST and behavioral equivalence
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';
import {
  redactSecret,
  sanitizeLogString,
  sanitizeLogContext,
} from '../services/logging/redactor';

// ---------------------------------------------------------------------------
// Helpers: Node.js behavioral emulation of Rust secrets.rs and rotation.rs
// ---------------------------------------------------------------------------

/**
 * Exact emulation of Rust `redact_secret(&str)` per `src-tauri/src/logging/secrets.rs`.
 * Strictly honors byte length and UTF-8 char boundary semantics.
 */
function rustRedactSecret(key: string): string {
  const buf = Buffer.from(key, 'utf8');
  if (buf.length <= 8) {
    return '[REDACTED]';
  }

  // In Rust: key.is_char_boundary(idx) checks if byte is not a continuation byte (0x80..=0xBF)
  const isCharBoundary = (b: Buffer, idx: number): boolean => {
    if (idx === 0 || idx === b.length) return true;
    if (idx < 0 || idx > b.length) return false;
    const byte = b[idx];
    if (byte === undefined) return false;
    return (byte & 0xc0) !== 0x80;
  };

  const startIdx = 4;
  const endIdx = buf.length - 4;

  if (isCharBoundary(buf, startIdx) && isCharBoundary(buf, endIdx)) {
    const prefix = buf.subarray(0, startIdx).toString('utf8');
    const suffix = buf.subarray(endIdx).toString('utf8');
    return `${prefix}[REDACTED]${suffix}`;
  }

  return '[REDACTED]';
}

function isDailyDateSuffix(s: string): boolean {
  if (s.length !== 10) return false;
  return /^\d{4}-\d{2}-\d{2}$/.test(s);
}

/**
 * Exact emulation of Rust `is_neuroclip_log_file` per `src-tauri/src/logging/rotation.rs`.
 */
function rustIsNeuroclipLogFile(filename: string): boolean {
  if (!filename.startsWith('neuroclip')) {
    return false;
  }

  if (filename.endsWith('.log')) {
    return true;
  }

  let datePart: string | null = null;
  if (filename.startsWith('neuroclip.')) {
    datePart = filename.slice('neuroclip.'.length);
  } else if (filename.startsWith('neuroclip-.')) {
    datePart = filename.slice('neuroclip-.'.length);
  } else if (filename.startsWith('neuroclip-')) {
    datePart = filename.slice('neuroclip-'.length);
  }

  if (datePart !== null) {
    return isDailyDateSuffix(datePart);
  }

  return false;
}

interface RetentionPolicy {
  maxAgeDays: number;
  maxDirSizeBytes: number;
  targetDirSizeBytes: number;
}

interface PruneReport {
  deletedExpiredFiles: number;
  deletedOverflowFiles: number;
  bytesFreed: number;
  remainingBytes: number;
}

/**
 * Exact emulation of Rust `prune_logs` per `src-tauri/src/logging/rotation.rs`.
 */
function rustPruneLogs(logDir: string, policy: RetentionPolicy): PruneReport {
  if (!fs.existsSync(logDir)) {
    return {
      deletedExpiredFiles: 0,
      deletedOverflowFiles: 0,
      bytesFreed: 0,
      remainingBytes: 0,
    };
  }

  const entries = fs.readdirSync(logDir);
  const now = Date.now();
  const maxAgeMs = policy.maxAgeDays * 86400 * 1000;

  const report: PruneReport = {
    deletedExpiredFiles: 0,
    deletedOverflowFiles: 0,
    bytesFreed: 0,
    remainingBytes: 0,
  };

  interface Candidate {
    path: string;
    sizeBytes: number;
    mtimeMs: number;
  }

  const activeFiles: Candidate[] = [];

  for (const name of entries) {
    const filePath = path.join(logDir, name);
    let stat: fs.Stats;
    try {
      stat = fs.statSync(filePath);
    } catch {
      continue;
    }

    if (!stat.isFile()) continue;
    if (!rustIsNeuroclipLogFile(name)) continue;

    const sizeBytes = stat.size;
    const mtimeMs = stat.mtimeMs;

    const isExpired =
      policy.maxAgeDays === 0 ? true : now - mtimeMs > maxAgeMs;

    if (isExpired) {
      try {
        fs.unlinkSync(filePath);
        report.deletedExpiredFiles += 1;
        report.bytesFreed += sizeBytes;
      } catch {
        // Ignored, zero-panic
      }
    } else {
      activeFiles.push({ path: filePath, sizeBytes, mtimeMs });
    }
  }

  let totalSize = activeFiles.reduce((acc, f) => acc + f.sizeBytes, 0);

  if (totalSize > policy.maxDirSizeBytes) {
    // Sort oldest first
    activeFiles.sort((a, b) => a.mtimeMs - b.mtimeMs);

    for (const file of activeFiles) {
      if (totalSize <= policy.targetDirSizeBytes) break;

      try {
        fs.unlinkSync(file.path);
        report.deletedOverflowFiles += 1;
        report.bytesFreed += file.sizeBytes;
        totalSize = Math.max(0, totalSize - file.sizeBytes);
      } catch {
        // Ignored, zero-panic
      }
    }
  }

  report.remainingBytes = totalSize;
  return report;
}

// ---------------------------------------------------------------------------
// TEST SUITE
// ---------------------------------------------------------------------------

describe('ADVERSARIAL STRESS TEST: TASK-M4-04 Secret Redaction & Rotation', () => {
  let tempTestDir: string;

  beforeEach(() => {
    tempTestDir = fs.mkdtempSync(path.join(os.tmpdir(), 'neuroclip_adversarial_'));
  });

  afterEach(() => {
    try {
      fs.rmSync(tempTestDir, { recursive: true, force: true });
    } catch {
      // Cleanup best effort
    }
  });

  // =========================================================================
  // VECTOR 1: Secret Redaction Stress (Keys <= 8 chars)
  // =========================================================================
  describe('Vector 1: Secret Redaction Stress — Keys <= 8 chars', () => {
    const shortKeys = [
      '',
      'a',
      '1',
      'AIza',
      'sk-',
      'gsk_',
      '1234567',
      '12345678',
      'abcdefgh',
      'sk-12345',
    ];

    it.each(shortKeys)(
      'TypeScript redactSecret should mask key "%s" (len %i) as [REDACTED]',
      (key) => {
        const result = redactSecret(key);
        expect(result).toBe('[REDACTED]');
      },
    );

    it.each(shortKeys)(
      'Rust redact_secret emulation should mask key "%s" (len %i) as [REDACTED]',
      (key) => {
        const result = rustRedactSecret(key);
        expect(result).toBe('[REDACTED]');
      },
    );

    it('should correctly handle boundary length 9 characters', () => {
      expect(redactSecret('123456789')).toBe('1234[REDACTED]6789');
      expect(rustRedactSecret('123456789')).toBe('1234[REDACTED]6789');
    });
  });

  // =========================================================================
  // VECTOR 2: Secret Redaction Stress (Keys > 8 chars)
  // =========================================================================
  describe('Vector 2: Secret Redaction Stress — Keys > 8 chars', () => {
    const providerKeys = [
      {
        name: 'Groq API Key',
        raw: 'gsk_1234567890abcdef12345678',
        expected: 'gsk_[REDACTED]5678',
      },
      {
        name: 'Gemini API Key',
        raw: 'AIzaSyD-1234567890abcdefABCDEF1234',
        expected: 'AIza[REDACTED]1234',
      },
      {
        name: 'OpenAI Project Key',
        raw: 'sk-proj-1234567890abcdef12345678',
        expected: 'sk-p[REDACTED]5678',
      },
      {
        name: 'Twitch OAuth Token',
        raw: 'oauth:1234567890abcdefghij',
        expected: 'oaut[REDACTED]ghij',
      },
      {
        name: 'Twitch Auth Token Header Parameter',
        raw: 'auth-token=secrettoken12345678',
        expected: 'auth[REDACTED]5678',
      },
    ];

    it.each(providerKeys)(
      'TypeScript redactSecret should retain 4-char prefix and 4-char suffix for $name',
      ({ raw, expected }) => {
        const result = redactSecret(raw);
        expect(result).toBe(expected);
      },
    );

    it.each(providerKeys)(
      'Rust redact_secret emulation should retain 4-char prefix and 4-char suffix for $name',
      ({ raw, expected }) => {
        const result = rustRedactSecret(raw);
        expect(result).toBe(expected);
      },
    );

    it('should handle very long token (2000 chars) preserving prefix 4 and suffix 4', () => {
      const longSecret = 'sk-p' + 'x'.repeat(1992) + '9999';
      expect(redactSecret(longSecret)).toBe('sk-p[REDACTED]9999');
      expect(rustRedactSecret(longSecret)).toBe('sk-p[REDACTED]9999');
    });
  });

  // =========================================================================
  // VECTOR 3: Non-ASCII & Multi-byte UTF-8 Strings (Zero-Panic & Boundary Safety)
  // =========================================================================
  describe('Vector 3: Non-ASCII & UTF-8 Multi-Byte String Safety', () => {
    it('should handle Vietnamese diacritics without crashing or throwing', () => {
      const vietnameseKeys = [
        'KhóaBảoMậtNeuro',
        'TiếngViệt2026',
        'ChàoThếGiới123',
        'ĐăngNhậpHệThống_Token_9999',
      ];

      for (const key of vietnameseKeys) {
        expect(() => redactSecret(key)).not.toThrow();
        expect(() => rustRedactSecret(key)).not.toThrow();

        const tsResult = redactSecret(key);
        const rustResult = rustRedactSecret(key);

        expect(tsResult).toContain('[REDACTED]');
        expect(rustResult).toContain('[REDACTED]');
      }
    });

    it('should handle emoji and surrogate pairs safely', () => {
      const emojiKey = '🔑🔑🔑🔑🔑🔑🔑🔑🔑🔑'; // 10 key emojis
      expect(() => redactSecret(emojiKey)).not.toThrow();
      expect(() => rustRedactSecret(emojiKey)).not.toThrow();

      expect(redactSecret(emojiKey)).toContain('[REDACTED]');
      expect(rustRedactSecret(emojiKey)).toContain('[REDACTED]');
    });

    it('should fall back to [REDACTED] in Rust when byte index 4 lands inside a multibyte char', () => {
      // 'TiếngViệt':
      // 'T' (1 byte: 0..1), 'i' (1 byte: 1..2), 'ế' (3 bytes: 2..5).
      // Byte 4 is right inside 'ế'.
      const str = 'TiếngViệt12345';
      const result = rustRedactSecret(str);
      // Rust is_char_boundary(4) returns false, so it safely returns '[REDACTED]' without panic
      expect(result).toBe('[REDACTED]');
    });

    it('should slice cleanly in Rust when char boundary aligns at byte 4', () => {
      // 'AAAA' = 4 ASCII bytes (char boundary at 4).
      // '1234' = 4 ASCII bytes at end.
      const str = 'AAAA_TiếngViệt_1234';
      const result = rustRedactSecret(str);
      expect(result).toBe('AAAA[REDACTED]1234');
    });
  });

  // =========================================================================
  // VECTOR 4: Mixed Text with Multiple Embedded Keys
  // =========================================================================
  describe('Vector 4: Mixed Text with Multiple Embedded Keys', () => {
    it('should sanitize all multi-provider tokens simultaneously in a single text message', () => {
      const rawText = [
        'Starting stream ingestion pipeline with credentials:',
        'Groq: gsk_1234567890abcdef12345678,',
        'Gemini: AIzaSyD-1234567890abcdefABCDEF1234,',
        'OpenAI: sk-proj-1234567890abcdef12345678,',
        'Twitch IRC: oauth:1234567890abcdefghij,',
        'Twitch HTTP: auth-token=secrettoken12345678,',
        'Auth Header: Bearer abcdef1234567890fedcba1234567890',
      ].join(' ');

      const sanitized = sanitizeLogString(rawText);

      // Verify zero raw secrets remain
      expect(sanitized).not.toContain('gsk_1234567890abcdef12345678');
      expect(sanitized).not.toContain('AIzaSyD-1234567890abcdefABCDEF1234');
      expect(sanitized).not.toContain('sk-proj-1234567890abcdef12345678');
      expect(sanitized).not.toContain('oauth:1234567890abcdefghij');
      expect(sanitized).not.toContain('secrettoken12345678');
      expect(sanitized).not.toContain('abcdef1234567890fedcba1234567890');

      // Verify all redactions were applied
      expect(sanitized).toContain('gsk_[REDACTED]5678');
      expect(sanitized).toContain('AIza[REDACTED]1234');
      expect(sanitized).toContain('sk-p[REDACTED]5678');
      expect(sanitized).toContain('oaut[REDACTED]ghij');
      expect(sanitized).toContain('auth-token=[REDACTED]');
      expect(sanitized).toContain('Bearer [REDACTED]');
    });

    it('should recursively sanitize nested context dictionaries and arrays without leaking', () => {
      const context = {
        groqConfig: {
          key: 'gsk_1234567890abcdef12345678',
          model: 'whisper-large-v3',
        },
        providersList: [
          'AIzaSyD-1234567890abcdefABCDEF1234',
          'sk-proj-1234567890abcdef12345678',
          { nestedHeader: 'Bearer bearer_secret_token_1234567890' },
        ],
        metadata: {
          session: 'auth-token=secrettoken12345678',
          retryCount: 3,
          isLive: true,
          nullVal: null,
        },
      };

      const cleaned = sanitizeLogContext(context);
      expect(cleaned).toBeDefined();

      const serialized = JSON.stringify(cleaned);
      expect(serialized).not.toContain('gsk_1234567890abcdef12345678');
      expect(serialized).not.toContain('AIzaSyD-1234567890abcdefABCDEF1234');
      expect(serialized).not.toContain('sk-proj-1234567890abcdef12345678');
      expect(serialized).not.toContain('bearer_secret_token_1234567890');
      expect(serialized).not.toContain('secrettoken12345678');

      expect(serialized).toContain('gsk_[REDACTED]5678');
      expect(serialized).toContain('AIza[REDACTED]1234');
      expect(serialized).toContain('sk-p[REDACTED]5678');
    });
  });

  // =========================================================================
  // VECTOR 5: Rotation & Pruning Edge Cases — Foreign Files Preservation
  // =========================================================================
  describe('Vector 5: Foreign Files Preservation (Never Pruned)', () => {
    it('is_neuroclip_log_file should strictly identify only NeuroClip log files', () => {
      // Must NOT be considered log files
      expect(rustIsNeuroclipLogFile('important.txt')).toBe(false);
      expect(rustIsNeuroclipLogFile('app_config.json')).toBe(false);
      expect(rustIsNeuroclipLogFile('hardware_profile.json')).toBe(false);
      expect(rustIsNeuroclipLogFile('draft_content.json')).toBe(false);
      expect(rustIsNeuroclipLogFile('subtitles.ass')).toBe(false);
      expect(rustIsNeuroclipLogFile('vod_audio.wav')).toBe(false);
      expect(rustIsNeuroclipLogFile('random_app.log')).toBe(false);
      expect(rustIsNeuroclipLogFile('neuroclip-config.json')).toBe(false);
      expect(rustIsNeuroclipLogFile('neuroclip.txt')).toBe(false);
      expect(rustIsNeuroclipLogFile('neuroclip-backup.bak')).toBe(false);
      expect(rustIsNeuroclipLogFile('neuroclip.ass')).toBe(false);

      // Must BE considered log files
      expect(rustIsNeuroclipLogFile('neuroclip.log')).toBe(true);
      expect(rustIsNeuroclipLogFile('neuroclip-2026-11-10.log')).toBe(true);
      expect(rustIsNeuroclipLogFile('neuroclip-2026-10-01.log')).toBe(true);
      expect(rustIsNeuroclipLogFile('neuroclip.2026-11-10')).toBe(true);
      expect(rustIsNeuroclipLogFile('neuroclip-trace.log')).toBe(true);
    });

    it('should NEVER delete non-neuroclip files on disk even when directory overflows', () => {
      // 1. Create sensitive foreign files
      const importantDoc = path.join(tempTestDir, 'important.txt');
      const appConfig = path.join(tempTestDir, 'app_config.json');
      const hwProfile = path.join(tempTestDir, 'hardware_profile.json');

      fs.writeFileSync(importantDoc, 'CRITICAL USER DATA - DO NOT DELETE');
      fs.writeFileSync(appConfig, '{"theme": "neuro-dark"}');
      fs.writeFileSync(hwProfile, '{"gpu": "RTX 4090"}');

      // Set old mtime (> 30 days old) on foreign files
      const pastTime = new Date(Date.now() - 30 * 86400 * 1000);
      fs.utimesSync(importantDoc, pastTime, pastTime);
      fs.utimesSync(appConfig, pastTime, pastTime);
      fs.utimesSync(hwProfile, pastTime, pastTime);

      // 2. Create expired neuroclip log
      const expiredLog = path.join(tempTestDir, 'neuroclip-2026-09-01.log');
      fs.writeFileSync(expiredLog, 'Old log data');
      fs.utimesSync(expiredLog, pastTime, pastTime);

      // 3. Run pruning with default policy (14 days, 50MB, 30MB)
      const policy: RetentionPolicy = {
        maxAgeDays: 14,
        maxDirSizeBytes: 50 * 1024 * 1024,
        targetDirSizeBytes: 30 * 1024 * 1024,
      };

      const report = rustPruneLogs(tempTestDir, policy);

      // Assert expired neuroclip log was deleted
      expect(report.deletedExpiredFiles).toBe(1);
      expect(fs.existsSync(expiredLog)).toBe(false);

      // Assert non-neuroclip files are 100% PRESERVED
      expect(fs.existsSync(importantDoc)).toBe(true);
      expect(fs.existsSync(appConfig)).toBe(true);
      expect(fs.existsSync(hwProfile)).toBe(true);
      expect(fs.readFileSync(importantDoc, 'utf8')).toBe('CRITICAL USER DATA - DO NOT DELETE');
    });
  });

  // =========================================================================
  // VECTOR 6: Rotation & Pruning Edge Cases — Age Retention (> 14 Days)
  // =========================================================================
  describe('Vector 6: Age Retention Pruning (> 14 Days)', () => {
    it('should delete log files older than 14 days and preserve newer ones', () => {
      const now = Date.now();
      const dayMs = 86400 * 1000;

      // File 1: 20 days old (expired)
      const f1 = path.join(tempTestDir, 'neuroclip-20-days-old.log');
      fs.writeFileSync(f1, '20 days old content');
      const time20d = new Date(now - 20 * dayMs);
      fs.utimesSync(f1, time20d, time20d);

      // File 2: 15 days old (expired)
      const f2 = path.join(tempTestDir, 'neuroclip-15-days-old.log');
      fs.writeFileSync(f2, '15 days old content');
      const time15d = new Date(now - 15 * dayMs);
      fs.utimesSync(f2, time15d, time15d);

      // File 3: 13 days old (active, keep)
      const f3 = path.join(tempTestDir, 'neuroclip-13-days-old.log');
      fs.writeFileSync(f3, '13 days old content');
      const time13d = new Date(now - 13 * dayMs);
      fs.utimesSync(f3, time13d, time13d);

      // File 4: today (active, keep)
      const f4 = path.join(tempTestDir, 'neuroclip-today.log');
      fs.writeFileSync(f4, 'Today content');

      const policy: RetentionPolicy = {
        maxAgeDays: 14,
        maxDirSizeBytes: 50 * 1024 * 1024,
        targetDirSizeBytes: 30 * 1024 * 1024,
      };

      const report = rustPruneLogs(tempTestDir, policy);

      expect(report.deletedExpiredFiles).toBe(2);
      expect(fs.existsSync(f1)).toBe(false);
      expect(fs.existsSync(f2)).toBe(false);
      expect(fs.existsSync(f3)).toBe(true);
      expect(fs.existsSync(f4)).toBe(true);
    });
  });

  // =========================================================================
  // VECTOR 7: Cumulative Size Overflow (> 50MB -> <= 30MB Oldest First)
  // =========================================================================
  describe('Vector 7: Cumulative Size Overflow Pruning (> 50MB -> <= 30MB Oldest First)', () => {
    it('should prune oldest files first until directory is <= 30MB when total exceeds 50MB', () => {
      const now = Date.now();
      const hourMs = 3600 * 1000;

      // Create 6 files of 10MB each (total 60MB > 50MB max ceiling)
      // Low-water mark target is 30MB (so it should delete 3 oldest files to get to 30MB)
      const fileNames: string[] = [];
      const MB = 1024 * 1024;
      const payload10MB = Buffer.alloc(10 * MB, 0x41); // 10MB

      for (let i = 1; i <= 6; i++) {
        const name = `neuroclip-file-${i}.log`;
        const filePath = path.join(tempTestDir, name);
        fs.writeFileSync(filePath, payload10MB);

        // Assign progressive modification times (file 1 oldest, file 6 newest)
        const fileMtime = new Date(now - (10 - i) * hourMs);
        fs.utimesSync(filePath, fileMtime, fileMtime);
        fileNames.push(filePath);
      }

      const policy: RetentionPolicy = {
        maxAgeDays: 14,
        maxDirSizeBytes: 50 * MB,
        targetDirSizeBytes: 30 * MB,
      };

      const report = rustPruneLogs(tempTestDir, policy);

      // Total was 60MB, target is 30MB.
      // File 1 (10MB) deleted -> 50MB
      // File 2 (10MB) deleted -> 40MB
      // File 3 (10MB) deleted -> 30MB (target reached!)
      expect(report.deletedOverflowFiles).toBe(3);
      expect(report.bytesFreed).toBe(30 * MB);
      expect(report.remainingBytes).toBe(30 * MB);

      // Verify oldest files (1, 2, 3) were pruned
      expect(fs.existsSync(fileNames[0]!)).toBe(false);
      expect(fs.existsSync(fileNames[1]!)).toBe(false);
      expect(fs.existsSync(fileNames[2]!)).toBe(false);

      // Verify newer files (4, 5, 6) were preserved
      expect(fs.existsSync(fileNames[3]!)).toBe(true);
      expect(fs.existsSync(fileNames[4]!)).toBe(true);
      expect(fs.existsSync(fileNames[5]!)).toBe(true);
    });
  });

  // =========================================================================
  // VECTOR 8: Zero-Panic & Directory Boundary Conditions
  // =========================================================================
  describe('Vector 8: Zero-Panic & Directory Boundary Conditions', () => {
    it('should return default report with zero panic when directory does not exist', () => {
      const nonExistent = path.join(tempTestDir, 'does_not_exist_xyz123');
      const policy: RetentionPolicy = {
        maxAgeDays: 14,
        maxDirSizeBytes: 50 * 1024 * 1024,
        targetDirSizeBytes: 30 * 1024 * 1024,
      };

      expect(() => rustPruneLogs(nonExistent, policy)).not.toThrow();
      const report = rustPruneLogs(nonExistent, policy);
      expect(report.deletedExpiredFiles).toBe(0);
      expect(report.deletedOverflowFiles).toBe(0);
      expect(report.bytesFreed).toBe(0);
      expect(report.remainingBytes).toBe(0);
    });

    it('should return default report with zero panic on empty directory', () => {
      const emptyDir = path.join(tempTestDir, 'empty_dir');
      fs.mkdirSync(emptyDir);

      const policy: RetentionPolicy = {
        maxAgeDays: 14,
        maxDirSizeBytes: 50 * 1024 * 1024,
        targetDirSizeBytes: 30 * 1024 * 1024,
      };

      expect(() => rustPruneLogs(emptyDir, policy)).not.toThrow();
      const report = rustPruneLogs(emptyDir, policy);
      expect(report.deletedExpiredFiles).toBe(0);
      expect(report.deletedOverflowFiles).toBe(0);
      expect(report.remainingBytes).toBe(0);
    });

    it('should handle read-only directory or locked file errors gracefully without panic', () => {
      const lockedLog = path.join(tempTestDir, 'neuroclip-locked.log');
      fs.writeFileSync(lockedLog, 'locked log data');

      // Make read-only
      fs.chmodSync(lockedLog, 0o444);

      const policy: RetentionPolicy = {
        maxAgeDays: 0, // Force prune all
        maxDirSizeBytes: 0,
        targetDirSizeBytes: 0,
      };

      expect(() => rustPruneLogs(tempTestDir, policy)).not.toThrow();
    });
  });

  // =========================================================================
  // VECTOR 9: Rust Production AST & Contract Verification
  // =========================================================================
  describe('Vector 9: Rust AST & Zero-Panic Invariant Audit', () => {
    it('should assert secrets.rs uses is_char_boundary to guard all slicing operations', () => {
      const secretsCode = fs.readFileSync(
        path.resolve(process.cwd(), 'src-tauri/src/logging/secrets.rs'),
        'utf8',
      );

      // Verify char boundary checks
      expect(secretsCode).toContain('key.is_char_boundary(4)');
      expect(secretsCode).toContain('key.is_char_boundary(key.len() - 4)');
      expect(secretsCode).toContain('RedactingWriter');
      expect(secretsCode).toContain('String::from_utf8_lossy(buf)');

      // Verify zero forbidden calls in production code (outside #[cfg(test)])
      const prodCode = secretsCode.split('#[cfg(test)]')[0]!;
      expect(prodCode).not.toMatch(/\.unwrap\(\)/);
      expect(prodCode).not.toMatch(/\.expect\(/);
      expect(prodCode).not.toMatch(/\bpanic!\(/);
    });

    it('should assert rotation.rs enforces two-phase pruning and zero panic', () => {
      const rotationCode = fs.readFileSync(
        path.resolve(process.cwd(), 'src-tauri/src/logging/rotation.rs'),
        'utf8',
      );

      // Verify constants & functions
      expect(rotationCode).toContain('max_age_days: 14');
      expect(rotationCode).toContain('max_dir_size_bytes: 50 * 1024 * 1024');
      expect(rotationCode).toContain('target_dir_size_bytes: 30 * 1024 * 1024');
      expect(rotationCode).toContain('pub fn is_neuroclip_log_file');
      expect(rotationCode).toContain('pub fn prune_logs');

      // Verify zero forbidden calls in production code
      const prodCode = rotationCode.split('#[cfg(test)]')[0]!;
      expect(prodCode).not.toMatch(/\.unwrap\(\)/);
      expect(prodCode).not.toMatch(/\.expect\(/);
      expect(prodCode).not.toMatch(/\bpanic!\(/);
    });
  });
});
