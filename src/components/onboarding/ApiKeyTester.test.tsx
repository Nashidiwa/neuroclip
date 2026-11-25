import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ApiKeyTester } from './ApiKeyTester.tsx';

describe('ApiKeyTester Component (AC-28.1, IT-28.2)', () => {
  it('renders input, mask toggle button, and external key link', () => {
    const handleChange = vi.fn();
    render(
      <ApiKeyTester
        provider="groq"
        label="Groq Cloud API Key"
        value="gsk_test123"
        onChange={handleChange}
        getKeyUrl="https://console.groq.com/keys"
        helpText="Nhập khóa Groq API"
      />,
    );

    expect(screen.getByText('Groq Cloud API Key')).toBeInTheDocument();
    expect(screen.getByText('Nhập khóa Groq API')).toBeInTheDocument();

    const link = screen.getByRole('link', { name: /Lấy Free API Key/i });
    expect(link).toHaveAttribute('href', 'https://console.groq.com/keys');
    expect(link).toHaveAttribute('target', '_blank');

    const input = screen.getByDisplayValue('gsk_test123');
    expect(input).toHaveAttribute('type', 'password');

    // Toggle mask
    const eyeBtn = screen.getByTitle('Hiện khóa');
    fireEvent.click(eyeBtn);
    expect(input).toHaveAttribute('type', 'text');

    fireEvent.click(screen.getByTitle('Ẩn khóa'));
    expect(input).toHaveAttribute('type', 'password');
  });

  it('disables test connection button when input value is empty or whitespace', () => {
    render(
      <ApiKeyTester
        provider="groq"
        label="Groq Key"
        value="   "
        onChange={vi.fn()}
        getKeyUrl="https://console.groq.com/keys"
      />,
    );

    const testBtn = screen.getByRole('button', { name: /Kiểm tra kết nối/i });
    expect(testBtn).toBeDisabled();
  });

  it('performs connection test and displays [Sẵn sàng] with latency within <= 2.0s SLA (AC-28.1, IT-28.2)', async () => {
    const mockTest = vi.fn().mockImplementation(async () => {
      // Simulate fast network call under 200ms
      return { success: true, latencyMs: 145 };
    });

    render(
      <ApiKeyTester
        provider="groq"
        label="Groq Key"
        value="gsk_valid_key_123"
        onChange={vi.fn()}
        getKeyUrl="https://console.groq.com/keys"
        onTest={mockTest}
      />,
    );

    const testBtn = screen.getByRole('button', { name: /Kiểm tra kết nối/i });
    expect(testBtn).not.toBeDisabled();

    fireEvent.click(testBtn);

    await waitFor(() => {
      expect(screen.getByText(/\[Sẵn sàng\] \(145 ms\)/i)).toBeInTheDocument();
    });

    expect(mockTest).toHaveBeenCalledWith('groq', 'gsk_valid_key_123');
  });

  it('displays [Không hợp lệ] and error message when connection test fails', async () => {
    const mockTest = vi.fn().mockResolvedValue({
      success: false,
      error: 'Khóa API không hợp lệ (HTTP 401 Unauthorized)',
    });

    render(
      <ApiKeyTester
        provider="gemini"
        label="Gemini Key"
        value="AIza_invalid_key"
        onChange={vi.fn()}
        getKeyUrl="https://aistudio.google.com/app/apikey"
        onTest={mockTest}
      />,
    );

    const testBtn = screen.getByRole('button', { name: /Kiểm tra kết nối/i });
    fireEvent.click(testBtn);

    await waitFor(() => {
      expect(screen.getByText(/\[Không hợp lệ\]/i)).toBeInTheDocument();
      expect(screen.getByText(/HTTP 401 Unauthorized/i)).toBeInTheDocument();
    });
  });

  it('resets status to idle when input value changes', async () => {
    const mockTest = vi.fn().mockResolvedValue({ success: true, latencyMs: 80 });
    const handleChange = vi.fn();

    const { rerender } = render(
      <ApiKeyTester
        provider="groq"
        label="Groq Key"
        value="gsk_key"
        onChange={handleChange}
        getKeyUrl="https://console.groq.com/keys"
        onTest={mockTest}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: /Kiểm tra kết nối/i }));

    await waitFor(() => {
      expect(screen.getByText(/\[Sẵn sàng\]/i)).toBeInTheDocument();
    });

    // Simulate user editing the input
    const input = screen.getByDisplayValue('gsk_key');
    fireEvent.change(input, { target: { value: 'gsk_key_edited' } });
    expect(handleChange).toHaveBeenCalledWith('gsk_key_edited');

    rerender(
      <ApiKeyTester
        provider="groq"
        label="Groq Key"
        value="gsk_key_edited"
        onChange={handleChange}
        getKeyUrl="https://console.groq.com/keys"
        onTest={mockTest}
      />,
    );

    expect(screen.getByText('Chưa kiểm tra kết nối')).toBeInTheDocument();
    expect(screen.queryByText(/\[Sẵn sàng\]/i)).not.toBeInTheDocument();
  });

  it('formats plain IpcErrorResponse objects cleanly without rendering [object Object]', async () => {
    const mockTest = vi.fn().mockRejectedValue({
      code: 'ERR_TRANSLATE_API_FAILED',
      message: 'Gọi API dịch thuật Gemini thất bại',
      details: 'HTTP 404 Not Found',
      recoverable: true,
    });

    render(
      <ApiKeyTester
        provider="gemini"
        label="Gemini Key"
        value="AQ.AbTestKey1234567890123456"
        onChange={vi.fn()}
        getKeyUrl="https://aistudio.google.com/app/apikey"
        onTest={mockTest}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: /Kiểm tra kết nối/i }));

    await waitFor(() => {
      expect(screen.getByText(/\[Lỗi kết nối\]/i)).toBeInTheDocument();
      expect(
        screen.getByText(/Gọi API dịch thuật Gemini thất bại \(HTTP 404 Not Found\)/i),
      ).toBeInTheDocument();
    });
    expect(screen.queryByText(/\[object Object\]/i)).not.toBeInTheDocument();
  });

  it('maps ERR_STT_INVALID_API_KEY and ERR_TRANSLATE_INVALID_API_KEY rejections to [Không hợp lệ]', async () => {
    const mockGroqTest = vi.fn().mockRejectedValue({
      code: 'ERR_STT_INVALID_API_KEY',
      message: 'Groq API Key không hợp lệ',
      details: null,
      recoverable: false,
    });

    const { rerender } = render(
      <ApiKeyTester
        provider="groq"
        label="Groq Key"
        value="gsk_bad_key"
        onChange={vi.fn()}
        getKeyUrl="https://console.groq.com/keys"
        onTest={mockGroqTest}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: /Kiểm tra kết nối/i }));

    await waitFor(() => {
      expect(screen.getByText(/\[Không hợp lệ\]/i)).toBeInTheDocument();
      expect(screen.getByText(/Groq API Key không hợp lệ/i)).toBeInTheDocument();
    });

    const mockGeminiTest = vi.fn().mockRejectedValue({
      code: 'ERR_TRANSLATE_INVALID_API_KEY',
      message: 'Google Gemini API Key không hợp lệ',
      details: null,
      recoverable: false,
    });

    rerender(
      <ApiKeyTester
        provider="gemini"
        label="Gemini Key"
        value="AQ.AbBadKey"
        onChange={vi.fn()}
        getKeyUrl="https://aistudio.google.com/app/apikey"
        onTest={mockGeminiTest}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: /Kiểm tra kết nối/i }));

    await waitFor(() => {
      expect(screen.getByText(/\[Không hợp lệ\]/i)).toBeInTheDocument();
      expect(screen.getByText(/Google Gemini API Key không hợp lệ/i)).toBeInTheDocument();
    });
  });
});
