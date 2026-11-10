import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useSettingsStore, DEFAULT_APP_CONFIG } from '../stores/settingsStore';
import * as settingsApi from '../services/config/settings';
import * as hardwareApi from '../services/config/hardware';
import { ERROR_CODES } from '../services/ipc/error';
import type { HardwareProfile } from '../services/config/hardware';

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

describe('TASK-M4-03: Settings Store State Transitions (UT-03.5)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useSettingsStore.getState().resetDefaults();
    useSettingsStore.getState().clearError();
  });

  it('should initialize with default configuration and idle state', () => {
    const state = useSettingsStore.getState();
    expect(state.config).toEqual(DEFAULT_APP_CONFIG);
    expect(state.hardwareProfile).toBeNull();
    expect(state.isLoading).toBe(false);
    expect(state.isSaving).toBe(false);
    expect(state.error).toBeNull();
  });

  it('should load remote config successfully', async () => {
    const mockConfig = {
      ...DEFAULT_APP_CONFIG,
      theme: 'light' as const,
      language: 'en' as const,
      audioBufferSizeKb: 128,
    };
    vi.mocked(settingsApi.fetchAppConfig).mockResolvedValueOnce(mockConfig);

    await useSettingsStore.getState().loadConfig();

    const state = useSettingsStore.getState();
    expect(state.config.theme).toBe('light');
    expect(state.config.language).toBe('en');
    expect(state.config.audioBufferSizeKb).toBe(128);
    expect(state.isLoading).toBe(false);
    expect(state.error).toBeNull();
  });

  it('should handle load remote config error gracefully', async () => {
    vi.mocked(settingsApi.fetchAppConfig).mockRejectedValueOnce({
      code: ERROR_CODES.CONFIG_NOT_FOUND,
      message: 'Không tìm thấy tệp cấu hình',
      recoverable: true,
    });

    await useSettingsStore.getState().loadConfig();

    const state = useSettingsStore.getState();
    expect(state.error).toContain('Không tìm thấy tệp cấu hình');
    expect(state.isLoading).toBe(false);
  });

  it('should update local config reactively with optimistic updates', () => {
    useSettingsStore.getState().updateConfig({
      audioBufferSizeKb: 256,
      theme: 'acrylic',
    });

    const state = useSettingsStore.getState();
    expect(state.config.audioBufferSizeKb).toBe(256);
    expect(state.config.theme).toBe('acrylic');
    expect(state.config.language).toBe('vi'); // untouched
  });

  it('should save config via IPC successfully', async () => {
    vi.mocked(settingsApi.saveAppConfig).mockResolvedValueOnce(undefined);

    const success = await useSettingsStore.getState().saveConfig();

    expect(success).toBe(true);
    const state = useSettingsStore.getState();
    expect(state.isSaving).toBe(false);
    expect(state.lastSavedAt).not.toBeNull();
    expect(state.error).toBeNull();
    expect(settingsApi.saveAppConfig).toHaveBeenCalledWith(state.config);
  });

  it('should handle save config failure and record friendly error', async () => {
    vi.mocked(settingsApi.saveAppConfig).mockRejectedValueOnce({
      code: ERROR_CODES.CONFIG_SAVE_FAILED,
      message: 'Không thể lưu tệp cấu hình',
      details: 'Permission denied',
      recoverable: true,
    });

    const success = await useSettingsStore.getState().saveConfig();

    expect(success).toBe(false);
    const state = useSettingsStore.getState();
    expect(state.isSaving).toBe(false);
    expect(state.error).toContain('Không thể lưu');
  });

  it('should load cached hardware profile', async () => {
    const mockProfile: HardwareProfile = {
      cpuInfo: { model: 'AMD Ryzen 7 5700X', cores: 8, threads: 16 },
      ramInfo: { totalMb: 32768, availableMb: 24000 },
      gpuInfo: { vendor: 'NVIDIA', model: 'GeForce RTX 3070', vramMb: 8192, cudaSupported: true },
      recommendedMode: 'offline_local',
      recommendationReason: '8GB VRAM và CUDA',
      timestamp: '2026-11-09T20:00:00Z',
    };
    vi.mocked(hardwareApi.fetchHardwareProfile).mockResolvedValueOnce(mockProfile);

    await useSettingsStore.getState().loadHardwareProfile();

    const state = useSettingsStore.getState();
    expect(state.hardwareProfile).toEqual(mockProfile);
    expect(state.isLoading).toBe(false);
  });

  it('should detect hardware and update profile', async () => {
    const detectedProfile: HardwareProfile = {
      cpuInfo: { model: 'i5-1135G7', cores: 4, threads: 8 },
      ramInfo: { totalMb: 8192, availableMb: 4000 },
      gpuInfo: { vendor: 'Intel', model: 'Iris Xe', vramMb: 0, cudaSupported: false },
      recommendedMode: 'cloud_byok',
      recommendationReason: 'iGPU văn phòng',
      timestamp: '2026-11-09T20:00:00Z',
    };
    vi.mocked(hardwareApi.detectHardwareProfile).mockResolvedValueOnce(detectedProfile);

    const result = await useSettingsStore.getState().detectHardware();

    expect(result).toEqual(detectedProfile);
    const state = useSettingsStore.getState();
    expect(state.hardwareProfile).toEqual(detectedProfile);
    expect(state.isLoading).toBe(false);
  });

  it('should handle detect hardware failure', async () => {
    vi.mocked(hardwareApi.detectHardwareProfile).mockRejectedValueOnce({
      code: ERROR_CODES.HARDWARE_PROBE_FAILED,
      message: 'Không thể đánh giá thông số phần cứng',
      recoverable: true,
    });

    const result = await useSettingsStore.getState().detectHardware();

    expect(result).toBeNull();
    const state = useSettingsStore.getState();
    expect(state.hardwareProfile).toBeNull();
    expect(state.error).toContain('Không thể đánh giá');
  });

  it('should reset defaults cleanly', () => {
    useSettingsStore.getState().updateConfig({
      theme: 'light',
      audioBufferSizeKb: 1024,
      groqApiKey: 'gsk_test_123',
    });

    useSettingsStore.getState().resetDefaults();

    const state = useSettingsStore.getState();
    expect(state.config).toEqual(DEFAULT_APP_CONFIG);
  });

  it('should clear error cleanly', () => {
    useSettingsStore.setState({ error: 'Lỗi bất kỳ' });
    expect(useSettingsStore.getState().error).toBe('Lỗi bất kỳ');

    useSettingsStore.getState().clearError();
    expect(useSettingsStore.getState().error).toBeNull();
  });

  it('should preserve existing sidecar paths when partially updating sidecarPaths (deep merge)', () => {
    // 1. Pre-populate store with complete sidecar configuration
    useSettingsStore.getState().updateConfig({
      sidecarPaths: {
        ffmpegPath: 'bin/ffmpeg.exe',
        ytdlpPath: 'bin/yt-dlp.exe',
        whisperWorkerPath: 'bin/whisper-worker.exe',
      },
    });

    const before = useSettingsStore.getState().config.sidecarPaths;
    expect(before.ffmpegPath).toBe('bin/ffmpeg.exe');
    expect(before.ytdlpPath).toBe('bin/yt-dlp.exe');
    expect(before.whisperWorkerPath).toBe('bin/whisper-worker.exe');

    // 2. Partially update only ffmpegPath
    useSettingsStore.getState().updateConfig({
      sidecarPaths: {
        ffmpegPath: 'custom/ffmpeg.exe',
      },
    });

    // 3. Verify ffmpegPath is updated while ytdlpPath and whisperWorkerPath remain intact
    const after = useSettingsStore.getState().config.sidecarPaths;
    expect(after.ffmpegPath).toBe('custom/ffmpeg.exe');
    expect(after.ytdlpPath).toBe('bin/yt-dlp.exe');
    expect(after.whisperWorkerPath).toBe('bin/whisper-worker.exe');
  });

  it('should prevent re-entrant saveConfig calls when isSaving is already true', async () => {
    let resolveSave: () => void = () => {};
    const deferredPromise = new Promise<void>((resolve) => {
      resolveSave = resolve;
    });

    vi.mocked(settingsApi.saveAppConfig).mockImplementationOnce(() => deferredPromise);

    // Initial save call enters in-flight state
    const firstSavePromise = useSettingsStore.getState().saveConfig();
    expect(useSettingsStore.getState().isSaving).toBe(true);

    // Concurrent second save call while isSaving is true must be rejected immediately
    const secondSaveResult = await useSettingsStore.getState().saveConfig();
    expect(secondSaveResult).toBe(false);

    // Verify IPC save was only triggered once
    expect(settingsApi.saveAppConfig).toHaveBeenCalledTimes(1);

    // Resolve in-flight save operation
    resolveSave();
    const firstSaveResult = await firstSavePromise;
    expect(firstSaveResult).toBe(true);
    expect(useSettingsStore.getState().isSaving).toBe(false);
  });
});
