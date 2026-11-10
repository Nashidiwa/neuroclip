import { describe, it, expect, vi, beforeEach } from 'vitest';
import Ajv from 'ajv';
import {
  type HardwareProfileJson,
  type GpuVendor,
  type RecommendedMode,
  toHardwareProfile,
  toHardwareProfileJson,
  isHardwareProfileJson,
  isHardwareProfile,
  isCpuInfoJson,
  isRamInfoJson,
  isGpuInfoJson,
  isGpuVendor,
  isRecommendedMode,
} from '../services/config/hardware';
import { HARDWARE_PROFILE_SCHEMA_5_1 } from './hardwareSchema.test';
import { useSettingsStore, DEFAULT_APP_CONFIG } from '../stores/settingsStore';
import * as settingsApi from '../services/config/settings';
import * as hardwareApi from '../services/config/hardware';
import { ERROR_CODES } from '../services/ipc/error';

vi.mock('../services/config/settings', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../services/config/settings')>();
  return {
    ...actual,
    fetchAppConfig: vi.fn(),
    saveAppConfig: vi.fn(),
  };
});

vi.mock('../services/config/hardware', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../services/config/hardware')>();
  return {
    ...actual,
    fetchHardwareProfile: vi.fn(),
    detectHardwareProfile: vi.fn(),
  };
});

/**
 * Pure heuristic recommendation evaluator matching the Rust backend
 * `evaluate_recommended_mode` in `src-tauri/src/config/hardware.rs`.
 */
function evaluateRecommendationHeuristic(
  ramMb: number,
  gpuVendor: GpuVendor,
  vramMb: number,
  cudaSupported: boolean,
): { mode: RecommendedMode; reason: string } {
  // 1. RAM < 8GB
  if (ramMb < 8192) {
    return {
      mode: 'cloud_byok',
      reason: 'RAM hệ thống < 8GB không đủ đáp ứng mô hình cục bộ',
    };
  }

  // 2. Non-CUDA / AMD / Unknown
  if (!cudaSupported || gpuVendor === 'AMD' || gpuVendor === 'Unknown') {
    if (gpuVendor === 'AMD') {
      return {
        mode: 'cloud_byok',
        reason: 'GPU AMD Radeon chưa hỗ trợ tăng tốc CUDA',
      };
    }
    return {
      mode: 'cloud_byok',
      reason: 'Cấu hình văn phòng/iGPU không có CUDA',
    };
  }

  // 3. Intel iGPU
  if (gpuVendor === 'Intel') {
    return {
      mode: 'cloud_byok',
      reason: 'Cấu hình iGPU Intel',
    };
  }

  // 4. Low dedicated VRAM (< 3000 MB)
  if (vramMb < 3000) {
    return {
      mode: 'cloud_byok',
      reason: 'VRAM card đồ họa < 3GB không đủ đáp ứng mô hình Whisper cục bộ',
    };
  }

  // 5. NVIDIA CUDA >= 8000 MB VRAM
  if (vramMb >= 8000) {
    if (ramMb >= 16384) {
      return {
        mode: 'offline_local',
        reason: 'GPU NVIDIA mạnh mẽ hỗ trợ CUDA và RAM >= 16GB',
      };
    }
    return {
      mode: 'hybrid',
      reason: 'GPU VRAM >= 8GB nhưng RAM hệ thống < 16GB',
    };
  }

  // 6. Mid-range NVIDIA GPU (3000 MB - 7999 MB VRAM)
  return {
    mode: 'hybrid',
    reason: 'GPU tầm trung hỗ trợ CUDA',
  };
}

