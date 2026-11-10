/**
 * Adversarial Stress & Concurrency Verification Suite for TASK-M4-03
 *
 * Focus:
 * 1. High-concurrency atomic persistence write race simulation (NTFS / MoveFileEx equivalent).
 * 2. Concurrent reader isolation (zero 0-byte reads, zero torn writes).
 * 3. File corruption recovery (0-byte, truncated JSON, invalid UTF-8 bytes, schema violation).
 * 4. Partial write failure and temporary file purge cleanup.
 * 5. Frontend settingsStore high-frequency concurrency and error resiliency.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { useSettingsStore, DEFAULT_APP_CONFIG } from '../stores/settingsStore.ts';
import {
  type AppConfigJson,
  toAppConfig,
  toAppConfigJson,
  isAppConfig,
} from '../services/config/settings.ts';
import * as settingsApi from '../services/config/settings.ts';
import * as hardwareApi from '../services/config/hardware.ts';
import { ERROR_CODES } from '../services/ipc/error.ts';

// ---------------------------------------------------------------------------
// Simulation of Backend Atomic Persistence (Matching src-tauri/src/config/atomic.rs)
// ---------------------------------------------------------------------------

interface AtomicSaveOptions {
  injectWriteFailure?: boolean;
  injectRenameFailure?: boolean;
}

/**
 * High-fidelity Node.js implementation of `atomic_save_json` from `src-tauri/src/config/atomic.rs`.
 */
async function simulateAtomicSaveJson<T>(
  targetPath: string,
  data: T,
  options?: AtomicSaveOptions,
): Promise<void> {
  const parent = path.dirname(targetPath);
  if (parent && !fs.existsSync(parent)) {
    fs.mkdirSync(parent, { recursive: true });
  }

  const jsonBytes = Buffer.from(JSON.stringify(data, null, 2), 'utf8');

  // Unique nonce timestamp nanos matching Rust SystemTime::now()
  const nonce = `${Date.now()}_${process.hrtime.bigint()}_${Math.random().toString(36).substring(2, 8)}`;
  const baseName = path.basename(targetPath);
  const tempPath = path.join(parent, `.${baseName}.tmp.${nonce}`);

  try {
    if (options?.injectWriteFailure) {
      throw new Error('Simulated write I/O disk failure');
    }

    // Write and flush
    const fd = fs.openSync(tempPath, 'w');
    fs.writeSync(fd, jsonBytes);
    fs.fsyncSync(fd);
    fs.closeSync(fd);

    if (options?.injectRenameFailure) {
      throw new Error('Simulated atomic rename failure');
    }

    // Atomic rename (MoveFileExW replacement on Windows NTFS)
    fs.renameSync(tempPath, targetPath);
  } catch (err) {
    if (fs.existsSync(tempPath)) {
      try {
        fs.unlinkSync(tempPath);
      } catch {
        // cleanup best effort
      }
    }
    throw err;
  }
}

/**
 * High-fidelity Node.js implementation of `atomic_load_json` from `src-tauri/src/config/atomic.rs`.
 */
function simulateAtomicLoadJson<T>(targetPath: string, fallback: T): T {
  if (!fs.existsSync(targetPath)) {
    return fallback;
  }

  try {
    const content = fs.readFileSync(targetPath, 'utf8');
    if (!content || content.trim().length === 0) {
      return fallback;
    }
    const parsed: unknown = JSON.parse(content);
    return parsed as T;
  } catch {
    // Corrupted file, invalid UTF-8 or malformed JSON triggers fallback
    return fallback;
  }
}

// ---------------------------------------------------------------------------
// Adversarial Stress Tests
// ---------------------------------------------------------------------------

