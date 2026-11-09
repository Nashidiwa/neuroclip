import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import React, { useState } from 'react';
import { ErrorBoundary } from './ErrorBoundary';
import { ErrorBanner } from './ErrorBanner';
import { ERROR_CODES, IpcErrorResponse } from '@/services/ipc/error';

// Component throwing arbitrary values
const AdversarialThrower: React.FC<{
  throwValue?: unknown;
  shouldThrow?: boolean;
}> = ({ throwValue, shouldThrow = true }) => {
  if (shouldThrow) {
    throw throwValue;
  }
  return <div data-testid="healthy-child">Healthy Content</div>;
};

describe('TASK-M4-02: Adversarial Stress Testing — ErrorBoundary & ErrorBanner', () => {
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    vi.restoreAllMocks();
    consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  afterEach(() => {
    consoleErrorSpy.mockRestore();
  });

  // =========================================================================
  // 1. NON-ERROR THROWABLES RESILIENCE
  // =========================================================================
  describe('1. Non-Error Throwables Resilience', () => {
    it('should catch string literal throw without crash and render error message', () => {
      render(
        <ErrorBoundary>
          <AdversarialThrower throwValue="Fatal: SQLite database is locked by concurrent process" />
        </ErrorBoundary>,
      );

      expect(screen.getByTestId('error-boundary-fallback')).toBeInTheDocument();
      expect(screen.getByTestId('error-banner')).toBeInTheDocument();
      expect(screen.getByTestId('error-code-badge')).toHaveTextContent(ERROR_CODES.INTERNAL_ERROR);
      expect(screen.getByTestId('error-message')).toHaveTextContent(
        'Fatal: SQLite database is locked by concurrent process',
      );
      expect(screen.getByTestId('error-retry-btn')).toBeInTheDocument();
    });

    it('should catch arbitrary object throw without crash and use fallback message', () => {
      render(
        <ErrorBoundary>
          <AdversarialThrower throwValue={{ arbitrary: 123, status: 'FATAL_CORRUPT' }} />
        </ErrorBoundary>,
      );

      expect(screen.getByTestId('error-boundary-fallback')).toBeInTheDocument();
      expect(screen.getByTestId('error-message')).toHaveTextContent(
        'Đã xảy ra sự cố kết xuất giao diện',
      );
      expect(screen.getByTestId('error-code-badge')).toHaveTextContent(ERROR_CODES.INTERNAL_ERROR);
    });

    it('should catch throw null cleanly without crash', () => {
      render(
        <ErrorBoundary>
          <AdversarialThrower throwValue={null} />
        </ErrorBoundary>,
      );

      expect(screen.getByTestId('error-boundary-fallback')).toBeInTheDocument();
      expect(screen.getByTestId('error-message')).toHaveTextContent(
        'Đã xảy ra sự cố kết xuất giao diện',
      );
    });

    it('should catch throw undefined cleanly without crash', () => {
      render(
        <ErrorBoundary>
          <AdversarialThrower throwValue={undefined} />
        </ErrorBoundary>,
      );

      expect(screen.getByTestId('error-boundary-fallback')).toBeInTheDocument();
      expect(screen.getByTestId('error-message')).toHaveTextContent(
        'Đã xảy ra sự cố kết xuất giao diện',
      );
    });

    it('should catch throw number primitive cleanly without crash', () => {
      render(
        <ErrorBoundary>
          <AdversarialThrower throwValue={504} />
        </ErrorBoundary>,
      );

      expect(screen.getByTestId('error-boundary-fallback')).toBeInTheDocument();
      expect(screen.getByTestId('error-code-badge')).toHaveTextContent(ERROR_CODES.INTERNAL_ERROR);
    });

    it('should pass non-Error thrown value and errorInfo to onError callback', () => {
      const onErrorMock = vi.fn();
      const payload = { customCode: 'FAIL_99', message: 'Custom failure message' };

      render(
        <ErrorBoundary onError={onErrorMock}>
          <AdversarialThrower throwValue={payload} />
        </ErrorBoundary>,
      );

      expect(onErrorMock).toHaveBeenCalledTimes(1);
      const [caughtErr, caughtInfo] = onErrorMock.mock.calls[0] ?? [];
      expect(caughtErr).toEqual(payload);
      expect(caughtInfo).toHaveProperty('componentStack');
    });

    it('should extract structured code and message if thrown object has them', () => {
      render(
        <ErrorBoundary>
          <AdversarialThrower
            throwValue={{
              code: ERROR_CODES.INGEST_BOT_BLOCKED,
              message: 'YouTube bot verification challenge required',
            }}
          />
        </ErrorBoundary>,
      );

      expect(screen.getByTestId('error-code-badge')).toHaveTextContent(ERROR_CODES.INGEST_BOT_BLOCKED);
      expect(screen.getByTestId('error-message')).toHaveTextContent(
        'YouTube bot verification challenge required',
      );
    });
  });

  // =========================================================================
  // 2. ONRETRY / ONRESET THROWING ERROR RESILIENCE
  // =========================================================================
  describe('2. onRetry & onReset Failure Modes', () => {
    it('should retain fallback card visible when onReset prop throws an error', () => {
      const explodingReset = vi.fn().mockImplementation(() => {
        throw new Error('Exploding during onReset cleanup!');
      });

      render(
        <ErrorBoundary onReset={explodingReset}>
          <AdversarialThrower throwValue={new Error('Initial Render Failure')} />
        </ErrorBoundary>,
      );

      expect(screen.getByTestId('error-boundary-fallback')).toBeInTheDocument();

      const capturedErrors: Error[] = [];
      const errorHandler = (event: ErrorEvent): void => {
        event.preventDefault();
        if (event.error instanceof Error) {
          capturedErrors.push(event.error);
        }
      };
      window.addEventListener('error', errorHandler);

      const retryBtn = screen.getByTestId('error-retry-btn');
      fireEvent.click(retryBtn);

      window.removeEventListener('error', errorHandler);

      expect(explodingReset).toHaveBeenCalledTimes(1);
      // The exception was caught by window error dispatcher per React event contract
      expect(capturedErrors.length).toBeGreaterThan(0);
      expect(capturedErrors[0]?.message).toBe('Exploding during onReset cleanup!');
      // The boundary remains rendered in fallback state because setState was aborted
      expect(screen.getByTestId('error-boundary-fallback')).toBeInTheDocument();
    });

    it('should handle subsequent failures gracefully when retry is clicked but child throws again', () => {
      const Harness: React.FC = () => {
        const [attempt, setAttempt] = useState<number>(1);

        return (
          <ErrorBoundary>
            <AdversarialThrower
              shouldThrow={true}
              throwValue={new Error(`Lỗi thất bại lần ${attempt}`)}
            />
            <button
              type="button"
              data-testid="increment-attempt"
              onClick={() => setAttempt((prev) => prev + 1)}
            >
              Tăng
            </button>
          </ErrorBoundary>
        );
      };

      render(<Harness />);

      expect(screen.getByTestId('error-message')).toHaveTextContent('Lỗi thất bại lần 1');

      // Click retry button in default fallback
      const retryBtn = screen.getByTestId('error-retry-btn');
      fireEvent.click(retryBtn);

      // Child throws again, ErrorBoundary catches it again with updated message
      expect(screen.getByTestId('error-boundary-fallback')).toBeInTheDocument();
      expect(screen.getByTestId('error-message')).toHaveTextContent('Lỗi thất bại lần 1');
    });

    it('should bubble error when ErrorBanner onRetry throws when clicked without breaking banner DOM', () => {
      const capturedErrors: Error[] = [];
      const errorHandler = (event: ErrorEvent): void => {
        event.preventDefault();
        if (event.error instanceof Error) {
          capturedErrors.push(event.error);
        }
      };
      window.addEventListener('error', errorHandler);

      const throwingOnRetry = vi.fn().mockImplementation(() => {
        throw new Error('Network connection dropped during retry');
      });

      const errorPayload: IpcErrorResponse = {
        code: ERROR_CODES.INGEST_NETWORK_TIMEOUT,
        message: 'Mất kết nối mạng',
        details: null,
        recoverable: true,
      };

      render(<ErrorBanner error={errorPayload} onRetry={throwingOnRetry} />);

      const retryBtn = screen.getByTestId('error-retry-btn');
      fireEvent.click(retryBtn);

      window.removeEventListener('error', errorHandler);

      expect(throwingOnRetry).toHaveBeenCalledTimes(1);
      expect(capturedErrors.length).toBeGreaterThan(0);
      expect(capturedErrors[0]?.message).toBe('Network connection dropped during retry');
      // Banner remains rendered and intact
      expect(screen.getByTestId('error-banner')).toBeInTheDocument();
    });
  });

  // =========================================================================
  // 3. EXTREME DETAIL PAYLOADS (100KB+ STACK TRACE / LOG DUMP)
  // =========================================================================
  describe('3. Extreme Detail Payloads (100KB+ Log Dump / Stack Trace)', () => {
    // Generate 120KB realistic stack trace / memory dump string
    const largeStackChunk = '    at async invokeHandler (src-tauri/src/export/capcut.rs:142:9)\n';
    const extremeDetails = 'CRITICAL OVERFLOW LOG DUMP:\n' + largeStackChunk.repeat(1800); // ~120KB

    it('should render 120KB details payload without crashing or freeze', () => {
      const errorPayload: IpcErrorResponse = {
        code: ERROR_CODES.EXPORT_FILE_WRITE_FAILED,
        message: 'Ghi tệp CapCut dự án thất bại do lỗi bộ đệm',
        details: extremeDetails,
        recoverable: true,
      };

      const startTime = performance.now();
      render(<ErrorBanner error={errorPayload} />);
      const duration = performance.now() - startTime;

      // Render duration must be snappy (< 200ms)
      expect(duration).toBeLessThan(200);

      // Details toggle button should be present
      const toggleBtn = screen.getByTestId('toggle-details-btn');
      expect(toggleBtn).toBeInTheDocument();

      // Open details panel
      fireEvent.click(toggleBtn);
      const preElement = screen.getByText((content) =>
        content.startsWith('CRITICAL OVERFLOW LOG DUMP:'),
      );
      expect(preElement).toBeInTheDocument();
      expect(preElement.tagName).toBe('PRE');

      // Verify scrolling containment classes
      expect(preElement.className).toContain('max-h-48');
      expect(preElement.className).toContain('overflow-x-auto');
      expect(preElement.className).toContain('whitespace-pre-wrap');
      expect(preElement.className).toContain('break-all');
    });

    it('should serialize and copy 120KB payload to clipboard cleanly', async () => {
      let copiedText = '';
      const writeTextMock = vi.fn().mockImplementation((text: string) => {
        copiedText = text;
        return Promise.resolve();
      });

      Object.assign(navigator, {
        clipboard: {
          writeText: writeTextMock,
        },
      });

      const errorPayload: IpcErrorResponse = {
        code: ERROR_CODES.HARDWARE_PROBE_FAILED,
        message: 'Phân tích phần cứng thất bại',
        details: extremeDetails,
        recoverable: false,
      };

      render(<ErrorBanner error={errorPayload} />);

      // Expand panel
      fireEvent.click(screen.getByTestId('toggle-details-btn'));

      // Click copy button
      const copyBtn = screen.getByTestId('copy-details-btn');
      fireEvent.click(copyBtn);

      await waitFor(() => {
        expect(writeTextMock).toHaveBeenCalledTimes(1);
      });

      expect(copiedText.length).toBeGreaterThan(100_000);
      expect(screen.getByText('Đã sao chép!')).toBeInTheDocument();

      // Verify copied JSON parses back correctly
      const parsed = JSON.parse(copiedText) as IpcErrorResponse;
      expect(parsed.code).toBe(ERROR_CODES.HARDWARE_PROBE_FAILED);
      expect(parsed.details).toBe(extremeDetails);
    });

    it('should handle clipboard writeText rejection gracefully without unhandled rejection', async () => {
      const writeTextMock = vi.fn().mockRejectedValue(new Error('Clipboard permission denied by OS policy'));
      Object.assign(navigator, {
        clipboard: {
          writeText: writeTextMock,
        },
      });

      const errorPayload: IpcErrorResponse = {
        code: ERROR_CODES.INTERNAL_ERROR,
        message: 'Lỗi',
        details: 'Sample technical details',
        recoverable: true,
      };

      render(<ErrorBanner error={errorPayload} />);

      fireEvent.click(screen.getByTestId('toggle-details-btn'));
      const copyBtn = screen.getByTestId('copy-details-btn');

      // Click copy - should not throw unhandled rejection
      fireEvent.click(copyBtn);

      await waitFor(() => {
        expect(writeTextMock).toHaveBeenCalledTimes(1);
      });

      // Still renders "Sao chép mã lỗi" because copy failed gracefully
      expect(screen.getByText('Sao chép mã lỗi')).toBeInTheDocument();
    });

    it('should handle missing navigator.clipboard gracefully without crashing', () => {
      // Temporarily mock navigator without clipboard
      const originalClipboard = navigator.clipboard;
      delete (navigator as { clipboard?: unknown }).clipboard;

      const errorPayload: IpcErrorResponse = {
        code: ERROR_CODES.INTERNAL_ERROR,
        message: 'Lỗi',
        details: 'Sample technical details',
        recoverable: true,
      };

      render(<ErrorBanner error={errorPayload} />);
      fireEvent.click(screen.getByTestId('toggle-details-btn'));

      const copyBtn = screen.getByTestId('copy-details-btn');
      // Should not throw
      expect(() => {
        fireEvent.click(copyBtn);
      }).not.toThrow();

      // Restore clipboard
      Object.assign(navigator, { clipboard: originalClipboard });
    });

    it('should unmount cleanly after copy without leaking timer or throwing', () => {
      vi.useFakeTimers();
      const writeTextMock = vi.fn().mockResolvedValue(undefined);
      Object.assign(navigator, { clipboard: { writeText: writeTextMock } });

      const errorPayload: IpcErrorResponse = {
        code: ERROR_CODES.STT_RATE_LIMIT,
        message: 'Rate limit',
        details: 'Details text',
        recoverable: true,
      };

      const { unmount } = render(<ErrorBanner error={errorPayload} />);
      fireEvent.click(screen.getByTestId('toggle-details-btn'));
      fireEvent.click(screen.getByTestId('copy-details-btn'));

      // Unmount component before 2000ms timer elapses
      unmount();

      // Advance timers to trigger setTimeout
      expect(() => {
        vi.advanceTimersByTime(2500);
      }).not.toThrow();

      vi.useRealTimers();
    });
  });

  // =========================================================================
  // 4. RESETKEYS RAPID ARRAY MUTATIONS
  // =========================================================================
  describe('4. resetKeys Rapid Array Prop Mutations', () => {
    it('should handle rapid array prop mutations and reset boundary correctly', () => {
      const onResetMock = vi.fn();

      const Harness: React.FC<{ keys: readonly unknown[]; shouldThrow: boolean }> = ({
        keys,
        shouldThrow,
      }) => (
        <ErrorBoundary resetKeys={keys} onReset={onResetMock}>
          <AdversarialThrower
            shouldThrow={shouldThrow}
            throwValue={new Error('Transient render error')}
          />
        </ErrorBoundary>
      );

      const { rerender } = render(<Harness keys={[1]} shouldThrow={true} />);
      expect(screen.getByTestId('error-boundary-fallback')).toBeInTheDocument();

      // Rapid mutation 1: Key increment with child still throwing
      rerender(<Harness keys={[2]} shouldThrow={true} />);
      expect(onResetMock).toHaveBeenCalledTimes(1);
      // Caught again
      expect(screen.getByTestId('error-boundary-fallback')).toBeInTheDocument();

      // Rapid mutation 2: Array length expansion
      rerender(<Harness keys={[2, 'sub-key']} shouldThrow={true} />);
      expect(onResetMock).toHaveBeenCalledTimes(2);

      // Rapid mutation 3: Array length contraction with child recovered
      rerender(<Harness keys={[99]} shouldThrow={false} />);
      expect(onResetMock).toHaveBeenCalledTimes(3);
      expect(screen.queryByTestId('error-boundary-fallback')).toBeNull();
      expect(screen.getByTestId('healthy-child')).toBeInTheDocument();
    });

    it('should NOT trigger spurious reset when resetKeys reference changes but element values are identical', () => {
      const onResetMock = vi.fn();

      const Harness: React.FC<{ keys: readonly unknown[] }> = ({ keys }) => (
        <ErrorBoundary resetKeys={keys} onReset={onResetMock}>
          <AdversarialThrower shouldThrow={true} throwValue={new Error('Sticky error')} />
        </ErrorBoundary>
      );

      const { rerender } = render(<Harness keys={[42, 'constant-value']} />);
      expect(screen.getByTestId('error-boundary-fallback')).toBeInTheDocument();
      expect(onResetMock).toHaveBeenCalledTimes(0);

      // Rerender with a NEW array reference having the same elements
      rerender(<Harness keys={[42, 'constant-value']} />);

      // Shallow element equality prevents unnecessary onReset invocation
      expect(onResetMock).toHaveBeenCalledTimes(0);
      expect(screen.getByTestId('error-boundary-fallback')).toBeInTheDocument();
    });

    it('should handle transition from undefined resetKeys to populated resetKeys', () => {
      const onResetMock = vi.fn();

      const Harness: React.FC<{ keys?: readonly unknown[] | undefined; shouldThrow: boolean }> = ({
        keys,
        shouldThrow,
      }) => (
        <ErrorBoundary resetKeys={keys} onReset={onResetMock}>
          <AdversarialThrower shouldThrow={shouldThrow} throwValue={new Error('Error')} />
        </ErrorBoundary>
      );

      const { rerender } = render(<Harness keys={undefined} shouldThrow={true} />);
      expect(screen.getByTestId('error-boundary-fallback')).toBeInTheDocument();

      // Update to provide resetKeys
      rerender(<Harness keys={['new-key']} shouldThrow={false} />);
      expect(onResetMock).toHaveBeenCalledTimes(1);
      expect(screen.getByTestId('healthy-child')).toBeInTheDocument();
    });

    it('should handle empty resetKeys array [] without infinite loops', () => {
      const onResetMock = vi.fn();

      const Harness: React.FC<{ keys: readonly unknown[] }> = ({ keys }) => (
        <ErrorBoundary resetKeys={keys} onReset={onResetMock}>
          <AdversarialThrower shouldThrow={true} throwValue={new Error('Persistent')} />
        </ErrorBoundary>
      );

      const { rerender } = render(<Harness keys={[]} />);
      expect(screen.getByTestId('error-boundary-fallback')).toBeInTheDocument();

      // Rerender with empty array
      rerender(<Harness keys={[]} />);
      expect(onResetMock).toHaveBeenCalledTimes(0);
    });
  });

  // =========================================================================
  // 5. BUTTON BEHAVIOR WHEN CALLBACKS ARE OMITTED
  // =========================================================================
  describe('5. Button Safety When Callbacks Are Omitted', () => {
    it('toggle-details-btn toggles cleanly without requiring external callback props', () => {
      const errorPayload: IpcErrorResponse = {
        code: ERROR_CODES.SIDECAR_BINARY_NOT_FOUND,
        message: 'ffmpeg binary not found',
        details: 'Checked paths: /usr/bin/ffmpeg, C:\\ffmpeg.exe',
        recoverable: false,
      };

      render(<ErrorBanner error={errorPayload} />);

      const toggleBtn = screen.getByTestId('toggle-details-btn');
      expect(screen.queryByTestId('error-details-panel')).toBeNull();

      fireEvent.click(toggleBtn);
      expect(screen.getByTestId('error-details-panel')).toBeInTheDocument();

      fireEvent.click(toggleBtn);
      expect(screen.queryByTestId('error-details-panel')).toBeNull();
    });

    it('error-retry-btn is completely omitted when onRetry is undefined', () => {
      const errorPayload: IpcErrorResponse = {
        code: ERROR_CODES.INGEST_NETWORK_TIMEOUT,
        message: 'Network timeout',
        details: null,
        recoverable: true, // Recoverable, but no onRetry prop provided!
      };

      render(<ErrorBanner error={errorPayload} onRetry={undefined} />);

      expect(screen.queryByTestId('error-retry-btn')).toBeNull();
    });

    it('error-retry-btn is completely omitted when recoverable is false, even if onRetry is provided', () => {
      const onRetryMock = vi.fn();
      const errorPayload: IpcErrorResponse = {
        code: ERROR_CODES.CONFIG_INVALID_FORMAT,
        message: 'Invalid JSON format in config',
        details: null,
        recoverable: false,
      };

      render(<ErrorBanner error={errorPayload} onRetry={onRetryMock} />);

      expect(screen.queryByTestId('error-retry-btn')).toBeNull();
    });

    it('error-dismiss-btn is completely omitted when onDismiss is undefined', () => {
      const errorPayload: IpcErrorResponse = {
        code: ERROR_CODES.INTERNAL_ERROR,
        message: 'Internal error',
        details: null,
        recoverable: false,
      };

      render(<ErrorBanner error={errorPayload} onDismiss={undefined} />);

      expect(screen.queryByTestId('error-dismiss-btn')).toBeNull();
    });

    it('copy-details-btn executes copy without requiring any external callback props', async () => {
      const writeTextMock = vi.fn().mockResolvedValue(undefined);
      Object.assign(navigator, { clipboard: { writeText: writeTextMock } });

      const errorPayload: IpcErrorResponse = {
        code: ERROR_CODES.TRANSLATE_API_FAILED,
        message: 'Gemini API Error 503 Service Unavailable',
        details: 'HTTP Status 503 from generativelanguage.googleapis.com',
        recoverable: true,
      };

      render(<ErrorBanner error={errorPayload} />);

      fireEvent.click(screen.getByTestId('toggle-details-btn'));
      const copyBtn = screen.getByTestId('copy-details-btn');

      fireEvent.click(copyBtn);

      await waitFor(() => {
        expect(writeTextMock).toHaveBeenCalledTimes(1);
      });
      expect(screen.getByText('Đã sao chép!')).toBeInTheDocument();
    });

    it('ErrorBoundary default fallback reset works cleanly when onReset is omitted', () => {
      const Harness: React.FC = () => {
        const [shouldThrow, setShouldThrow] = useState<boolean>(true);

        return (
          <div>
            <button
              type="button"
              data-testid="fix-state-btn"
              onClick={() => setShouldThrow(false)}
            >
              Fix State
            </button>
            <ErrorBoundary>
              <AdversarialThrower
                shouldThrow={shouldThrow}
                throwValue={new Error('Initial Error')}
              />
            </ErrorBoundary>
          </div>
        );
      };

      render(<Harness />);
      expect(screen.getByTestId('error-boundary-fallback')).toBeInTheDocument();

      // Fix state first
      fireEvent.click(screen.getByTestId('fix-state-btn'));

      // Click retry button in ErrorBoundary default fallback (which has onReset=undefined)
      const retryBtn = screen.getByTestId('error-retry-btn');
      expect(() => {
        fireEvent.click(retryBtn);
      }).not.toThrow();

      expect(screen.queryByTestId('error-boundary-fallback')).toBeNull();
      expect(screen.getByTestId('healthy-child')).toBeInTheDocument();
    });
  });

  // =========================================================================
  // 6. FURTHER ADVERSARIAL EDGE CASES
  // =========================================================================
  describe('6. Further Adversarial Edge Cases', () => {
    it('should handle malformed pseudo-JSON string errors without failing', () => {
      render(<ErrorBanner error="{ invalid pseudo json syntax here }" />);

      expect(screen.getByTestId('error-banner')).toBeInTheDocument();
      expect(screen.getByTestId('error-code-badge')).toHaveTextContent(ERROR_CODES.INTERNAL_ERROR);
      expect(screen.getByTestId('error-message')).toHaveTextContent(
        '{ invalid pseudo json syntax here }',
      );
    });

    it('should NOT render details toggle when details string contains only whitespace', () => {
      const errorPayload: IpcErrorResponse = {
        code: ERROR_CODES.INTERNAL_ERROR,
        message: 'Lỗi',
        details: '    \n\t   ',
        recoverable: false,
      };

      render(<ErrorBanner error={errorPayload} />);

      expect(screen.queryByTestId('toggle-details-btn')).toBeNull();
      expect(screen.queryByTestId('error-details-panel')).toBeNull();
    });

    it('should handle circular reference in error object during copy-details gracefully', () => {
      const circularError: Record<string, unknown> = {
        code: ERROR_CODES.INTERNAL_ERROR,
        message: 'Lỗi vòng lặp tham chiếu',
        details: 'Circular structure inside details',
        recoverable: true,
      };
      circularError['self'] = circularError;

      render(<ErrorBanner error={circularError as unknown as IpcErrorResponse} />);

      fireEvent.click(screen.getByTestId('toggle-details-btn'));
      const copyBtn = screen.getByTestId('copy-details-btn');

      // Click copy: JSON.stringify will throw TypeError inside try-catch, caught cleanly
      expect(() => {
        fireEvent.click(copyBtn);
      }).not.toThrow();
    });
  });
});

