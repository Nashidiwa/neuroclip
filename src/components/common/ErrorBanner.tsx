import { useState, type FC } from 'react';
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
        setIsCopied(true);
        setTimeout(() => setIsCopied(false), 2000);
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
      className={`rounded-lg bg-bg-secondary/95 border border-border-dark border-l-4 ${borderAccentClass} shadow-md overflow-hidden transition-all text-slate-100 ${className}`}
    >
      <div className={`flex items-start justify-between gap-3 ${compact ? 'p-3' : 'p-4'}`}>
        {/* Left side: Icon + Content */}
        <div className="flex items-start gap-3 flex-1 min-w-0">
          <div className={`mt-0.5 flex-shrink-0 ${iconColorClass}`}>
            {isRecoverable ? (
              <AlertTriangle className="w-5 h-5" data-testid="icon-recoverable" />
            ) : (
              <AlertCircle className="w-5 h-5" data-testid="icon-unrecoverable" />
            )}
          </div>

          <div className="flex-1 min-w-0 space-y-1.5">
            {/* Header: Title / Badge */}
            <div className="flex items-center flex-wrap gap-2">
              {title !== undefined && title.length > 0 && (
                <span className="font-semibold text-xs text-white tracking-wide">
                  {title}
                </span>
              )}
              <span
                data-testid="error-code-badge"
                className="font-mono text-[11px] font-medium px-2 py-0.5 rounded bg-bg-tertiary text-accent-neuro border border-border-dark select-all"
              >
                {errorObj.code}
              </span>
              {isRecoverable ? (
                <span className="text-[10px] font-medium px-1.5 py-0.2 rounded bg-accent-vedal/10 text-accent-vedal border border-accent-vedal/30">
                  Có thể thử lại
                </span>
              ) : (
                <span className="text-[10px] font-medium px-1.5 py-0.2 rounded bg-accent-evil/10 text-accent-evil border border-accent-evil/30">
                  Lỗi nghiêm trọng
                </span>
              )}
            </div>

            {/* Error Message in Vietnamese */}
            <p className="text-xs text-slate-200 leading-relaxed break-words" data-testid="error-message">
              {errorObj.message}
            </p>

            {/* Collapsible Details Action */}
            {hasDetails && (
              <div className="pt-1">
                <button
                  type="button"
                  data-testid="toggle-details-btn"
                  onClick={() => setIsDetailsOpen((prev) => !prev)}
                  className="text-[11px] text-slate-400 hover:text-white flex items-center gap-1 transition-colors underline-offset-2 hover:underline focus:outline-none"
                >
                  {isDetailsOpen ? (
                    <>
                      <ChevronUp className="w-3.5 h-3.5" />
                      <span>Ẩn chi tiết kỹ thuật</span>
                    </>
                  ) : (
                    <>
                      <ChevronDown className="w-3.5 h-3.5" />
                      <span>Xem chi tiết lỗi kỹ thuật</span>
                    </>
                  )}
                </button>
              </div>
            )}
          </div>
        </div>

        {/* Right side: Actions (Retry, Dismiss) */}
        <div className="flex items-center gap-2 flex-shrink-0 ml-2">
          {showRetry && (
            <button
              type="button"
              data-testid="error-retry-btn"
              onClick={onRetry}
              title="Thử lại thao tác này"
              className="px-3 py-1.5 text-xs font-semibold rounded bg-accent-vedal/15 text-accent-vedal border border-accent-vedal/40 hover:bg-accent-vedal/25 active:scale-95 transition-all flex items-center gap-1.5 shadow-sm"
            >
              <RefreshCw className="w-3 h-3 animate-none" />
              <span>Thử lại</span>
            </button>
          )}

          {onDismiss !== undefined && (
            <button
              type="button"
              data-testid="error-dismiss-btn"
              onClick={onDismiss}
              title="Đóng thông báo"
              className="p-1.5 text-slate-400 hover:text-white hover:bg-bg-tertiary rounded transition-colors"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          )}
        </div>
      </div>

      {/* Expanded Technical Details Panel */}
      {hasDetails && isDetailsOpen && (
        <div
          data-testid="error-details-panel"
          className="border-t border-border-dark bg-bg-primary/95 p-3 space-y-2 select-text"
        >
          <div className="flex items-center justify-between text-[11px] text-slate-400">
            <span className="font-mono">Technical Details / Stack Trace:</span>
            <button
              type="button"
              data-testid="copy-details-btn"
              onClick={handleCopy}
              className="flex items-center gap-1 text-[11px] text-slate-300 hover:text-white bg-bg-tertiary px-2 py-0.5 rounded border border-border-dark transition-colors"
            >
              {isCopied ? (
                <>
                  <Check className="w-3 h-3 text-accent-vedal" />
                  <span className="text-accent-vedal">Đã sao chép!</span>
                </>
              ) : (
                <>
                  <Copy className="w-3 h-3" />
                  <span>Sao chép mã lỗi</span>
                </>
              )}
            </button>
          </div>

          <pre className="text-[11px] font-mono text-slate-300 bg-bg-secondary p-2.5 rounded border border-border-dark/60 overflow-x-auto max-h-48 whitespace-pre-wrap break-all leading-normal">
            {errorObj.details}
          </pre>
        </div>
      )}
    </div>
  );
};

export default ErrorBanner;