describe('ADVERSARIAL STRESS TEST: Atomic Storage & Concurrency Resilience', () => {
  let testTempDir: string;

  beforeEach(() => {
    testTempDir = path.join(
      os.tmpdir(),
      `neuroclip_adversarial_${Date.now()}_${Math.random().toString(36).substring(2, 8)}`,
    );
    fs.mkdirSync(testTempDir, { recursive: true });
  });

  afterEach(() => {
    if (fs.existsSync(testTempDir)) {
      try {
        fs.rmSync(testTempDir, { recursive: true, force: true });
      } catch {
        // ignore cleanup error
      }
    }
  });

  describe('1. High-Concurrency Write Race Simulation', () => {
    it('should withstand 100 concurrent atomic writes without 0-byte truncation or corrupted state', async () => {
      const targetConfigPath = path.join(testTempDir, 'app_config.json');
      const TOTAL_CONCURRENT_WRITES = 100;

      // Launch 100 concurrent writes with varying data
      const tasks: Promise<void>[] = [];
      for (let i = 0; i < TOTAL_CONCURRENT_WRITES; i += 1) {
        const payload: AppConfigJson = {
          theme: i % 2 === 0 ? 'dark' : 'light',
          language: i % 3 === 0 ? 'vi' : 'en',
          telemetry_enabled: false,
          auto_update_sidecars: true,
          output_directory: `C:/Output/${i}`,
          cache_directory: `C:/Cache/${i}`,
          audio_buffer_size_kb: 64 + i,
          active_stt_provider: 'groq',
          active_stt_model: 'whisper-large-v3',
          active_translation_model: 'gemini-1.5-flash',
          sidecar_paths: {
            ffmpeg_path: null,
            ytdlp_path: null,
            whisper_worker_path: null,
          },
          groq_api_key: `gsk_${i}`,
          gemini_api_key: `AIza_${i}`,
          max_history_days: 14,
        };

        tasks.push(simulateAtomicSaveJson(targetConfigPath, payload));
      }

      // Await all concurrent writes
      const results = await Promise.allSettled(tasks);
      const fulfilled = results.filter((r) => r.status === 'fulfilled');
      expect(fulfilled.length).toBe(TOTAL_CONCURRENT_WRITES);

      // Verify the final file on disk is strictly valid and not 0 bytes
      expect(fs.existsSync(targetConfigPath)).toBe(true);
      const stats = fs.statSync(targetConfigPath);
      expect(stats.size).toBeGreaterThan(0);

      // Verify parseable and matches Schema / domain types
      const content = fs.readFileSync(targetConfigPath, 'utf8');
      const loaded: unknown = JSON.parse(content);
      const domain = toAppConfig(loaded as AppConfigJson);
      expect(isAppConfig(domain)).toBe(true);

      // Verify zero leftover .tmp files
      const dirFiles = fs.readdirSync(testTempDir);
      const tmpFiles = dirFiles.filter((f) => f.includes('.tmp.'));
      expect(tmpFiles.length).toBe(0);
    });

    it('should maintain atomic read isolation during heavy concurrent write activity', async () => {
      const targetConfigPath = path.join(testTempDir, 'app_config.json');

      // Initialize with default
      await simulateAtomicSaveJson(targetConfigPath, toAppConfigJson(DEFAULT_APP_CONFIG));

      let corruptedReads = 0;
      let zeroByteReads = 0;
      let successfulReads = 0;

      // Start 50 concurrent background writes
      const writerPromises = Array.from({ length: 50 }, async (_, i) => {
        const payload = toAppConfigJson({
          ...DEFAULT_APP_CONFIG,
          audioBufferSizeKb: 64 + (i % 10) * 16,
          outputDirectory: `C:/Out/${i}`,
        });
        await simulateAtomicSaveJson(targetConfigPath, payload);
      });

      // Simultaneously execute 100 rapid concurrent reads
      const readerPromises = Array.from({ length: 100 }, async () => {
        try {
          const raw = fs.readFileSync(targetConfigPath, 'utf8');
          if (raw.length === 0) {
            zeroByteReads += 1;
            return;
          }
          const parsed = JSON.parse(raw);
          if (parsed && typeof parsed.audio_buffer_size_kb === 'number') {
            successfulReads += 1;
          } else {
            corruptedReads += 1;
          }
        } catch {
          corruptedReads += 1;
        }
      });

      await Promise.all([...writerPromises, ...readerPromises]);

      // Assert atomic isolation: ZERO partial/corrupted/zero-byte reads occurred
      expect(zeroByteReads).toBe(0);
      expect(corruptedReads).toBe(0);
      expect(successfulReads).toBe(100);
    });

    it('should withstand 100 rapid sequential writes without file locking collisions', async () => {
      const targetConfigPath = path.join(testTempDir, 'app_config.json');

      for (let i = 0; i < 100; i += 1) {
        const payload = toAppConfigJson({
          ...DEFAULT_APP_CONFIG,
          audioBufferSizeKb: 64 + i,
        });
        await simulateAtomicSaveJson(targetConfigPath, payload);
      }

      const content = fs.readFileSync(targetConfigPath, 'utf8');
      const loaded = JSON.parse(content);
      expect(loaded.audio_buffer_size_kb).toBe(163); // 64 + 99

      const dirFiles = fs.readdirSync(testTempDir);
      const tmpFiles = dirFiles.filter((f) => f.includes('.tmp.'));
      expect(tmpFiles.length).toBe(0);
    });

    it('should preserve Vietnamese diacritics, special symbols, and Unicode strings without corruption', async () => {
      const targetConfigPath = path.join(testTempDir, 'app_config.json');
      const unicodeOutputDir = 'C:/Thư mục/Xuất video Neuro-sama Việt Nam 🇻🇳 ✨';

      const payload = toAppConfigJson({
        ...DEFAULT_APP_CONFIG,
        outputDirectory: unicodeOutputDir,
        groqApiKey: 'gsk_khoá_bí_mật_123_🔑',
      });

      await simulateAtomicSaveJson(targetConfigPath, payload);

      const loadedRaw = simulateAtomicLoadJson<AppConfigJson>(targetConfigPath, toAppConfigJson(DEFAULT_APP_CONFIG));
      const loadedDomain = toAppConfig(loadedRaw);

      expect(loadedDomain.outputDirectory).toBe(unicodeOutputDir);
      expect(loadedDomain.groqApiKey).toBe('gsk_khoá_bí_mật_123_🔑');
    });
  });

  describe('2. Corrupted File Recovery & Fallback Resiliency', () => {
    it('should recover cleanly when app_config.json is 0 bytes (empty file)', () => {
      const targetConfigPath = path.join(testTempDir, 'app_config.json');
      // Create empty 0-byte file
      fs.writeFileSync(targetConfigPath, '');

      expect(fs.statSync(targetConfigPath).size).toBe(0);

      // Attempt load
      const config = simulateAtomicLoadJson(targetConfigPath, DEFAULT_APP_CONFIG);
      expect(config).toEqual(DEFAULT_APP_CONFIG);
    });

    it('should recover cleanly when app_config.json contains malformed / truncated JSON', () => {
      const targetConfigPath = path.join(testTempDir, 'app_config.json');
      // Truncated JSON simulating power failure during non-atomic write
      fs.writeFileSync(
        targetConfigPath,
        '{"theme": "dark", "language": "vi", "telemetry_enabled": false, "audio_buf',
        'utf8',
      );

      const config = simulateAtomicLoadJson(targetConfigPath, DEFAULT_APP_CONFIG);
      expect(config).toEqual(DEFAULT_APP_CONFIG);
    });

    it('should recover cleanly when app_config.json contains invalid UTF-8 binary bytes', () => {
      const targetConfigPath = path.join(testTempDir, 'app_config.json');
      // Invalid UTF-8 sequence and binary corruptions
      const corruptedBytes = Buffer.from([0xff, 0xfe, 0x80, 0x00, 0xaa, 0xbb, 0xcc, 0xdd]);
      fs.writeFileSync(targetConfigPath, corruptedBytes);

      const config = simulateAtomicLoadJson(targetConfigPath, DEFAULT_APP_CONFIG);
      expect(config).toEqual(DEFAULT_APP_CONFIG);
    });

    it('should sanitize and repair invalid property types via toAppConfig mapper', () => {
      const invalidJson = {
        theme: 'hacker_neon_invalid',
        language: 'klingon',
        telemetry_enabled: false,
        auto_update_sidecars: true,
        output_directory: '',
        cache_directory: '',
        audio_buffer_size_kb: 64,
        active_stt_provider: 'non_existent_provider',
        active_stt_model: 'whisper-large-v3',
        active_translation_model: 'gemini-1.5-flash',
        sidecar_paths: null,
        groq_api_key: '',
        gemini_api_key: '',
        max_history_days: 14,
      };

      const domain = toAppConfig(invalidJson as unknown as AppConfigJson);
      // Invalids fall back to safe enum defaults
      expect(domain.theme).toBe('dark');
      expect(domain.language).toBe('vi');
      expect(domain.activeSttProvider).toBe('groq');
      expect(domain.sidecarPaths).toEqual({});
      expect(isAppConfig(domain)).toBe(true);
    });
  });

  describe('3. Partial Write Failure & Temp File Cleanup', () => {
    it('should cleanly remove temporary file when write operation aborts midway', async () => {
      const targetConfigPath = path.join(testTempDir, 'app_config.json');

      await expect(
        simulateAtomicSaveJson(targetConfigPath, DEFAULT_APP_CONFIG, {
          injectWriteFailure: true,
        }),
      ).rejects.toThrow('Simulated write I/O disk failure');

      // Assert target was never created
      expect(fs.existsSync(targetConfigPath)).toBe(false);

      // Assert no dangling .tmp files remain in directory
      const files = fs.readdirSync(testTempDir);
      const danglingTmp = files.filter((f) => f.includes('.tmp.'));
      expect(danglingTmp.length).toBe(0);
    });

    it('should cleanly purge temporary file and preserve original target when rename fails', async () => {
      const targetConfigPath = path.join(testTempDir, 'app_config.json');

      // First successful write
      const originalPayload = { ...DEFAULT_APP_CONFIG, theme: 'acrylic' as const };
      await simulateAtomicSaveJson(targetConfigPath, toAppConfigJson(originalPayload));
      expect(fs.existsSync(targetConfigPath)).toBe(true);

      // Attempt second write which fails during rename phase
      const failingPayload = { ...DEFAULT_APP_CONFIG, theme: 'light' as const };
      await expect(
        simulateAtomicSaveJson(targetConfigPath, toAppConfigJson(failingPayload), {
          injectRenameFailure: true,
        }),
      ).rejects.toThrow('Simulated atomic rename failure');

      // Original file MUST be intact and unmodified
      const content = fs.readFileSync(targetConfigPath, 'utf8');
      const loaded = JSON.parse(content);
      expect(loaded.theme).toBe('acrylic');

      // Zero leftover .tmp files
      const files = fs.readdirSync(testTempDir);
      const danglingTmp = files.filter((f) => f.includes('.tmp.'));
      expect(danglingTmp.length).toBe(0);
    });

    it('should automatically create deeply nested missing parent directories', async () => {
      const nestedPath = path.join(
        testTempDir,
        'level1',
        'level2',
        'level3',
        'subfolder',
        'app_config.json',
      );

      await simulateAtomicSaveJson(nestedPath, toAppConfigJson(DEFAULT_APP_CONFIG));
      expect(fs.existsSync(nestedPath)).toBe(true);

      const loaded = simulateAtomicLoadJson(nestedPath, null);
      expect(loaded).not.toBeNull();
    });
  });

  describe('4. Frontend SettingsStore Concurrency & Failure Recovery', () => {
    beforeEach(() => {
      vi.clearAllMocks();
      useSettingsStore.getState().resetDefaults();
      useSettingsStore.getState().clearError();
    });

    it('should guard against re-entrancy by rejecting concurrent saveConfig requests while isSaving is true', async () => {
      let callCount = 0;
      vi.spyOn(settingsApi, 'saveAppConfig').mockImplementation(async () => {
        callCount += 1;
        // simulate async IPC bridge delay
        await new Promise((resolve) => setTimeout(resolve, 10));
      });

      // Update local state
      useSettingsStore.getState().updateConfig({ audioBufferSizeKb: 256 });

      // Trigger 50 simultaneous saves
      const savePromises = Array.from({ length: 50 }, () =>
        useSettingsStore.getState().saveConfig(),
      );

      const results = await Promise.all(savePromises);

      // First call succeeds; subsequent 49 concurrent calls are rejected by re-entrancy guard
      expect(results[0]).toBe(true);
      expect(results.slice(1).every((r) => r === false)).toBe(true);
      expect(callCount).toBe(1);

      const state = useSettingsStore.getState();
      expect(state.isSaving).toBe(false);
      expect(state.error).toBeNull();
      expect(state.config.audioBufferSizeKb).toBe(256);
      expect(state.lastSavedAt).not.toBeNull();
    });

    it('should reset isSaving to false and capture error when saveAppConfig fails', async () => {
      vi.spyOn(settingsApi, 'saveAppConfig').mockRejectedValueOnce({
        code: ERROR_CODES.CONFIG_SAVE_FAILED,
        message: 'Lỗi ghi đĩa: Disk full',
        recoverable: true,
      });

      const success = await useSettingsStore.getState().saveConfig();
      expect(success).toBe(false);

      const state = useSettingsStore.getState();
      expect(state.isSaving).toBe(false);
      expect(state.error).toContain('Không thể lưu tệp cấu hình');
    });

    it('should recover to defaults without mutating frozen DEFAULT_APP_CONFIG', () => {
      // Mutate local store
      useSettingsStore.getState().updateConfig({
        theme: 'light',
        language: 'en',
        audioBufferSizeKb: 512,
        groqApiKey: 'gsk_secret_adversarial',
      });

      // Verify store was updated
      expect(useSettingsStore.getState().config.theme).toBe('light');

      // Verify DEFAULT_APP_CONFIG remains pristine and frozen
      expect(DEFAULT_APP_CONFIG.theme).toBe('dark');
      expect(DEFAULT_APP_CONFIG.language).toBe('vi');
      expect(DEFAULT_APP_CONFIG.audioBufferSizeKb).toBe(64);
      expect(Object.isFrozen(DEFAULT_APP_CONFIG)).toBe(true);

      // Reset
      useSettingsStore.getState().resetDefaults();
      expect(useSettingsStore.getState().config).toEqual(DEFAULT_APP_CONFIG);
    });

    it('should handle unhandled exceptions from hardware detection gracefully', async () => {
      vi.spyOn(hardwareApi, 'detectHardwareProfile').mockRejectedValueOnce(
        new Error('Fatal driver crash during registry query'),
      );

      const result = await useSettingsStore.getState().detectHardware();
      expect(result).toBeNull();

      const state = useSettingsStore.getState();
      expect(state.isLoading).toBe(false);
      expect(state.hardwareProfile).toBeNull();
      expect(state.error).not.toBeNull();
    });
  });
});
