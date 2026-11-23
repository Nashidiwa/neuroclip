import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useHardwareStore, mapModeToSttProvider } from './useHardwareStore';
import * as hardwareApi from '../services/config/hardware';
import { useSettingsStore } from './settingsStore';
import type { HardwareProfile } from '../services/config/hardware';

vi.mock('../services/config/hardware', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../services/config/hardware')>();
  return {
    ...actual,
    fetchHardwareProfile: vi.fn(),
    detectHardwareProfile: vi.fn(),
  };
});

describe('useHardwareStore (TASK-M4-27)', () => {
  const mockHighEndProfile: HardwareProfile = {
    cpuInfo: { model: 'AMD Ryzen 9 7950X', cores: 16, threads: 32 },
    ramInfo: { totalMb: 32768, availableMb: 24576 },
    gpuInfo: { vendor: 'NVIDIA', model: 'GeForce RTX 4090', vramMb: 24576, cudaSupported: true },
    recommendedMode: 'offline_local',
    recommendationReason: 'GPU 24GB VRAM và CUDA hỗ trợ tốt mô hình lớn',
    timestamp: '2026-11-23T12:00:00.000Z',
  };

  const mockLowEndProfile: HardwareProfile = {
    cpuInfo: { model: 'Intel Core i5-1135G7', cores: 4, threads: 8 },
    ramInfo: { totalMb: 8192, availableMb: 4096 },
    gpuInfo: { vendor: 'Intel', model: 'Iris Xe Graphics', vramMb: 128, cudaSupported: false },
    recommendedMode: 'cloud_byok',
    recommendationReason: 'Máy tính văn phòng/iGPU nên dùng Cloud',
    timestamp: '2026-11-23T12:00:00.000Z',
  };

  beforeEach(() => {
    vi.clearAllMocks();
    useHardwareStore.getState().reset();
  });

  it('initializes with default empty state', () => {
    const state = useHardwareStore.getState();
    expect(state.profile).toBeNull();
    expect(state.selectedMode).toBeNull();
    expect(state.isOverridden).toBe(false);
    expect(state.isLoading).toBe(false);
    expect(state.error).toBeNull();
    expect(state.lastProbedAt).toBeNull();
  });

  it('fetches existing cached hardware profile successfully', async () => {
    vi.mocked(hardwareApi.fetchHardwareProfile).mockResolvedValueOnce(mockHighEndProfile);

    const result = await useHardwareStore.getState().fetchProfile();

    expect(result).toEqual(mockHighEndProfile);
    const state = useHardwareStore.getState();
    expect(state.profile).toEqual(mockHighEndProfile);
    expect(state.selectedMode).toBe('offline_local');
    expect(state.isOverridden).toBe(false);
    expect(state.isLoading).toBe(false);
    expect(state.error).toBeNull();
  });

  it('handles null when fetching cached profile that does not exist yet', async () => {
    vi.mocked(hardwareApi.fetchHardwareProfile).mockResolvedValueOnce(null);

    const result = await useHardwareStore.getState().fetchProfile();

    expect(result).toBeNull();
    const state = useHardwareStore.getState();
    expect(state.profile).toBeNull();
    expect(state.isLoading).toBe(false);
    expect(state.error).toBeNull();
  });

  it('triggers live hardware detection and updates state and selected mode', async () => {
    vi.mocked(hardwareApi.detectHardwareProfile).mockResolvedValueOnce(mockLowEndProfile);

    const result = await useHardwareStore.getState().detectHardware();

    expect(result).toEqual(mockLowEndProfile);
    const state = useHardwareStore.getState();
    expect(state.profile).toEqual(mockLowEndProfile);
    expect(state.selectedMode).toBe('cloud_byok');
    expect(state.isOverridden).toBe(false);
    expect(state.lastProbedAt).toBe('2026-11-23T12:00:00.000Z');
  });

  it('handles IPC errors gracefully on hardware detection failure', async () => {
    vi.mocked(hardwareApi.detectHardwareProfile).mockRejectedValueOnce(
      new Error('WMI Query Failed'),
    );

    const result = await useHardwareStore.getState().detectHardware();

    expect(result).toBeNull();
    const state = useHardwareStore.getState();
    expect(state.profile).toBeNull();
    expect(state.error).toContain('Không thể quét thông số phần cứng');
    expect(state.isLoading).toBe(false);
  });

  it('handles IPC errors gracefully on hardware profile fetch failure', async () => {
    vi.mocked(hardwareApi.fetchHardwareProfile).mockRejectedValueOnce(new Error('IPC Read Failed'));

    const result = await useHardwareStore.getState().fetchProfile();

    expect(result).toBeNull();
    const state = useHardwareStore.getState();
    expect(state.profile).toBeNull();
    expect(state.error).toContain('Không thể tải thông số phần cứng');
    expect(state.isLoading).toBe(false);
  });

  it('allows manual override of recommended mode and tracks isOverridden', async () => {
    vi.mocked(hardwareApi.fetchHardwareProfile).mockResolvedValueOnce(mockHighEndProfile);
    await useHardwareStore.getState().fetchProfile();

    expect(useHardwareStore.getState().selectedMode).toBe('offline_local');
    expect(useHardwareStore.getState().isOverridden).toBe(false);

    // Override to hybrid
    useHardwareStore.getState().selectModeOverride('hybrid');
    expect(useHardwareStore.getState().selectedMode).toBe('hybrid');
    expect(useHardwareStore.getState().isOverridden).toBe(true);

    // Re-select original recommended mode resets isOverridden
    useHardwareStore.getState().selectModeOverride('offline_local');
    expect(useHardwareStore.getState().isOverridden).toBe(false);
  });

  it('resets override back to recommended mode with resetOverride()', async () => {
    vi.mocked(hardwareApi.fetchHardwareProfile).mockResolvedValueOnce(mockHighEndProfile);
    await useHardwareStore.getState().fetchProfile();

    useHardwareStore.getState().selectModeOverride('cloud_byok');
    expect(useHardwareStore.getState().isOverridden).toBe(true);

    useHardwareStore.getState().resetOverride();
    expect(useHardwareStore.getState().selectedMode).toBe('offline_local');
    expect(useHardwareStore.getState().isOverridden).toBe(false);
  });

  it('synchronizes selected mode with useSettingsStore via applyToSettings', async () => {
    vi.mocked(hardwareApi.fetchHardwareProfile).mockResolvedValueOnce(mockHighEndProfile);
    await useHardwareStore.getState().fetchProfile();

    useHardwareStore.getState().applyToSettings('offline_local');
    expect(useSettingsStore.getState().config.activeSttProvider).toBe('local_whisper');

    useHardwareStore.getState().applyToSettings('hybrid');
    expect(useSettingsStore.getState().config.activeSttProvider).toBe('groq');

    useHardwareStore.getState().applyToSettings('cloud_byok');
    expect(useSettingsStore.getState().config.activeSttProvider).toBe('cloud_byok');
  });

  it('synchronizes current selectedMode when applyToSettings called with no arguments', async () => {
    vi.mocked(hardwareApi.fetchHardwareProfile).mockResolvedValueOnce(mockHighEndProfile);
    await useHardwareStore.getState().fetchProfile();

    useHardwareStore.getState().selectModeOverride('hybrid');
    useHardwareStore.getState().applyToSettings();
    expect(useSettingsStore.getState().config.activeSttProvider).toBe('groq');
  });

  it('clears error with clearError()', () => {
    useHardwareStore.setState({ error: 'Some hardware error' });
    expect(useHardwareStore.getState().error).toBe('Some hardware error');

    useHardwareStore.getState().clearError();
    expect(useHardwareStore.getState().error).toBeNull();
  });

  it('correctly maps recommended modes to STT providers', () => {
    expect(mapModeToSttProvider('offline_local')).toBe('local_whisper');
    expect(mapModeToSttProvider('hybrid')).toBe('groq');
    expect(mapModeToSttProvider('cloud_byok')).toBe('cloud_byok');
  });

  describe('Adversarial Remediations (Finding 1A & 1B)', () => {
    it('Finding 1A: prevents out-of-order race condition when slow fetchProfile resolves after fast detectHardware', async () => {
      vi.mocked(hardwareApi.fetchHardwareProfile).mockImplementation(
        () => new Promise((resolve) => setTimeout(() => resolve(mockLowEndProfile), 40)),
      );
      vi.mocked(hardwareApi.detectHardwareProfile).mockImplementation(
        () => new Promise((resolve) => setTimeout(() => resolve(mockHighEndProfile), 10)),
      );

      const fetchPromise = useHardwareStore.getState().fetchProfile();
      const detectPromise = useHardwareStore.getState().detectHardware();

      await Promise.all([fetchPromise, detectPromise]);

      const state = useHardwareStore.getState();
      expect(state.profile?.cpuInfo.model).toBe('AMD Ryzen 9 7950X');
      expect(state.profile?.recommendedMode).toBe('offline_local');
      expect(state.selectedMode).toBe('offline_local');
      expect(state.isLoading).toBe(false);
    });

    it('Finding 1A: ignores late IPC error from older superseded probe request', async () => {
      vi.mocked(hardwareApi.fetchHardwareProfile).mockImplementation(
        () =>
          new Promise((_, reject) => setTimeout(() => reject(new Error('Slow IPC failure')), 40)),
      );
      vi.mocked(hardwareApi.detectHardwareProfile).mockImplementation(
        () => new Promise((resolve) => setTimeout(() => resolve(mockHighEndProfile), 10)),
      );

      const fetchPromise = useHardwareStore.getState().fetchProfile();
      const detectPromise = useHardwareStore.getState().detectHardware();

      await Promise.all([fetchPromise, detectPromise]);

      const state = useHardwareStore.getState();
      expect(state.profile).toEqual(mockHighEndProfile);
      expect(state.error).toBeNull();
      expect(state.isLoading).toBe(false);
    });

    it('Finding 1B: preserves user manual mode override selected before initial profile fetch completes', async () => {
      vi.mocked(hardwareApi.fetchHardwareProfile).mockImplementation(
        () => new Promise((resolve) => setTimeout(() => resolve(mockHighEndProfile), 25)),
      );

      const fetchPromise = useHardwareStore.getState().fetchProfile();

      // User clicks mode option before fetch completes
      useHardwareStore.getState().selectModeOverride('hybrid');
      expect(useHardwareStore.getState().selectedMode).toBe('hybrid');
      expect(useHardwareStore.getState().isOverridden).toBe(true);

      await fetchPromise;

      const state = useHardwareStore.getState();
      expect(state.profile).toEqual(mockHighEndProfile);
      expect(state.selectedMode).toBe('hybrid');
      expect(state.isOverridden).toBe(true);
    });

    it('Finding 1B: preserves user manual mode override across detectHardware() re-probes', async () => {
      vi.mocked(hardwareApi.fetchHardwareProfile).mockResolvedValueOnce(mockHighEndProfile);
      await useHardwareStore.getState().fetchProfile();

      useHardwareStore.getState().selectModeOverride('cloud_byok');
      expect(useHardwareStore.getState().selectedMode).toBe('cloud_byok');
      expect(useHardwareStore.getState().isOverridden).toBe(true);

      // Re-probe hardware with fresh detection
      vi.mocked(hardwareApi.detectHardwareProfile).mockResolvedValueOnce(mockHighEndProfile);
      await useHardwareStore.getState().detectHardware();

      const state = useHardwareStore.getState();
      expect(state.profile).toEqual(mockHighEndProfile);
      expect(state.selectedMode).toBe('cloud_byok');
      expect(state.isOverridden).toBe(true);

      // User can still explicitly reset back to recommended
      useHardwareStore.getState().resetOverride();
      expect(useHardwareStore.getState().selectedMode).toBe('offline_local');
      expect(useHardwareStore.getState().isOverridden).toBe(false);
    });

    it('Finding 1B: normalizes isOverridden to false if user override matches profile recommendedMode upon arrival', async () => {
      vi.mocked(hardwareApi.fetchHardwareProfile).mockImplementation(
        () => new Promise((resolve) => setTimeout(() => resolve(mockHighEndProfile), 20)),
      );

      const fetchPromise = useHardwareStore.getState().fetchProfile();

      // User clicks offline_local while loading, which happens to match recommendedMode
      useHardwareStore.getState().selectModeOverride('offline_local');
      expect(useHardwareStore.getState().selectedMode).toBe('offline_local');
      expect(useHardwareStore.getState().isOverridden).toBe(true);

      await fetchPromise;

      const state = useHardwareStore.getState();
      expect(state.profile).toEqual(mockHighEndProfile);
      expect(state.selectedMode).toBe('offline_local');
      expect(state.isOverridden).toBe(false);
    });
  });
});
