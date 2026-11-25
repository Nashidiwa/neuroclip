import React, { useState } from 'react';
import {
  Key,
  Eye,
  EyeOff,
  RefreshCw,
  CheckCircle2,
  XCircle,
  AlertTriangle,
  ExternalLink,
} from 'lucide-react';
import { testApiKeyConnection } from '../../services/config/settings.ts';
import { ERROR_CODES, parseIpcError } from '../../services/ipc/error.ts';
import { sanitizeLogString } from '../../services/logging/redactor.ts';

export type ApiKeyStatus = 'idle' | 'testing' | 'valid' | 'invalid' | 'error';

export interface ApiKeyTesterProps {
  readonly provider: 'groq' | 'gemini';
  readonly label: string;
  readonly value: string;
  readonly onChange: (value: string) => void;
  readonly getKeyUrl: string;
  readonly placeholder?: string | undefined;
  readonly helpText?: string | undefined;
  readonly onTest?: (
    provider: 'groq' | 'gemini',
    key: string,
  ) => Promise<{ success: boolean; latencyMs?: number | undefined; error?: string | undefined }>;
}

export const ApiKeyTester: React.FC<ApiKeyTesterProps> = ({
  provider,
  label,
  value,
  onChange,
  getKeyUrl,
  placeholder = 'Nhập API key...',
  helpText,
  onTest,
}) => {
  const [showKey, setShowKey] = useState<boolean>(false);
  const [status, setStatus] = useState<ApiKeyStatus>('idle');
  const [latencyMs, setLatencyMs] = useState<number | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const handleTestConnection = async (): Promise<void> => {
    const trimmed = value.trim();
    if (!trimmed) {
      return;
    }

    setStatus('testing');
    setErrorMessage(null);
    setLatencyMs(null);

    const startTime = performance.now();

    try {
      if (onTest) {
        const result = await onTest(provider, trimmed);
        const elapsed = result.latencyMs ?? Math.round(performance.now() - startTime);
        setLatencyMs(elapsed);

        if (result.success) {
          setStatus('valid');
        } else {
          setStatus('invalid');
          setErrorMessage(result.error ?? 'Khóa API không hợp lệ hoặc bị từ chối');
        }
      } else {
        const success = await testApiKeyConnection(provider, trimmed);
        const elapsed = Math.round(performance.now() - startTime);
        setLatencyMs(elapsed);

        if (success) {
          setStatus('valid');
        } else {
          setStatus('invalid');
          setErrorMessage('Khóa API không hợp lệ hoặc bị từ chối');
        }
      }
    } catch (err: unknown) {
      const elapsed = Math.round(performance.now() - startTime);
      setLatencyMs(elapsed);
      const parsed = parseIpcError(err, 'Lỗi kết nối tới máy chủ AI');
      const isInvalidKey =
        parsed.code === ERROR_CODES.STT_INVALID_API_KEY ||
        parsed.code === ERROR_CODES.TRANSLATE_INVALID_API_KEY;
      setStatus(isInvalidKey ? 'invalid' : 'error');
      const cleanMsg = sanitizeLogString(parsed.message);
      const cleanDetails = parsed.details ? sanitizeLogString(parsed.details).trim() : '';
      const formatted =
        cleanDetails.length > 0 && !cleanMsg.includes(cleanDetails)
          ? `${cleanMsg} (${cleanDetails})`
          : cleanMsg;
      setErrorMessage(formatted.length > 0 ? formatted : 'Lỗi kết nối tới máy chủ AI');
    }
  };

  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>): void => {
    onChange(e.target.value);
    if (status !== 'idle') {
      setStatus('idle');
      setLatencyMs(null);
      setErrorMessage(null);
    }
  };

  return (
    <div className="text-text-primary space-y-2 rounded-lg border border-border-dark bg-bg-secondary p-4 shadow-sm">
      <div className="flex items-center justify-between">
        <label className="text-text-primary flex items-center gap-2 text-sm font-semibold">
          <Key className="h-4 w-4 text-accent-neuro" />
          <span>{label}</span>
        </label>
        <a
          href={getKeyUrl}
          target="_blank"
          rel="noopener noreferrer"
          className="inline-flex items-center gap-1 text-xs text-accent-chat transition-colors hover:text-accent-neuro hover:underline"
          title={`Lấy khóa ${provider === 'groq' ? 'Groq' : 'Gemini'} chính thức`}
        >
          <span>Lấy Free API Key</span>
          <ExternalLink className="h-3 w-3" />
        </a>
      </div>

      <div className="relative flex items-center">
        <input
          type={showKey ? 'text' : 'password'}
          value={value}
          onChange={handleInputChange}
          placeholder={placeholder}
          className="text-text-primary placeholder:text-text-muted w-full rounded-md border border-border-dark bg-bg-primary px-3 py-2 pr-10 text-sm focus:border-accent-neuro focus:outline-none focus:ring-1 focus:ring-accent-neuro"
        />
        <button
          type="button"
          onClick={() => setShowKey(!showKey)}
          className="text-text-muted hover:text-text-primary absolute right-3 transition-colors focus:outline-none"
          title={showKey ? 'Ẩn khóa' : 'Hiện khóa'}
        >
          {showKey ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
        </button>
      </div>

      {helpText && <p className="text-text-muted text-xs">{helpText}</p>}

      <div className="flex flex-wrap items-center justify-between gap-2 pt-1">
        <div>
          {status === 'idle' && (
            <span className="text-text-muted text-xs">Chưa kiểm tra kết nối</span>
          )}
          {status === 'testing' && (
            <span className="inline-flex items-center gap-1.5 text-xs text-accent-neuro">
              <RefreshCw className="h-3.5 w-3.5 animate-spin" />
              <span>Đang kiểm tra kết nối...</span>
            </span>
          )}
          {status === 'valid' && (
            <span className="inline-flex items-center gap-1.5 text-xs font-medium text-accent-vedal">
              <CheckCircle2 className="h-3.5 w-3.5" />
              <span>[Sẵn sàng] {latencyMs !== null ? `(${latencyMs} ms)` : ''}</span>
            </span>
          )}
          {status === 'invalid' && (
            <span className="inline-flex items-center gap-1.5 text-xs font-medium text-accent-evil">
              <XCircle className="h-3.5 w-3.5" />
              <span>[Không hợp lệ]</span>
            </span>
          )}
          {status === 'error' && (
            <span className="inline-flex items-center gap-1.5 text-xs font-medium text-accent-guest">
              <AlertTriangle className="h-3.5 w-3.5" />
              <span>[Lỗi kết nối]</span>
            </span>
          )}
        </div>

        <button
          type="button"
          onClick={handleTestConnection}
          disabled={!value.trim() || status === 'testing'}
          className="text-text-primary inline-flex items-center gap-1.5 rounded bg-bg-tertiary px-3 py-1.5 text-xs font-medium transition-all hover:bg-border-dark active:scale-95 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {status === 'testing' ? (
            <RefreshCw className="h-3.5 w-3.5 animate-spin" />
          ) : (
            <RefreshCw className="h-3.5 w-3.5" />
          )}
          <span>Kiểm tra kết nối</span>
        </button>
      </div>

      {errorMessage && (
        <div className="rounded border border-accent-evil/30 bg-accent-evil/10 p-2 text-xs text-accent-evil">
          {errorMessage}
        </div>
      )}
    </div>
  );
};