describe('TASK-M4-03: Adversarial Challenge Suite', () => {
  const ajv = new Ajv({ allErrors: true, format: 'full' });
  const validateSchema = ajv.compile(HARDWARE_PROFILE_SCHEMA_5_1);

  // =========================================================================
  // 1. SCHEMA 5.1 ADVERSARIAL BOUNDARY VALIDATION
  // =========================================================================
  describe('1. Schema 5.1 Draft-07 Adversarial Boundaries', () => {
    const validBase: HardwareProfileJson = {
      cpu_info: { model: 'AMD Ryzen 7 7800X3D', cores: 8, threads: 16 },
      ram_info: { total_mb: 32768, available_mb: 24000 },
      gpu_info: {
        vendor: 'NVIDIA',
        model: 'GeForce RTX 4080',
        vram_mb: 16384,
        cuda_supported: true,
      },
      recommended_mode: 'offline_local',
      recommendation_reason: '16GB VRAM CUDA + 32GB RAM',
      timestamp: '2026-11-09T20:30:00.000Z',
    };

    it('should validate baseline compliant profile', () => {
      expect(validateSchema(validBase)).toBe(true);
      expect(isHardwareProfileJson(validBase)).toBe(true);
    });

    describe('1.1 Negative and Non-Integer Numerical Attacks', () => {
      it('should reject float values for cores, threads, total_mb, available_mb, vram_mb', () => {
        // Cores as float
        const floatCores = {
          ...validBase,
          cpu_info: { ...validBase.cpu_info, cores: 8.5 },
        };
        expect(validateSchema(floatCores)).toBe(false);
        expect(isCpuInfoJson(floatCores.cpu_info)).toBe(false);

        // Threads as float
        const floatThreads = {
          ...validBase,
          cpu_info: { ...validBase.cpu_info, threads: 15.9 },
        };
        expect(validateSchema(floatThreads)).toBe(false);
        expect(isCpuInfoJson(floatThreads.cpu_info)).toBe(false);

        // RAM total_mb as float
        const floatRam = {
          ...validBase,
          ram_info: { ...validBase.ram_info, total_mb: 32768.12 },
        };
        expect(validateSchema(floatRam)).toBe(false);
        expect(isRamInfoJson(floatRam.ram_info)).toBe(false);

        // VRAM as float
        const floatVram = {
          ...validBase,
          gpu_info: { ...validBase.gpu_info, vram_mb: 8192.5 },
        };
        expect(validateSchema(floatVram)).toBe(false);
        expect(isGpuInfoJson(floatVram.gpu_info)).toBe(false);
      });

      it('should reject negative values for cores and threads in type guards', () => {
        const negativeCores = {
          ...validBase,
          cpu_info: { ...validBase.cpu_info, cores: -4 },
        };
        expect(isCpuInfoJson(negativeCores.cpu_info)).toBe(false);

        const zeroCores = {
          ...validBase,
          cpu_info: { ...validBase.cpu_info, cores: 0 },
        };
        expect(isCpuInfoJson(zeroCores.cpu_info)).toBe(false);

        const negativeThreads = {
          ...validBase,
          cpu_info: { ...validBase.cpu_info, threads: -16 },
        };
        expect(isCpuInfoJson(negativeThreads.cpu_info)).toBe(false);

        const negativeRam = {
          ...validBase,
          ram_info: { ...validBase.ram_info, total_mb: -1024 },
        };
        expect(isRamInfoJson(negativeRam.ram_info)).toBe(false);

        const negativeVram = {
          ...validBase,
          gpu_info: { ...validBase.gpu_info, vram_mb: -1 },
        };
        expect(isGpuInfoJson(negativeVram.gpu_info)).toBe(false);
      });

      it('should reject NaN and Infinity in numbers via runtime type guards and JSON parsing', () => {
        const nanVram = {
          ...validBase,
          gpu_info: { ...validBase.gpu_info, vram_mb: Number.NaN },
        };
        expect(validateSchema(nanVram)).toBe(false);
        expect(isGpuInfoJson(nanVram.gpu_info)).toBe(false);

        const infCores = {
          ...validBase,
          cpu_info: { ...validBase.cpu_info, cores: Number.POSITIVE_INFINITY },
        };
        // Runtime guard strictly rejects Infinity
        expect(isCpuInfoJson(infCores.cpu_info)).toBe(false);
        // Wire serialization converts Infinity to null which Ajv strictly rejects
        const wirePayload = JSON.parse(JSON.stringify(infCores));
        expect(validateSchema(wirePayload)).toBe(false);
      });
    });

    describe('1.2 String Coercion and Type Mismatch Attacks', () => {
      it('should reject string numbers for integer properties', () => {
        const stringNumbers = {
          ...validBase,
          cpu_info: { model: 'AMD', cores: '8', threads: '16' },
          ram_info: { total_mb: '32768', available_mb: '24000' },
          gpu_info: { vendor: 'NVIDIA', model: 'RTX', vram_mb: '16384', cuda_supported: 'true' },
        };
        expect(validateSchema(stringNumbers)).toBe(false);
        expect(isHardwareProfileJson(stringNumbers)).toBe(false);
      });

      it('should reject non-boolean cuda_supported', () => {
        const truthyCuda = {
          ...validBase,
          gpu_info: { ...validBase.gpu_info, cuda_supported: 1 },
        };
        expect(validateSchema(truthyCuda)).toBe(false);
        expect(isGpuInfoJson(truthyCuda.gpu_info)).toBe(false);
      });
    });

    describe('1.3 Enumeration Bounds Attacks', () => {
      it('should reject case-sensitive mismatch or unauthorized vendor names', () => {
        const unauthorizedVendors = [
          'nvidia',
          'Nvidia',
          'amd',
          'AMD_RADEON',
          'intel',
          'Apple',
          'Qualcomm',
          'ARM',
          'Broadcom',
          '',
          ' ',
          'UNKNOWN',
        ];

        for (const vendor of unauthorizedVendors) {
          const badVendor = {
            ...validBase,
            gpu_info: { ...validBase.gpu_info, vendor },
          };
          expect(validateSchema(badVendor)).toBe(false);
          expect(isGpuVendor(vendor)).toBe(false);
        }
      });

      it('should reject invalid recommended_mode enum variations', () => {
        const invalidModes = [
          'offline',
          'local',
          'cloud',
          'CLOUD_BYOK',
          'Offline_Local',
          'Hybrid',
          'byok',
          'online',
          '',
        ];

        for (const mode of invalidModes) {
          const badMode = {
            ...validBase,
            recommended_mode: mode,
          };
          expect(validateSchema(badMode)).toBe(false);
          expect(isRecommendedMode(mode)).toBe(false);
        }
      });
    });

    describe('1.4 Timestamp Format & RFC 3339 Attacks', () => {
      it('should reject invalid, legacy or non-date-time timestamps', () => {
        const invalidTimestamps = [
          '2026/11/09 20:30:00',
          '09-11-2026T20:30:00Z',
          'Mon Nov 09 2026',
          '1731182400000',
          '2026-11-09',
          '2026-13-45T99:99:99Z',
          'invalid-iso-string',
          '',
        ];

        for (const ts of invalidTimestamps) {
          const badTs = {
            ...validBase,
            timestamp: ts,
          };
          expect(validateSchema(badTs)).toBe(false);
        }
      });

      it('should accept valid RFC 3339 offsets and millisecond representations', () => {
        const validTimestamps = [
          '2026-11-09T20:30:00Z',
          '2026-11-09T20:30:00.123Z',
          '2026-11-09T20:30:00+07:00',
          '2026-11-09T13:30:00-00:00',
        ];

        for (const ts of validTimestamps) {
          const goodTs = {
            ...validBase,
            timestamp: ts,
          };
          expect(validateSchema(goodTs)).toBe(true);
        }
      });
    });

    describe('1.5 Null / Undefined / Primitive Injections', () => {
      it('should safely return false for arbitrary non-object root inputs in type guards', () => {
        const nonObjects = [null, undefined, 42, 'string', true, Symbol('test'), () => {}, []];
        for (const item of nonObjects) {
          expect(isHardwareProfileJson(item)).toBe(false);
          expect(isHardwareProfile(item)).toBe(false);
          expect(isCpuInfoJson(item)).toBe(false);
          expect(isRamInfoJson(item)).toBe(false);
          expect(isGpuInfoJson(item)).toBe(false);
        }
      });

      it('should reject null for required sub-objects in Ajv and type guards', () => {
        expect(validateSchema({ ...validBase, cpu_info: null })).toBe(false);
        expect(validateSchema({ ...validBase, ram_info: null })).toBe(false);
        expect(validateSchema({ ...validBase, gpu_info: null })).toBe(false);

        expect(isHardwareProfileJson({ ...validBase, cpu_info: null })).toBe(false);
        expect(isHardwareProfileJson({ ...validBase, ram_info: null })).toBe(false);
        expect(isHardwareProfileJson({ ...validBase, gpu_info: null })).toBe(false);
      });
    });

    describe('1.6 Bidirectional Converter Edge Cases', () => {
      it('should handle optional recommendation_reason when omitted, empty, or populated', () => {
        // Without reason
        const noReason: HardwareProfileJson = {
          cpu_info: { model: 'Intel', cores: 4, threads: 8 },
          ram_info: { total_mb: 16384, available_mb: 8000 },
          gpu_info: { vendor: 'Intel', model: 'Iris', vram_mb: 0, cuda_supported: false },
          recommended_mode: 'cloud_byok',
          timestamp: '2026-11-09T20:00:00Z',
        };
        const domain1 = toHardwareProfile(noReason);
        expect(domain1.recommendationReason).toBeUndefined();
        const wire1 = toHardwareProfileJson(domain1);
        expect(wire1.recommendation_reason).toBeUndefined();

        // With reason
        const withReason: HardwareProfileJson = {
          ...noReason,
          recommendation_reason: 'iGPU',
        };
        const domain2 = toHardwareProfile(withReason);
        expect(domain2.recommendationReason).toBe('iGPU');
        const wire2 = toHardwareProfileJson(domain2);
        expect(wire2.recommendation_reason).toBe('iGPU');
      });

      it('should idempotently return domain object if already in camelCase', () => {
        const domain = toHardwareProfile(validBase);
        const doubleConverted = toHardwareProfile(domain);
        expect(doubleConverted).toBe(domain);
      });
    });
  });

  // =========================================================================
  // 2. RECOMMENDATION HEURISTICS BOUNDARY & STRESS TESTING
  // =========================================================================
  describe('2. Recommendation Heuristics Boundary Matrix', () => {
    describe('2.1 Exact VRAM Cliffs (2999MB vs 3000MB, 7999MB vs 8000MB, 0MB)', () => {
      it('should classify 0 VRAM as cloud_byok', () => {
        const res = evaluateRecommendationHeuristic(16384, 'NVIDIA', 0, true);
        expect(res.mode).toBe('cloud_byok');
        expect(res.reason).toContain('< 3GB');
      });

      it('should classify 2999MB VRAM as cloud_byok (strictly below 3GB threshold)', () => {
        const res = evaluateRecommendationHeuristic(16384, 'NVIDIA', 2999, true);
        expect(res.mode).toBe('cloud_byok');
        expect(res.reason).toContain('< 3GB');
      });

      it('should classify 3000MB VRAM as hybrid (exact lower boundary of mid-range tier)', () => {
        const res = evaluateRecommendationHeuristic(16384, 'NVIDIA', 3000, true);
        expect(res.mode).toBe('hybrid');
        expect(res.reason).toContain('tầm trung');
      });

      it('should classify 7999MB VRAM as hybrid (strictly below 8GB threshold for local whisper)', () => {
        const res = evaluateRecommendationHeuristic(32768, 'NVIDIA', 7999, true);
        expect(res.mode).toBe('hybrid');
        expect(res.reason).toContain('tầm trung');
      });

      it('should classify 8000MB VRAM + 16384MB RAM as offline_local (exact lower boundary of local tier)', () => {
        const res = evaluateRecommendationHeuristic(16384, 'NVIDIA', 8000, true);
        expect(res.mode).toBe('offline_local');
        expect(res.reason).toContain('NVIDIA');
      });
    });

    describe('2.2 Exact RAM Cliffs (8191MB vs 8192MB, 16383MB vs 16384MB)', () => {
      it('should classify 8191MB RAM with high-end RTX 4090 24GB VRAM as cloud_byok (system RAM constraint)', () => {
        const res = evaluateRecommendationHeuristic(8191, 'NVIDIA', 24576, true);
        expect(res.mode).toBe('cloud_byok');
        expect(res.reason).toContain('RAM hệ thống < 8GB');
      });

      it('should classify 8192MB RAM with high-end RTX 4090 24GB VRAM as hybrid (RAM bottleneck)', () => {
        const res = evaluateRecommendationHeuristic(8192, 'NVIDIA', 24576, true);
        expect(res.mode).toBe('hybrid');
        expect(res.reason).toContain('RAM hệ thống < 16GB');
      });

      it('should classify 16383MB RAM with high-end RTX 4090 as hybrid (just under 16GB threshold)', () => {
        const res = evaluateRecommendationHeuristic(16383, 'NVIDIA', 24576, true);
        expect(res.mode).toBe('hybrid');
        expect(res.reason).toContain('RAM hệ thống < 16GB');
      });

      it('should classify 16384MB RAM with high-end RTX 4090 as offline_local', () => {
        const res = evaluateRecommendationHeuristic(16384, 'NVIDIA', 24576, true);
        expect(res.mode).toBe('offline_local');
      });
    });

    describe('2.3 Non-NVIDIA and Non-CUDA GPUs with Massive VRAM', () => {
      it('should classify AMD Radeon RX 7900 XTX (24GB VRAM) as cloud_byok due to lack of CUDA', () => {
        const res = evaluateRecommendationHeuristic(65536, 'AMD', 24576, false);
        expect(res.mode).toBe('cloud_byok');
        expect(res.reason).toContain('AMD Radeon');
      });

      it('should classify Intel Arc A770 (16GB VRAM) as cloud_byok', () => {
        const res = evaluateRecommendationHeuristic(32768, 'Intel', 16384, false);
        expect(res.mode).toBe('cloud_byok');
        expect(res.reason).toContain('không có CUDA');
      });

      it('should classify Unknown GPU (64GB VRAM) as cloud_byok', () => {
        const res = evaluateRecommendationHeuristic(131072, 'Unknown', 65536, false);
        expect(res.mode).toBe('cloud_byok');
        expect(res.reason).toContain('văn phòng/iGPU');
      });

      it('should classify NVIDIA GPU without CUDA support (driver disabled or legacy) as cloud_byok', () => {
        const res = evaluateRecommendationHeuristic(16384, 'NVIDIA', 8192, false);
        expect(res.mode).toBe('cloud_byok');
        expect(res.reason).toContain('không có CUDA');
      });
    });
  });

  // =========================================================================
  // 3. FRONTEND STORE STATE TRANSITIONS & RACE CONDITIONS
  // =========================================================================
  describe('3. Settings Store State Transitions Stress Tests', () => {
    beforeEach(() => {
      vi.clearAllMocks();
      useSettingsStore.getState().resetDefaults();
      useSettingsStore.getState().clearError();
    });

    it('should withstand 100 rapid sequential synchronous updateConfig calls without state corruption', () => {
      for (let i = 1; i <= 100; i += 1) {
        useSettingsStore.getState().updateConfig({
          audioBufferSizeKb: i * 64,
          maxHistoryDays: i % 30,
        });
      }

      const state = useSettingsStore.getState();
      expect(state.config.audioBufferSizeKb).toBe(100 * 64);
      expect(state.config.maxHistoryDays).toBe(100 % 30);
      expect(state.config.theme).toBe('dark'); // Preserved
      expect(state.error).toBeNull();
    });

    it('should correctly notify multiple subscribers and clean up on unsubscribe', () => {
      const listenerA = vi.fn();
      const listenerB = vi.fn();

      const unsubA = useSettingsStore.subscribe(listenerA);
      const unsubB = useSettingsStore.subscribe(listenerB);

      useSettingsStore.getState().updateConfig({ theme: 'light' });
      expect(listenerA).toHaveBeenCalledTimes(1);
      expect(listenerB).toHaveBeenCalledTimes(1);

      // Unsubscribe A
      unsubA();

      useSettingsStore.getState().updateConfig({ theme: 'acrylic' });
      expect(listenerA).toHaveBeenCalledTimes(1); // Not called again
      expect(listenerB).toHaveBeenCalledTimes(2); // Called again

      // Unsubscribe B
      unsubB();
    });

    it('should preserve optimistic updates when background saveConfig completes', async () => {
      let resolveSave: () => void = () => {};
      const savePromise = new Promise<void>((resolve) => {
        resolveSave = resolve;
      });
      vi.mocked(settingsApi.saveAppConfig).mockReturnValueOnce(savePromise);

      // Initial state
      useSettingsStore.getState().updateConfig({ theme: 'light' });

      // Trigger save (which hangs until resolveSave)
      const saveActionPromise = useSettingsStore.getState().saveConfig();
      expect(useSettingsStore.getState().isSaving).toBe(true);

      // While saving is pending, user modifies config further:
      useSettingsStore.getState().updateConfig({
        theme: 'acrylic',
        audioBufferSizeKb: 512,
      });

      expect(useSettingsStore.getState().config.theme).toBe('acrylic');
      expect(useSettingsStore.getState().config.audioBufferSizeKb).toBe(512);

      // Now resolve save
      resolveSave();
      const saveResult = await saveActionPromise;

      expect(saveResult).toBe(true);
      const finalState = useSettingsStore.getState();
      expect(finalState.isSaving).toBe(false);
      expect(finalState.lastSavedAt).not.toBeNull();
      // Crucial: The optimistic update made while save was pending must NOT be wiped!
      expect(finalState.config.theme).toBe('acrylic');
      expect(finalState.config.audioBufferSizeKb).toBe(512);
    });

    it('should handle concurrent saveConfig calls safely via re-entrancy guard', async () => {
      vi.mocked(settingsApi.saveAppConfig).mockResolvedValue(undefined);

      const [res1, res2] = await Promise.all([
        useSettingsStore.getState().saveConfig(),
        useSettingsStore.getState().saveConfig(),
      ]);

      expect(res1).toBe(true);
      expect(res2).toBe(false); // Second concurrent call safely rejected by re-entrancy guard
      expect(useSettingsStore.getState().isSaving).toBe(false);
      expect(useSettingsStore.getState().error).toBeNull();
    });

    it('should handle loadConfig IPC error rejection without corrupting local config', async () => {
      useSettingsStore.getState().updateConfig({ theme: 'light', audioBufferSizeKb: 256 });
      const currentConfig = { ...useSettingsStore.getState().config };

      vi.mocked(settingsApi.fetchAppConfig).mockRejectedValueOnce({
        code: ERROR_CODES.CONFIG_NOT_FOUND,
        message: 'Tệp cấu hình không tồn tại',
        recoverable: true,
      });

      await useSettingsStore.getState().loadConfig();

      const state = useSettingsStore.getState();
      expect(state.isLoading).toBe(false);
      expect(state.error).toContain('Không tìm thấy tệp cấu hình');
      // Existing config is preserved
      expect(state.config).toEqual(currentConfig);
    });

    it('should handle detectHardware IPC rejection and clear hardware profile safely', async () => {
      vi.mocked(hardwareApi.detectHardwareProfile).mockRejectedValueOnce(
        new Error('WMI Registry query timed out'),
      );

      const result = await useSettingsStore.getState().detectHardware();

      expect(result).toBeNull();
      const state = useSettingsStore.getState();
      expect(state.hardwareProfile).toBeNull();
      expect(state.isLoading).toBe(false);
      expect(state.error).toBeTruthy();
    });

    it('should preserve detected hardware profile when user resets config to defaults', () => {
      const mockProfile = {
        cpuInfo: { model: 'i9', cores: 16, threads: 32 },
        ramInfo: { totalMb: 65536, availableMb: 48000 },
        gpuInfo: { vendor: 'NVIDIA' as const, model: 'RTX 4090', vramMb: 24576, cudaSupported: true },
        recommendedMode: 'offline_local' as const,
        timestamp: '2026-11-09T20:00:00Z',
      };
      useSettingsStore.setState({ hardwareProfile: mockProfile });
      useSettingsStore.getState().updateConfig({ theme: 'light', audioBufferSizeKb: 1024 });

      useSettingsStore.getState().resetDefaults();

      const state = useSettingsStore.getState();
      expect(state.config).toEqual(DEFAULT_APP_CONFIG);
      expect(state.hardwareProfile).toEqual(mockProfile); // Profile is preserved
      expect(state.error).toBeNull();
    });

    it('should support functional setState updates correctly', () => {
      useSettingsStore.setState((prev) => ({
        config: {
          ...prev.config,
          audioBufferSizeKb: 128,
        },
      }));

      expect(useSettingsStore.getState().config.audioBufferSizeKb).toBe(128);
    });
  });
});
