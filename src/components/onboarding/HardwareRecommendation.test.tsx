import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { HardwareRecommendation } from './HardwareRecommendation';
import { useHardwareStore } from '../../stores/useHardwareStore';
import * as hardwareApi from '../../services/config/hardware';
import type { HardwareProfile } from '../../services/config/hardware';

vi.mock('../../services/config/hardware', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../services/config/hardware')>();
  return {
    ...actual,
    fetchHardwareProfile: vi.fn(),
    detectHardwareProfile: vi.fn(),
  };
});

describe('HardwareRecommendation (TASK-M4-27)', () => {
  const mockNvidiaProfile: HardwareProfile = {
    cpuInfo: { model: 'AMD Ryzen 7 5800X', cores: 8, threads: 16 },
    ramInfo: { totalMb: 16384, availableMb: 12000 },
    gpuInfo: { vendor: 'NVIDIA', model: 'GeForce RTX 3070', vramMb: 8192, cudaSupported: true },
    recommendedMode: 'offline_local',
    recommendationReason: 'GPU NVIDIA 8GB VRAM hỗ trợ CUDA và RAM 16GB',
    timestamp: '2026-11-23T12:00:00Z',
  };

  const mockIrisProfile: HardwareProfile = {
    cpuInfo: { model: 'Intel Core i5-1135G7', cores: 4, threads: 8 },
    ramInfo: { totalMb: 8192, availableMb: 4000 },
    gpuInfo: { vendor: 'Intel', model: 'Intel Iris Xe', vramMb: 128, cudaSupported: false },
    recommendedMode: 'cloud_byok',
    recommendationReason: 'Cấu hình iGPU Intel nên dùng Cloud BYOK',
    timestamp: '2026-11-23T12:00:00Z',
  };

  beforeEach(() => {
    vi.clearAllMocks();
    useHardwareStore.getState().reset();
  });

  it('renders hardware specs and recommended mode badge when profile is present', () => {
    useHardwareStore.setState({
      profile: mockNvidiaProfile,
      selectedMode: 'offline_local',
      isLoading: false,
    });

    render(<HardwareRecommendation />);

    expect(screen.getByTestId('hardware-recommendation-card')).toBeInTheDocument();
    expect(screen.getByText('AMD Ryzen 7 5800X')).toBeInTheDocument();
    expect(screen.getByText('16.0 GB Tổng')).toBeInTheDocument();
    expect(screen.getByText('GeForce RTX 3070')).toBeInTheDocument();
    expect(screen.getByText('8.0 GB VRAM')).toBeInTheDocument();
    expect(screen.getByTestId('cuda-support-badge')).toHaveTextContent('CUDA Sẵn sàng');
    expect(screen.getByTestId('recommended-mode-badge')).toHaveTextContent('offline_local');
    expect(screen.getByTestId('recommendation-reason-text')).toHaveTextContent(
      'GPU NVIDIA 8GB VRAM hỗ trợ CUDA và RAM 16GB',
    );
  });

  it('allows user to switch modes and shows manual override indicator', () => {
    useHardwareStore.setState({
      profile: mockNvidiaProfile,
      selectedMode: 'offline_local',
      isOverridden: false,
    });

    render(<HardwareRecommendation />);

    expect(screen.queryByTestId('mode-overridden-badge')).not.toBeInTheDocument();

    const hybridOption = screen.getByTestId('mode-option-hybrid');
    fireEvent.click(hybridOption);

    expect(useHardwareStore.getState().selectedMode).toBe('hybrid');
    expect(useHardwareStore.getState().isOverridden).toBe(true);
    expect(screen.getByTestId('mode-overridden-badge')).toBeInTheDocument();
  });

  it('displays low-end warning when weak PC selects offline_local', () => {
    useHardwareStore.setState({
      profile: mockIrisProfile,
      selectedMode: 'cloud_byok',
      isOverridden: false,
    });

    render(<HardwareRecommendation />);

    expect(screen.queryByTestId('low-end-local-warning')).not.toBeInTheDocument();

    const offlineLocalOption = screen.getByTestId('mode-option-offline_local');
    fireEvent.click(offlineLocalOption);

    expect(screen.getByTestId('low-end-local-warning')).toBeInTheDocument();
    expect(screen.getByText(/Lưu ý hiệu năng:/i)).toBeInTheDocument();
  });

  it('calls detectHardware when clicking "Quét lại cấu hình"', async () => {
    vi.mocked(hardwareApi.detectHardwareProfile).mockResolvedValueOnce(mockNvidiaProfile);
    useHardwareStore.setState({
      profile: mockNvidiaProfile,
      selectedMode: 'offline_local',
    });

    render(<HardwareRecommendation />);

    const reprobeBtn = screen.getByTestId('btn-reprobe-hardware');
    fireEvent.click(reprobeBtn);

    await waitFor(() => {
      expect(hardwareApi.detectHardwareProfile).toHaveBeenCalledTimes(1);
    });
  });

  it('triggers onProceed callback with selected mode when clicking confirm button', () => {
    const handleProceed = vi.fn();
    useHardwareStore.setState({
      profile: mockNvidiaProfile,
      selectedMode: 'hybrid',
    });

    render(<HardwareRecommendation onProceed={handleProceed} />);

    const confirmBtn = screen.getByTestId('btn-confirm-hardware-mode');
    fireEvent.click(confirmBtn);

    expect(handleProceed).toHaveBeenCalledWith('hybrid');
  });

  it('renders error message when error exists in store', () => {
    useHardwareStore.setState({
      error: 'Không thể kết nối đến cảm biến phần cứng',
    });

    render(<HardwareRecommendation />);

    expect(screen.getByTestId('hardware-probe-error')).toBeInTheDocument();
    expect(screen.getByText('Không thể kết nối đến cảm biến phần cứng')).toBeInTheDocument();
  });

  it('shows loading indicator skeleton when profile is null', () => {
    useHardwareStore.setState({
      profile: null,
      isLoading: true,
    });

    render(<HardwareRecommendation />);

    expect(screen.getByTestId('hardware-loading-skeleton')).toBeInTheDocument();
    expect(screen.getByText(/Đang truy vấn thông số phần cứng hệ thống/i)).toBeInTheDocument();
  });

  it('automatically fetches hardware profile on mount if profile is null', async () => {
    vi.mocked(hardwareApi.fetchHardwareProfile).mockResolvedValueOnce(mockNvidiaProfile);

    render(<HardwareRecommendation />);

    await waitFor(() => {
      expect(hardwareApi.fetchHardwareProfile).toHaveBeenCalledTimes(1);
    });
  });

  describe('Adversarial Remediations (Finding 2D & 1B)', () => {
    it('Finding 2D: executes mount probing flow exactly once on clean boot without duplicate calls', async () => {
      // Simulate clean boot: no hardware_profile.json on disk (fetch returns null)
      vi.mocked(hardwareApi.fetchHardwareProfile).mockResolvedValueOnce(null);
      vi.mocked(hardwareApi.detectHardwareProfile).mockResolvedValueOnce(mockIrisProfile);

      render(<HardwareRecommendation />);

      // Wait until hardware detection completes and specs grid renders
      await waitFor(() => {
        expect(screen.getByTestId('hardware-specs-grid')).toBeInTheDocument();
      });

      // Verify each IPC call was triggered exactly ONCE (no double probe invocation)
      expect(hardwareApi.fetchHardwareProfile).toHaveBeenCalledTimes(1);
      expect(hardwareApi.detectHardwareProfile).toHaveBeenCalledTimes(1);
      expect(screen.getByText('Intel Core i5-1135G7')).toBeInTheDocument();
      expect(screen.getByTestId('recommended-mode-badge')).toHaveTextContent('cloud_byok');
    });

    it('Finding 1B: preserves user manual mode selection made before profile probe completes', async () => {
      // Simulate slower probe
      vi.mocked(hardwareApi.fetchHardwareProfile).mockImplementation(
        () => new Promise((resolve) => setTimeout(() => resolve(mockNvidiaProfile), 30)),
      );

      render(<HardwareRecommendation />);

      // Verify loading skeleton is visible
      expect(screen.getByTestId('hardware-loading-skeleton')).toBeInTheDocument();

      // User immediately clicks hybrid option while loading
      const hybridOption = screen.getByTestId('mode-option-hybrid');
      fireEvent.click(hybridOption);

      // Verify store updated immediately
      expect(useHardwareStore.getState().selectedMode).toBe('hybrid');
      expect(useHardwareStore.getState().isOverridden).toBe(true);

      // Wait for probe to complete
      await waitFor(() => {
        expect(screen.getByTestId('hardware-specs-grid')).toBeInTheDocument();
      });

      // Verify user's explicit selection was NOT reset to recommendedMode (offline_local)
      expect(useHardwareStore.getState().selectedMode).toBe('hybrid');
      expect(useHardwareStore.getState().isOverridden).toBe(true);
      expect(screen.getByTestId('mode-overridden-badge')).toBeInTheDocument();
    });
  });
});
