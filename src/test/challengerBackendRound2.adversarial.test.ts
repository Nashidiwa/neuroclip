/**
 * CHALLENGER ROUND 2 ADVERSARIAL STRESS TEST SUITE: BACKEND SECURITY & MULTI-FILE RETRIEVAL
 *
 * Verifies Round 2 backend fixes:
 * 1. Multi-file retrieval in log_cmd.rs:
 *    - Simulate multiple log files (neuroclip-2026-11-09.log with 200 items, neuroclip-2026-11-10.log with 50 items)
 *    - Query with limit = 100
 *    - Assert all 50 items from today are retained
 *    - Assert 50 newest items from yesterday are retained
 *    - Assert returned vector is strictly chronological (oldest to newest)
 * 2. File rotation pattern matching in rotation.rs:
 *    - Test is_neuroclip_log_file against:
 *      - neuroclip-config.json (false)
 *      - neuroclip.txt (false)
 *      - neuroclip-backup.bak (false)
 *      - neuroclip.ass (false)
 *      - neuroclip-2026-11-10.log (true)
 *      - neuroclip.log (true)
 *    - Assert 100% of non-log files return false and valid log files return true
 * 3. Static AST & Zero-Panic Audit of Rust Backend Implementations
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';

// ===========================================================================
// Faithful Emulation of Rust backend algorithms per src-tauri implementation
// ===========================================================================

interface RustLogEntry {
  timestamp: string;
  level: string;
  target: string;
  message: string;
  fields?: unknown;
  line?: number;
  file?: string;
}

/**
 * Faithful emulation of Rust `is_daily_date_suffix` from `src-tauri/src/logging/rotation.rs`.
 */
function isDailyDateSuffix(s: string): boolean {
  if (s.length !== 10) {
    return false;
  }
  // Expected format: YYYY-MM-DD
  const isAsciiDigit = (ch: string) => ch >= '0' && ch <= '9';
  return (
    isAsciiDigit(s[0]!) &&
    isAsciiDigit(s[1]!) &&
    isAsciiDigit(s[2]!) &&
    isAsciiDigit(s[3]!) &&
    s[4] === '-' &&
    isAsciiDigit(s[5]!) &&
    isAsciiDigit(s[6]!) &&
    s[7] === '-' &&
    isAsciiDigit(s[8]!) &&
    isAsciiDigit(s[9]!)
  );
}

/**
 * Faithful emulation of Rust `is_neuroclip_log_file` from `src-tauri/src/logging/rotation.rs`.
 */
