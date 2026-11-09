import React, { useState } from 'react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ErrorBoundary } from './ErrorBoundary';

// Child component that optionally throws during render
const ThrowingChild: React.FC<{ shouldThrow?: boolean; message?: string }> = ({
  shouldThrow = false,
  message = 'Lỗi render mô phỏng',
}) => {
  if (shouldThrow) {
    throw new Error(message);
  }
  return <div data-testid="child-content">Nội dung con hoạt động bình thường</div>;
};

describe('TASK-M4-02: React ErrorBoundary Component (IT-02.2)', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('should render children when no exception occurs', () => {
    render(
      <ErrorBoundary>
        <ThrowingChild shouldThrow={false} />
      </ErrorBoundary>,
    );

    expect(screen.getByTestId('child-content')).toHaveTextContent(
      'Nội dung con hoạt động bình thường',
    );
  });

  it('should catch rendering exception and render default fallback with ErrorBanner', () => {
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    render(
      <ErrorBoundary>
        <ThrowingChild shouldThrow={true} message="Cố tình gây lỗi trong component" />
      </ErrorBoundary>,
    );

    expect(screen.getByTestId('error-boundary-fallback')).toBeInTheDocument();
    expect(screen.getByText('Sự Cố Hiển Thị Giao Diện')).toBeInTheDocument();
    expect(screen.getByTestId('error-banner')).toBeInTheDocument();
    expect(screen.getByTestId('error-message')).toHaveTextContent(
      'Cố tình gây lỗi trong component',
    );
    expect(screen.getByTestId('error-retry-btn')).toBeInTheDocument();

    consoleSpy.mockRestore();
  });

  it('should invoke onError callback when error is caught', () => {
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const onError = vi.fn();

    render(
      <ErrorBoundary onError={onError}>
        <ThrowingChild shouldThrow={true} message="Test error callback" />
      </ErrorBoundary>,
    );

    expect(onError).toHaveBeenCalledTimes(1);
    const [caughtError, errorInfo] = onError.mock.calls[0] ?? [];
    expect(caughtError).toBeInstanceOf(Error);
    expect((caughtError as Error).message).toBe('Test error callback');
    expect(errorInfo).toBeDefined();

    consoleSpy.mockRestore();
  });

  it('should support static fallback ReactNode', () => {
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    render(
      <ErrorBoundary fallback={<div data-testid="custom-static-fallback">Tùy biến tĩnh</div>}>
        <ThrowingChild shouldThrow={true} />
      </ErrorBoundary>,
    );

    expect(screen.getByTestId('custom-static-fallback')).toBeInTheDocument();
    expect(screen.queryByTestId('error-boundary-fallback')).toBeNull();

    consoleSpy.mockRestore();
  });

  it('should support function render prop fallback receiving error and resetErrorBoundary', () => {
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    render(
      <ErrorBoundary
        fallback={({ error, resetErrorBoundary }) => (
          <div data-testid="custom-render-fallback">
            <span data-testid="custom-error-code">{error.code}</span>
            <button
              type="button"
              data-testid="custom-retry-btn"
              onClick={resetErrorBoundary}
            >
              Phục hồi
            </button>
          </div>
        )}
      >
        <ThrowingChild shouldThrow={true} />
      </ErrorBoundary>,
    );

    expect(screen.getByTestId('custom-render-fallback')).toBeInTheDocument();
    expect(screen.getByTestId('custom-error-code')).toHaveTextContent('ERR_INTERNAL_ERROR');

    consoleSpy.mockRestore();
  });

  it('should reset error state when clicking "Thử lại" in default fallback', () => {
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const onReset = vi.fn();

    const Harness: React.FC = () => {
      const [hasExploded, setHasExploded] = useState<boolean>(true);

      return (
        <ErrorBoundary onReset={onReset}>
          <ThrowingChild
            shouldThrow={hasExploded}
            message="Lỗi có thể khắc phục khi retry"
          />
          <button
            type="button"
            data-testid="fix-issue-btn"
            onClick={() => setHasExploded(false)}
          >
            Sửa lỗi
          </button>
        </ErrorBoundary>
      );
    };

    render(<Harness />);

    expect(screen.getByTestId('error-boundary-fallback')).toBeInTheDocument();

    // Fix the issue and trigger retry
    const retryBtn = screen.getByTestId('error-retry-btn');
    fireEvent.click(retryBtn);

    expect(onReset).toHaveBeenCalledTimes(1);

    consoleSpy.mockRestore();
  });

  it('should automatically reset boundary when resetKeys prop changes', () => {
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    const Harness: React.FC = () => {
      const [key, setKey] = useState<number>(1);
      const [shouldThrow, setShouldThrow] = useState<boolean>(true);

      return (
        <div>
          <button
            type="button"
            data-testid="change-key-btn"
            onClick={() => {
              setShouldThrow(false);
              setKey((prev) => prev + 1);
            }}
          >
            Đổi Key
          </button>
          <ErrorBoundary resetKeys={[key]}>
            <ThrowingChild shouldThrow={shouldThrow} message="Lỗi reset key" />
          </ErrorBoundary>
        </div>
      );
    };

    render(<Harness />);

    expect(screen.getByTestId('error-boundary-fallback')).toBeInTheDocument();

    // Mutate resetKeys
    fireEvent.click(screen.getByTestId('change-key-btn'));

    // Boundary automatically resets and renders healthy child
    expect(screen.queryByTestId('error-boundary-fallback')).toBeNull();
    expect(screen.getByTestId('child-content')).toBeInTheDocument();

    consoleSpy.mockRestore();
  });
});
