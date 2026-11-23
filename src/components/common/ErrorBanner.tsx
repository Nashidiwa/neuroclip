import { useEffect, useRef, useState, type FC } from 'react';
import {
  AlertTriangle,
  AlertCircle,
  RefreshCw,
  ChevronDown,
  ChevronUp,
  Copy,
  Check,
  X,
} from 'lucide-react';
import { IpcErrorResponse, isIpcError, parseIpcError } from '@/services/ipc/error';

export interface ErrorBannerProps {
  /** Error payload (IpcErrorResponse, standard Error, raw object, string, or null/undefined) */
  error: IpcErrorResponse | string | Error | Record<string, unknown> | null | undefined | unknown;
  /** Optional custom title override */
  title?: string | undefined;
  /** Retry callback invoked when user clicks "Thử lại" */
  onRetry?: (() => void) | undefined;
  /** Dismiss callback invoked when user clicks close */
  onDismiss?: (() => void) | undefined;
  /** Optional container CSS class name */
  className?: string | undefined;
  /** Compact rendering mode for small widget spaces */
  compact?: boolean | undefined;
}

/**
 * ErrorBanner Component
 * Displays standardized error banner with icon, code tag, Vietnamese message,
 * collapsible technical details, and "Thử lại (Retry)" button adhering to
 * NeuroClip VTuber Dark Theme (#12131C, #1A1C28, #FFB6C1, #FF0055, #98FB98).
 */