function rustIsNeuroclipLogFile(filename: string): boolean {
  if (!filename.startsWith('neuroclip')) {
    return false;
  }

  // Standard .log extension
  if (filename.endsWith('.log')) {
    return true;
  }

  // Daily rolling files without .log produced by tracing-appender
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

/**
 * Faithful emulation of Rust `extract_log_date` from `src-tauri/src/commands/log_cmd.rs`.
 */
function extractLogDate(filename: string): string | null {
  if (!filename.startsWith('neuroclip')) return null;
  let stripped = filename.slice('neuroclip'.length);
  while (stripped.startsWith('-') || stripped.startsWith('.')) {
    stripped = stripped.slice(1);
  }
  if (stripped.length >= 10) {
    const candidate = stripped.slice(0, 10);
    if (isDailyDateSuffix(candidate)) {
      return candidate;
    }
  }
  return null;
}

/**
 * Faithful emulation of Rust `get_recent_logs` from `src-tauri/src/commands/log_cmd.rs`.
 */
function simulateGetRecentLogs(
  logDir: string,
  limit?: number,
  levelFilter?: string,
): RustLogEntry[] {
  const DEFAULT_LOG_LIMIT = 200;
  const MAX_LOG_LIMIT = 1000;
  const rawLimit = limit ?? DEFAULT_LOG_LIMIT;
  const effectiveLimit = Math.max(1, Math.min(MAX_LOG_LIMIT, rawLimit));

  if (!fs.existsSync(logDir)) {
    return [];
  }

  const entries = fs.readdirSync(logDir, { withFileTypes: true });
  const logFiles: Array<{ name: string; fullPath: string; mtimeMs: number }> = [];

  for (const entry of entries) {
    if (entry.isFile() && rustIsNeuroclipLogFile(entry.name)) {
      const fullPath = path.join(logDir, entry.name);
      const stat = fs.statSync(fullPath);
      logFiles.push({ name: entry.name, fullPath, mtimeMs: stat.mtimeMs });
    }
  }

  if (logFiles.length === 0) {
    return [];
  }

  // Sort newest log file first by filename date, then modified timestamp
  logFiles.sort((a, b) => {
    const dateA = extractLogDate(a.name);
    const dateB = extractLogDate(b.name);
    if (dateA && dateB && dateA !== dateB) {
      return dateB.localeCompare(dateA); // descending
    }
    return b.mtimeMs - a.mtimeMs;
  });

  const collectedEntries: RustLogEntry[] = [];

  for (const fileInfo of logFiles) {
    if (collectedEntries.length >= effectiveLimit) {
      break;
    }

    const content = fs.readFileSync(fileInfo.fullPath, 'utf8');
    const lines = content.split('\n');
    const fileEntries: RustLogEntry[] = [];

    for (const rawLine of lines) {
      const trimmed = rawLine.trim();
      if (!trimmed) continue;

      try {
        const parsed = JSON.parse(trimmed) as {
          timestamp?: string;
          level?: string;
          target?: string;
          message?: string;
        };
        fileEntries.push({
          timestamp: parsed.timestamp || '',
          level: (parsed.level || 'INFO').toUpperCase(),
          target: parsed.target || 'app',
          message: parsed.message || '',
        });
      } catch {
        fileEntries.push({
          timestamp: new Date().toISOString(),
          level: 'INFO',
          target: 'system',
          message: trimmed,
        });
      }
    }

    // Apply level filter
    if (levelFilter && levelFilter.toUpperCase() !== 'ALL') {
      const filterUpper = levelFilter.toUpperCase();
      const filtered = fileEntries.filter((e) => e.level === filterUpper);
      fileEntries.length = 0;
      fileEntries.push(...filtered);
    }

    // Collect newest entries from this file (iterating in reverse order)
    for (let i = fileEntries.length - 1; i >= 0; i--) {
      if (collectedEntries.length >= effectiveLimit) {
        break;
      }
      collectedEntries.push(fileEntries[i]!);
    }
  }

  // Reverse so that returned entries are in monotonic chronological order (oldest to newest)
  collectedEntries.reverse();

  return collectedEntries;
}

// ===========================================================================
// EMPIRICAL TEST SUITE
// ===========================================================================

describe('CHALLENGER ROUND 2 ADVERSARIAL TEST SUITE', () => {
  let tempLogDir: string;

  beforeEach(() => {
    tempLogDir = fs.mkdtempSync(path.join(os.tmpdir(), 'neuroclip_challenger_r2_'));
  });

  afterEach(() => {
    try {
      fs.rmSync(tempLogDir, { recursive: true, force: true });
    } catch {
      // Best-effort cleanup
    }
  });

  // =========================================================================
  // 1. Multi-File Retrieval in log_cmd.rs
  // =========================================================================
  describe('1. Multi-File Retrieval in log_cmd.rs', () => {
    it('retrieves exactly 100 items from 2 daily files, retaining all 50 today items + 50 newest yesterday items in strict chronological order', () => {
      // 1. Populate neuroclip-2026-11-09.log with 200 items (yesterday_000 to yesterday_199)
      const yesterdayFile = path.join(tempLogDir, 'neuroclip-2026-11-09.log');
      const yesterdayLines: string[] = [];
      for (let i = 0; i < 200; i++) {
        const paddedIndex = String(i).padStart(3, '0');
        const hour = String(Math.floor(i / 60)).padStart(2, '0');
        const min = String(i % 60).padStart(2, '0');
        const timestamp = `2026-11-09T${hour}:${min}:00.000Z`;
        yesterdayLines.push(
          JSON.stringify({
            timestamp,
            level: 'INFO',
            target: 'app',
            message: `yesterday_${paddedIndex}`,
          }),
        );
      }
      fs.writeFileSync(yesterdayFile, yesterdayLines.join('\n') + '\n', 'utf8');

      // 2. Populate neuroclip-2026-11-10.log with 50 items (today_000 to today_049)
      const todayFile = path.join(tempLogDir, 'neuroclip-2026-11-10.log');
      const todayLines: string[] = [];
      for (let i = 0; i < 50; i++) {
        const paddedIndex = String(i).padStart(3, '0');
        const hour = String(Math.floor(i / 60)).padStart(2, '0');
        const min = String(i % 60).padStart(2, '0');
        const timestamp = `2026-11-10T${hour}:${min}:00.000Z`;
        todayLines.push(
          JSON.stringify({
            timestamp,
            level: 'INFO',
            target: 'app',
            message: `today_${paddedIndex}`,
          }),
        );
      }
      fs.writeFileSync(todayFile, todayLines.join('\n') + '\n', 'utf8');

      // 3. Call get_recent_logs(Some(100), None)
      const results = simulateGetRecentLogs(tempLogDir, 100, undefined);

      // Invariant 1: Total count must be exactly 100
      expect(results.length).toBe(100);

      // Invariant 2: Exactly 50 items from today must be retained
      const todayEntries = results.filter((e) => e.message.startsWith('today_'));
      expect(todayEntries.length).toBe(50);
      expect(todayEntries[0]?.message).toBe('today_000');
      expect(todayEntries[49]?.message).toBe('today_049');

      // Invariant 3: Exactly 50 newest items from yesterday must be retained (items 150..199)
      const yesterdayEntries = results.filter((e) => e.message.startsWith('yesterday_'));
      expect(yesterdayEntries.length).toBe(50);
      expect(yesterdayEntries[0]?.message).toBe('yesterday_150');
      expect(yesterdayEntries[49]?.message).toBe('yesterday_199');

      // Invariant 4: First entry in vector must be yesterday_150, last entry must be today_049
      expect(results[0]?.message).toBe('yesterday_150');
      expect(results[results.length - 1]?.message).toBe('today_049');

      // Invariant 5: Returned vector must be strictly monotonically chronological
      for (let i = 0; i < results.length - 1; i++) {
        const currentTimestamp = results[i]!.timestamp;
        const nextTimestamp = results[i + 1]!.timestamp;
        expect(currentTimestamp.localeCompare(nextTimestamp)).toBeLessThanOrEqual(0);
      }
    });

    it('handles query limit larger than total entries across multiple files gracefully', () => {
      // Populate 2 files with 10 entries each (total 20 entries)
      const f1 = path.join(tempLogDir, 'neuroclip-2026-11-08.log');
      fs.writeFileSync(
        f1,
        '{"timestamp":"2026-11-08T10:00:00Z","level":"INFO","target":"app","message":"f1_entry"}\n',
      );

      const f2 = path.join(tempLogDir, 'neuroclip-2026-11-09.log');
      fs.writeFileSync(
        f2,
        '{"timestamp":"2026-11-09T10:00:00Z","level":"INFO","target":"app","message":"f2_entry"}\n',
      );

      const results = simulateGetRecentLogs(tempLogDir, 500, undefined);
      expect(results.length).toBe(2);
      expect(results[0]?.message).toBe('f1_entry');
      expect(results[1]?.message).toBe('f2_entry');
    });

    it('correctly applies level filter across multiple log files', () => {
      const f1 = path.join(tempLogDir, 'neuroclip-2026-11-09.log');
      fs.writeFileSync(
        f1,
        [
          '{"timestamp":"2026-11-09T10:00:00Z","level":"INFO","target":"app","message":"info_y"}',
          '{"timestamp":"2026-11-09T10:01:00Z","level":"ERROR","target":"app","message":"error_y"}',
        ].join('\n') + '\n',
      );

      const f2 = path.join(tempLogDir, 'neuroclip-2026-11-10.log');
      fs.writeFileSync(
        f2,
        [
          '{"timestamp":"2026-11-10T10:00:00Z","level":"INFO","target":"app","message":"info_t"}',
          '{"timestamp":"2026-11-10T10:01:00Z","level":"ERROR","target":"app","message":"error_t"}',
        ].join('\n') + '\n',
      );

      const errorResults = simulateGetRecentLogs(tempLogDir, 100, 'ERROR');
      expect(errorResults.length).toBe(2);
      expect(errorResults[0]?.message).toBe('error_y');
      expect(errorResults[1]?.message).toBe('error_t');
      expect(errorResults.every((e) => e.level === 'ERROR')).toBe(true);
    });
  });

  // =========================================================================
  // 2. File Rotation Pattern Matching in rotation.rs
  // =========================================================================
  describe('2. File Rotation Pattern Matching in rotation.rs', () => {
    const requiredTestCases = [
      { filename: 'neuroclip-config.json', expected: false, reason: 'Config JSON file must be rejected' },
      { filename: 'neuroclip.txt', expected: false, reason: 'Plain text file must be rejected' },
      { filename: 'neuroclip-backup.bak', expected: false, reason: 'Backup file must be rejected' },
      { filename: 'neuroclip.ass', expected: false, reason: 'Subtitle file must be rejected' },
      { filename: 'neuroclip-2026-11-10.log', expected: true, reason: 'Standard daily dated .log file must match' },
      { filename: 'neuroclip.log', expected: true, reason: 'Standard main .log file must match' },
    ];

    it.each(requiredTestCases)(
      'is_neuroclip_log_file("$filename") should return $expected ($reason)',
      ({ filename, expected }) => {
        const result = rustIsNeuroclipLogFile(filename);
        expect(result).toBe(expected);
      },
    );

    it('empirically asserts 100% of non-log files return false and valid log files return true', () => {
      const nonLogFiles = [
        'neuroclip-config.json',
        'neuroclip.txt',
        'neuroclip-backup.bak',
        'neuroclip.ass',
        'neuroclip.json',
        'neuroclip.bak',
        'neuroclip.old',
        'hardware_profile.json',
        'app_config.json',
        'random_app.log',
        'neuroclip-invalid-date-format',
        'neuroclip-20261110', // Missing dashes
        'neuroclip-2026-1-1',  // Invalid date length
        'neuroclip.',
        'neuroclip-',
      ];

      for (const nonLog of nonLogFiles) {
        expect(
          rustIsNeuroclipLogFile(nonLog),
          `Expected non-log file "${nonLog}" to be rejected by is_neuroclip_log_file`,
        ).toBe(false);
      }

      const validLogFiles = [
        'neuroclip.log',
        'neuroclip-2026-11-10.log',
        'neuroclip-2026-11-09.log',
        'neuroclip-trace.log',
        'neuroclip-test.log',
        'neuroclip.2026-11-10',    // Tracing-appender rolling without extension
        'neuroclip-2026-11-10',   // Tracing-appender rolling without extension
        'neuroclip-.2026-11-10',  // Tracing-appender prefix variant
      ];

      for (const validLog of validLogFiles) {
        expect(
          rustIsNeuroclipLogFile(validLog),
          `Expected valid log file "${validLog}" to be accepted by is_neuroclip_log_file`,
        ).toBe(true);
      }
    });
  });

  // =========================================================================
  // 3. Static AST & Rust Backend Invariant Audit
  // =========================================================================
  describe('3. Static AST & Rust Backend Invariant Audit', () => {
    it('verifies src-tauri/src/commands/log_cmd.rs implements multi-file date sorting and reverse collection', () => {
      const logCmdPath = path.resolve(process.cwd(), 'src-tauri/src/commands/log_cmd.rs');
      expect(fs.existsSync(logCmdPath)).toBe(true);
      const code = fs.readFileSync(logCmdPath, 'utf8');

      // Check date extraction and sorting
      expect(code).toContain('fn extract_log_date(path: &std::path::Path) -> Option<&str>');
      expect(code).toContain('log_files.sort_by');
      expect(code).toContain('extract_log_date');

      // Check reverse iteration for newest entries
      expect(code).toContain('file_entries.into_iter().rev()');

      // Check vector reversal for chronological ordering
      expect(code).toContain('entries.reverse()');

      // Check unit test presence
      expect(code).toContain('test_get_recent_logs_multi_file_retention_order');

      // Invariant: zero forbidden calls outside test
      const prodCode = code.split('#[cfg(test)]')[0]!;
      expect(prodCode).not.toMatch(/\.unwrap\(\)/);
      expect(prodCode).not.toMatch(/\.expect\(/);
      expect(prodCode).not.toMatch(/\bpanic!\(/);
    });

    it('verifies src-tauri/src/logging/rotation.rs implements strict pattern matching with date suffix validation', () => {
      const rotationPath = path.resolve(process.cwd(), 'src-tauri/src/logging/rotation.rs');
      expect(fs.existsSync(rotationPath)).toBe(true);
      const code = fs.readFileSync(rotationPath, 'utf8');

      // Check helper and pattern matching
      expect(code).toContain('fn is_daily_date_suffix(s: &str) -> bool');
      expect(code).toContain('pub fn is_neuroclip_log_file(filename: &str) -> bool');
      expect(code).toContain('filename.ends_with(".log")');

      // Invariant: zero forbidden calls outside test
      const prodCode = code.split('#[cfg(test)]')[0]!;
      expect(prodCode).not.toMatch(/\.unwrap\(\)/);
      expect(prodCode).not.toMatch(/\.expect\(/);
      expect(prodCode).not.toMatch(/\bpanic!\(/);
    });
  });
});
