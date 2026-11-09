import { Component, type ErrorInfo, type ReactNode } from 'react';
import { AlertTriangle } from 'lucide-react';
import { IpcErrorResponse, parseIpcError } from '@/services/ipc/error';
import { ErrorBanner } from '@/components/common/ErrorBanner';

export interface ErrorBoundaryFallbackProps {
  error: IpcErrorResponse;
  resetErrorBoundary: () => void;
}

export interface ErrorBoundaryProps {
  children: ReactNode;
  /** Custom fallback renderer function or static ReactNode */
  fallback?: ReactNode | ((props: ErrorBoundaryFallbackProps) => ReactNode) | undefined;
  /** Optional callback fired when an error is caught */
  onError?: ((error: Error, errorInfo: ErrorInfo) => void) | undefined;
  /** Optional callback fired when user triggers retry/reset */
  onReset?: (() => void) | undefined;
  /** Custom section title displayed in the default fallback card */
  title?: string | undefined;
  /** Array of values that automatically reset the boundary when mutated */
  resetKeys?: readonly unknown[] | undefined;
}

export interface ErrorBoundaryState {
  hasError: boolean;
  error: IpcErrorResponse | null;
}

/**
 * React Error Boundary capturing render exceptions and rendering a fallback UI.
 * Integrates Vietnamese messaging and retry mechanism per docs/CONVENTIONS.md §5.3.
 */
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  public override state: ErrorBoundaryState = {
    hasError: false,
    error: null,
  };

  public static getDerivedStateFromError(error: unknown): ErrorBoundaryState {
    const parsed = parseIpcError(error, 'Đã xảy ra sự cố kết xuất giao diện');
    return {
      hasError: true,
      error: {
        ...parsed,
        recoverable: true,
      },
    };
  }

  public override componentDidCatch(error: Error, errorInfo: ErrorInfo): void {
    if (this.props.onError) {
      this.props.onError(error, errorInfo);
    }
    // Telemetry log output without throwing or panicking
    console.error('[ErrorBoundary caught exception]:', error, errorInfo);
  }

  public override componentDidUpdate(prevProps: ErrorBoundaryProps): void {
    const { resetKeys } = this.props;
    if (this.state.hasError && resetKeys !== undefined) {
      const prevKeys = prevProps.resetKeys;
      if (prevKeys === undefined || prevKeys.length !== resetKeys.length) {
        this.resetErrorBoundary();
        return;
      }

      for (let i = 0; i < resetKeys.length; i += 1) {
        if (resetKeys[i] !== prevKeys[i]) {
          this.resetErrorBoundary();
          return;
        }
      }
    }
  }

  public resetErrorBoundary = (): void => {
    if (this.props.onReset) {
      this.props.onReset();
    }
    this.setState({
      hasError: false,
      error: null,
    });
  };

  public override render(): ReactNode {
    if (!this.state.hasError) {
      return this.props.children;
    }

    const { error } = this.state;
    const baseError: IpcErrorResponse =
      error ?? parseIpcError(new Error('Lỗi hiển thị không xác định'));
    const safeError: IpcErrorResponse = {
      ...baseError,
      recoverable: true,
    };

    const { fallback, title } = this.props;

    if (typeof fallback === 'function') {
      return fallback({
        error: safeError,
        resetErrorBoundary: this.resetErrorBoundary,
      });
    }

    if (fallback !== undefined && fallback !== null) {
      return fallback;
    }

    // Default Fallback Card styled with NeuroClip Dark Acrylic Theme
    return (
      <div
        role="alert"
        aria-live="assertive"
        className="m-4 p-6 rounded-xl bg-bg-secondary/90 border border-border-dark text-slate-100 shadow-acrylic flex flex-col space-y-4"
        data-testid="error-boundary-fallback"
      >
        <div className="flex items-center space-x-2 text-accent-evil">
          <AlertTriangle className="w-5 h-5 flex-shrink-0" />
          <h2 className="text-base font-semibold">
            {title ?? 'Sự Cố Hiển Thị Giao Diện'}
          </h2>
        </div>

        <p className="text-sm text-slate-300 leading-relaxed">
          NeuroClip đã ghi nhận sự cố bất ngờ trong phân vùng hiển thị này.
          Dữ liệu phiên làm việc và các tiến trình nền vẫn đang được bảo toàn.
        </p>

        <ErrorBanner
          error={safeError}
          onRetry={this.resetErrorBoundary}
          title="Chi tiết mã lỗi phát sinh"
        />
      </div>
    );
  }
}

export default ErrorBoundary;