export const ErrorBanner: FC<ErrorBannerProps> = ({
  error,
  title,
  onRetry,
  onDismiss,
  className = '',
  compact = false,
}) => {
  const [isDetailsOpen, setIsDetailsOpen] = useState<boolean>(false);
  const [isCopied, setIsCopied] = useState<boolean>(false);
  const mountedRef = useRef<boolean>(true);
  const copyTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      if (copyTimerRef.current !== null) {
        clearTimeout(copyTimerRef.current);
        copyTimerRef.current = null;
      }
    };
  }, []);

  if (error === null || error === undefined) {
    return null;
  }

  const errorObj = isIpcError(error) ? error : parseIpcError(error);
  const isRecoverable = errorObj.recoverable;
  const hasDetails = typeof errorObj.details === 'string' && errorObj.details.trim().length > 0;
  const showRetry = onRetry !== undefined && isRecoverable;

  const handleCopy = async (): Promise<void> => {
    try {
      if (
        typeof navigator !== 'undefined' &&
        navigator.clipboard &&
        typeof navigator.clipboard.writeText === 'function'
      ) {
        const payloadToCopy = JSON.stringify(errorObj, null, 2);
        await navigator.clipboard.writeText(payloadToCopy);
        if (!mountedRef.current) return;
        setIsCopied(true);
        if (copyTimerRef.current !== null) {
          clearTimeout(copyTimerRef.current);
        }
        copyTimerRef.current = setTimeout(() => {
          copyTimerRef.current = null;
          if (mountedRef.current) {
            setIsCopied(false);
          }
        }, 2000);
      }
    } catch {
      // Non-clipboard browser environments fallback
    }
  };

  const borderAccentClass = isRecoverable ? 'border-l-accent-neuro' : 'border-l-accent-evil';
  const iconColorClass = isRecoverable ? 'text-accent-neuro' : 'text-accent-evil';

  return (
    <div
      role="alert"
      data-testid="error-banner"
      className={`rounded-lg border border-l-4 border-border-dark bg-bg-secondary/95 ${borderAccentClass} overflow-hidden text-slate-100 shadow-md transition-all ${className}`}
    >
      <div className={`flex items-start justify-between gap-3 ${compact ? 'p-3' : 'p-4'}`}>
        {/* Left side: Icon + Content */}
        <div className="flex min-w-0 flex-1 items-start gap-3">
          <div className={`mt-0.5 flex-shrink-0 ${iconColorClass}`}>
            {isRecoverable ? (
              <AlertTriangle className="h-5 w-5" data-testid="icon-recoverable" />
            ) : (
              <AlertCircle className="h-5 w-5" data-testid="icon-unrecoverable" />
            )}
          </div>

          <div className="min-w-0 flex-1 space-y-1.5">
            {/* Header: Title / Badge */}
            <div className="flex flex-wrap items-center gap-2">
              {title !== undefined && title.length > 0 && (
                <span className="text-xs font-semibold tracking-wide text-white">{title}</span>
              )}
              <span
                data-testid="error-code-badge"
                className="select-all rounded border border-border-dark bg-bg-tertiary px-2 py-0.5 font-mono text-[11px] font-medium text-accent-neuro"
              >
                {errorObj.code}
              </span>
              {isRecoverable ? (
                <span className="py-0.2 rounded border border-accent-vedal/30 bg-accent-vedal/10 px-1.5 text-[10px] font-medium text-accent-vedal">
                  Có thể thử lại
                </span>
              ) : (
                <span className="py-0.2 rounded border border-accent-evil/30 bg-accent-evil/10 px-1.5 text-[10px] font-medium text-accent-evil">
                  Lỗi nghiêm trọng
                </span>
              )}
            </div>

            {/* Error Message in Vietnamese */}
            <p
              className="break-words text-xs leading-relaxed text-slate-200"
              data-testid="error-message"
            >
              {errorObj.message}
            </p>

            {/* Collapsible Details Action */}
            {hasDetails && (
              <div className="pt-1">
                <button
                  type="button"
                  data-testid="toggle-details-btn"
                  onClick={() => setIsDetailsOpen((prev) => !prev)}
                  className="flex items-center gap-1 text-[11px] text-slate-400 underline-offset-2 transition-colors hover:text-white hover:underline focus:outline-none"
                >
                  {isDetailsOpen ? (
                    <>
                      <ChevronUp className="h-3.5 w-3.5" />
                      <span>Ẩn chi tiết kỹ thuật</span>
                    </>
                  ) : (
                    <>
                      <ChevronDown className="h-3.5 w-3.5" />
                      <span>Xem chi tiết lỗi kỹ thuật</span>
                    </>
                  )}
                </button>
              </div>
            )}
          </div>
        </div>

        {/* Right side: Actions (Retry, Dismiss) */}
        <div className="ml-2 flex flex-shrink-0 items-center gap-2">
          {showRetry && (
            <button
              type="button"
              data-testid="error-retry-btn"
              onClick={onRetry}
              title="Thử lại thao tác này"
              className="flex items-center gap-1.5 rounded border border-accent-vedal/40 bg-accent-vedal/15 px-3 py-1.5 text-xs font-semibold text-accent-vedal shadow-sm transition-all hover:bg-accent-vedal/25 active:scale-95"
            >
              <RefreshCw className="h-3 w-3 animate-none" />
              <span>Thử lại</span>
            </button>
          )}

          {onDismiss !== undefined && (
            <button
              type="button"
              data-testid="error-dismiss-btn"
              onClick={onDismiss}
              title="Đóng thông báo"
              className="rounded p-1.5 text-slate-400 transition-colors hover:bg-bg-tertiary hover:text-white"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          )}
        </div>
      </div>

      {/* Expanded Technical Details Panel */}
      {hasDetails && isDetailsOpen && (
        <div
          data-testid="error-details-panel"
          className="select-text space-y-2 border-t border-border-dark bg-bg-primary/95 p-3"
        >
          <div className="flex items-center justify-between text-[11px] text-slate-400">
            <span className="font-mono">Technical Details / Stack Trace:</span>
            <button
              type="button"
              data-testid="copy-details-btn"
              onClick={handleCopy}
              className="flex items-center gap-1 rounded border border-border-dark bg-bg-tertiary px-2 py-0.5 text-[11px] text-slate-300 transition-colors hover:text-white"
            >
              {isCopied ? (
                <>
                  <Check className="h-3 w-3 text-accent-vedal" />
                  <span className="text-accent-vedal">Đã sao chép!</span>
                </>
              ) : (
                <>
                  <Copy className="h-3 w-3" />
                  <span>Sao chép mã lỗi</span>
                </>
              )}
            </button>
          </div>

          <pre className="max-h-48 overflow-x-auto whitespace-pre-wrap break-all rounded border border-border-dark/60 bg-bg-secondary p-2.5 font-mono text-[11px] leading-normal text-slate-300">
            {errorObj.details}
          </pre>
        </div>
      )}
    </div>
  );
};

export default ErrorBanner;
