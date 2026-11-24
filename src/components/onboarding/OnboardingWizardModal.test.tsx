import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { OnboardingWizardModal } from './OnboardingWizardModal.tsx';
import { useSettingsStore } from '../../stores/settingsStore.ts';
import { useHardwareStore } from '../../stores/useHardwareStore.ts';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}));

describe('OnboardingWizardModal Component (UT-28.1, AC-28.3)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useSettingsStore.setState({
      config: { ...useSettingsStore.getState().config, isFirstRun: true },
    });
    useHardwareStore.setState({
      profile: null,
      selectedMode: 'hybrid',
      isLoading: false,
      error: null,
    });
  });

  it('renders nothing when isOpen is false (UT-28.1)', () => {
    const { container } = render(<OnboardingWizardModal isOpen={false} onClose={vi.fn()} />);
    expect(container).toBeEmptyDOMElement();
  });

  it('renders Step 1 (Welcome) by default when isOpen is true', () => {
    render(<OnboardingWizardModal isOpen={true} onClose={vi.fn()} />);

    expect(
      screen.getByRole('heading', { name: /Chào Mừng Đến Với NeuroClip Studio/i }),
    ).toBeInTheDocument();
    expect(screen.getByText(/Audio-First Ingestion/i)).toBeInTheDocument();
    expect(screen.getByText(/Multimodal Highlights/i)).toBeInTheDocument();
    expect(screen.getByText(/Theatrical Subtitles/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Bắt đầu thiết lập/i })).toBeInTheDocument();
  });

  it('navigates through all 4 steps and completes onboarding', async () => {
    const handleClose = vi.fn();
    const handleComplete = vi.fn();

    render(
      <OnboardingWizardModal isOpen={true} onClose={handleClose} onComplete={handleComplete} />,
    );

    // Step 1 -> Step 2
    fireEvent.click(screen.getByRole('button', { name: /Bắt đầu thiết lập/i }));
    expect(screen.getByText(/Đánh giá năng lực phần cứng & đề xuất chế độ/i)).toBeInTheDocument();

    // Step 2 -> Step 3
    fireEvent.click(screen.getByRole('button', { name: /Tiếp tục \(Khóa API\)/i }));
    expect(screen.getByText(/Cấu hình khóa BYOK API Keys/i)).toBeInTheDocument();
    expect(screen.getByText(/Groq Cloud API Key/i)).toBeInTheDocument();
    expect(screen.getByText(/Google Gemini API Key/i)).toBeInTheDocument();

    // Fill API key in Step 3
    const inputs = screen.getAllByPlaceholderText(/gsk_\.\.\.|AIza\.\.\./);
    expect(inputs.length).toBe(2);

    // Step 3 -> Step 4
    fireEvent.click(screen.getByRole('button', { name: /^Tiếp tục$/i }));
    expect(screen.getByText(/Sẵn Sàng Sáng Tạo Cùng NeuroClip!/i)).toBeInTheDocument();
    expect(screen.getByText(/Tóm tắt cấu hình khởi chạy/i)).toBeInTheDocument();

    // Step 4: Finish
    const finishBtn = screen.getByRole('button', { name: /Bắt đầu sáng tạo/i });
    fireEvent.click(finishBtn);

    await waitFor(() => {
      expect(handleComplete).toHaveBeenCalledTimes(1);
      expect(handleClose).toHaveBeenCalledTimes(1);
    });
  });

  it('supports going back to previous steps using Quay lại button', () => {
    render(<OnboardingWizardModal isOpen={true} onClose={vi.fn()} />);

    // Go to step 2
    fireEvent.click(screen.getByRole('button', { name: /Bắt đầu thiết lập/i }));
    expect(screen.getByText(/Đánh giá năng lực phần cứng & đề xuất chế độ/i)).toBeInTheDocument();

    // Go back to step 1
    fireEvent.click(screen.getByRole('button', { name: /Quay lại/i }));
    expect(screen.getByText(/Audio-First Ingestion/i)).toBeInTheDocument();
  });

  it('calls onClose when close icon (X) is clicked', () => {
    const handleClose = vi.fn();
    render(<OnboardingWizardModal isOpen={true} onClose={handleClose} />);

    fireEvent.click(screen.getByTitle(/Đóng trình hướng dẫn/i));
    expect(handleClose).toHaveBeenCalledTimes(1);
  });
});
