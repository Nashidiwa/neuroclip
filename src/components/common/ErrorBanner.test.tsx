import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ErrorBanner } from './ErrorBanner';
import { ERROR_CODES, IpcErrorResponse } from '@/services/ipc/error';

describe('TASK-M4-02: ErrorBanner Component (AC-02.2 & IT-02.2)', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('should return null when error is null or undefined', () => {
    const { container: container1 } = render(<ErrorBanner error={null} />);
    expect(container1.firstChild).toBeNull();

    const { container: container2 } = render(<ErrorBanner error={undefined} />);
    expect(container2.firstChild).toBeNull();
  });

  it('should render error code badge and Vietnamese message', () => {
    const errorPayload: IpcErrorResponse = {
      code: ERROR_CODES.INGEST_DISK_FULL,
      message: 'Dung lượng ổ đĩa không đủ (cần tối thiểu 5000MB, hiện có 1200MB)',
      details: null,
      recoverable: true,
    };

    render(<ErrorBanner error={errorPayload} />);

    expect(screen.getByTestId('error-code-badge')).toHaveTextContent(ERROR_CODES.INGEST_DISK_FULL);
    expect(screen.getByTestId('error-message')).toHaveTextContent(
      'Dung lượng ổ đĩa không đủ (cần tối thiểu 5000MB, hiện có 1200MB)',
    );
  });

  it('should render custom title if provided', () => {
    const errorPayload: IpcErrorResponse = {
      code: ERROR_CODES.INGEST_INVALID_URL,
      message: 'URL không hợp lệ',
      details: null,
      recoverable: false,
    };

    render(<ErrorBanner error={errorPayload} title="Lỗi tải video" />);

    expect(screen.getByText('Lỗi tải video')).toBeInTheDocument();
  });

  it('should display recoverable badge, icon, and accent border for recoverable error', () => {
    const errorPayload: IpcErrorResponse = {
      code: ERROR_CODES.INGEST_NETWORK_TIMEOUT,
      message: 'Mất kết nối mạng',
      details: null,
      recoverable: true,
    };

    render(<ErrorBanner error={errorPayload} />);

    expect(screen.getByTestId('icon-recoverable')).toBeInTheDocument();
    expect(screen.getByText('Có thể thử lại')).toBeInTheDocument();
    expect(screen.getByTestId('error-banner').className).toContain('border-l-accent-neuro');
  });

  it('should display unrecoverable badge, icon, and evil accent border for non-recoverable error', () => {
    const errorPayload: IpcErrorResponse = {
      code: ERROR_CODES.INTERNAL_ERROR,
      message: 'Lỗi hệ thống nội bộ',
      details: null,
      recoverable: false,
    };

    render(<ErrorBanner error={errorPayload} />);

    expect(screen.getByTestId('icon-unrecoverable')).toBeInTheDocument();
    expect(screen.getByText('Lỗi nghiêm trọng')).toBeInTheDocument();
    expect(screen.getByTestId('error-banner').className).toContain('border-l-accent-evil');
  });

  it('should render retry button when onRetry is passed and error is recoverable', () => {
    const onRetry = vi.fn();
    const errorPayload: IpcErrorResponse = {
      code: ERROR_CODES.STT_RATE_LIMIT,
      message: 'Groq API Rate Limit',
      details: null,
      recoverable: true,
    };

    render(<ErrorBanner error={errorPayload} onRetry={onRetry} />);

    const retryBtn = screen.getByTestId('error-retry-btn');
    expect(retryBtn).toBeInTheDocument();
    expect(retryBtn).toHaveTextContent('Thử lại');

    fireEvent.click(retryBtn);
    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  it('should NOT render retry button when recoverable is false', () => {
    const onRetry = vi.fn();
    const errorPayload: IpcErrorResponse = {
      code: ERROR_CODES.STT_PAYLOAD_TOO_LARGE,
      message: 'Payload quá lớn',
      details: null,
      recoverable: false,
    };

    render(<ErrorBanner error={errorPayload} onRetry={onRetry} />);

    expect(screen.queryByTestId('error-retry-btn')).toBeNull();
    expect(screen.queryByRole('button', { name: /Thử lại/i })).toBeNull();
  });

  it('should render dismiss button when onDismiss is passed and trigger callback', () => {
    const onDismiss = vi.fn();
    const errorPayload: IpcErrorResponse = {
      code: ERROR_CODES.INGEST_CHAT_UNAVAILABLE,
      message: 'Chat replay không tồn tại',
      details: null,
      recoverable: true,
    };

    render(<ErrorBanner error={errorPayload} onDismiss={onDismiss} />);

    const dismissBtn = screen.getByTestId('error-dismiss-btn');
    expect(dismissBtn).toBeInTheDocument();

    fireEvent.click(dismissBtn);
    expect(onDismiss).toHaveBeenCalledTimes(1);
  });

  it('should toggle technical details panel and support clipboard copy', async () => {
    const writeTextMock = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, {
      clipboard: {
        writeText: writeTextMock,
      },
    });

    const errorPayload: IpcErrorResponse = {
      code: ERROR_CODES.TRANSLATE_SAFETY_FILTER,
      message: 'Bộ lọc an toàn kích hoạt',
      details: 'finishReason: SAFETY in candidate[0]',
      recoverable: true,
    };

    render(<ErrorBanner error={errorPayload} />);

    // Toggle button should be present
    const toggleBtn = screen.getByTestId('toggle-details-btn');
    expect(toggleBtn).toHaveTextContent('Xem chi tiết lỗi kỹ thuật');
    expect(screen.queryByTestId('error-details-panel')).toBeNull();

    // Expand details
    fireEvent.click(toggleBtn);
    expect(screen.getByTestId('error-details-panel')).toBeInTheDocument();
    expect(screen.getByText('finishReason: SAFETY in candidate[0]')).toBeInTheDocument();
    expect(toggleBtn).toHaveTextContent('Ẩn chi tiết kỹ thuật');

    // Click copy button
    const copyBtn = screen.getByTestId('copy-details-btn');
    fireEvent.click(copyBtn);

    await waitFor(() => {
      expect(writeTextMock).toHaveBeenCalledTimes(1);
    });
    expect(screen.getByText('Đã sao chép!')).toBeInTheDocument();

    // Collapse details
    fireEvent.click(toggleBtn);
    expect(screen.queryByTestId('error-details-panel')).toBeNull();
  });

  it('should automatically parse string error prop via parseIpcError', () => {
    render(<ErrorBanner error="Lỗi chuỗi phát sinh" />);

    expect(screen.getByTestId('error-code-badge')).toHaveTextContent(ERROR_CODES.INTERNAL_ERROR);
    expect(screen.getByTestId('error-message')).toHaveTextContent('Lỗi chuỗi phát sinh');
  });

  it('should accept explicit undefined optional props under exactOptionalPropertyTypes', () => {
    const errorPayload: IpcErrorResponse = {
      code: ERROR_CODES.INTERNAL_ERROR,
      message: 'Lỗi thử nghiệm',
      details: null,
      recoverable: true,
    };

    render(
      <ErrorBanner
        error={errorPayload}
        title={undefined}
        onRetry={undefined}
        onDismiss={undefined}
        className={undefined}
        compact={undefined}
      />,
    );

    expect(screen.getByTestId('error-code-badge')).toHaveTextContent(ERROR_CODES.INTERNAL_ERROR);
    expect(screen.queryByTestId('error-retry-btn')).toBeNull();
    expect(screen.queryByTestId('error-dismiss-btn')).toBeNull();
  });

  it('should safely parse JavaScript Error instances via parseIpcError', () => {
    const nativeError = new Error('Database connection failed unexpectedly');

    render(<ErrorBanner error={nativeError} />);

    expect(screen.getByTestId('error-code-badge')).toHaveTextContent(ERROR_CODES.INTERNAL_ERROR);
    expect(screen.getByTestId('error-message')).toHaveTextContent(
      'Database connection failed unexpectedly',
    );
    expect(screen.getByTestId('icon-unrecoverable')).toBeInTheDocument();
  });

  it('should safely parse arbitrary non-IpcError plain objects via parseIpcError', () => {
    const rawObject = {
      code: 'ERR_CUSTOM_FAIL',
      message: 'Tải tài nguyên thất bại từ server ngoại vi',
      recoverable: true,
    };

    render(<ErrorBanner error={rawObject} />);

    expect(screen.getByTestId('error-code-badge')).toHaveTextContent('ERR_CUSTOM_FAIL');
    expect(screen.getByTestId('error-message')).toHaveTextContent(
      'Tải tài nguyên thất bại từ server ngoại vi',
    );
    expect(screen.getByTestId('icon-recoverable')).toBeInTheDocument();
  });
});
