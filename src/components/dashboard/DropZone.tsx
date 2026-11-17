/**
 * NeuroClip Offline Local Video DropZone Component (`TASK-M4-07`)
 *
 * Supports drag-and-drop (Tauri native Webview drag-drop + HTML5 fallback),
 * file picker (`<input type="file">` / optional native dialog hook), and direct path
 * submission for offline `.mp4`, `.mkv`, `.webm`, `.mov`, `.ts`, `.flv` files with
 * zero outbound network dependency.
 *
 * Strict adherence to docs/CONVENTIONS.md (zero-any, exactOptionalPropertyTypes).
 */

import React, { useCallback, useEffect, useRef, useState } from 'react';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import {
  CheckCircle2,
  FileAudio,
  FileVideo,
  FolderOpen,
  HardDrive,
  Loader2,
  UploadCloud,
} from 'lucide-react';
import { ErrorBanner } from '@/components/common/ErrorBanner';
import { ERROR_CODES, parseIpcError, type IpcErrorResponse } from '@/services/ipc/error.ts';
import {
  importLocalVideo,
  toLocalAssetUrl,
  type VodManifest,
} from '@/services/ipc/ingestion.ts';

export const SUPPORTED_VIDEO_EXTENSIONS = ['.mp4', '.mkv', '.webm', '.mov', '.ts', '.flv'] as const;

export interface DropZoneProps {
  onImportSuccess?: ((manifest: VodManifest) => void) | undefined;
  onSelectFilePath?: ((filePath: string) => void) | undefined;
  onBrowseRequest?: (() => Promise<string | null>) | undefined;
  customCacheDir?: string | null | undefined;
  disabled?: boolean | undefined;
  className?: string | undefined;
}

/**
 * Validate whether a file name or path ends with a supported local video extension.
 */
export function isSupportedVideoPath(filePath: string): boolean {
  const trimmed = filePath.trim().toLowerCase();
  if (!trimmed) return false;
  return SUPPORTED_VIDEO_EXTENSIONS.some((ext) => trimmed.endsWith(ext));
}

function createUnsupportedExtensionError(filePath: string): IpcErrorResponse {
  return {
    code: ERROR_CODES.INGEST_INVALID_URL,
    message: `Định dạng tệp không được hỗ trợ. Vui lòng chọn tệp video ${SUPPORTED_VIDEO_EXTENSIONS.join(', ')}.`,
    details: `Unsupported local video path: ${filePath}`,
    recoverable: false,
  };
}

function createMissingNativePathError(fileName: string): IpcErrorResponse {
  return {
    code: ERROR_CODES.STORAGE_FILE_NOT_FOUND,
    message:
      'Trình duyệt không cung cấp đường dẫn thư mục đầy đủ từ hộp thoại tệp. Vui lòng kéo thả tệp trực tiếp vào cửa sổ ứng dụng hoặc dán đường dẫn đầy đủ vào ô nhập bên dưới.',
    details: `DOM File object for "${fileName}" does not expose a native filesystem path.`,
    recoverable: true,
  };
}

function extractFilePath(file: File): string | null {
  const fileWithPath = file as File & { path?: unknown };
  if (typeof fileWithPath.path === 'string' && fileWithPath.path.trim().length > 0) {
    return fileWithPath.path.trim();
  }
  return null;
}

export const DropZone: React.FC<DropZoneProps> = ({
  onImportSuccess,
  onSelectFilePath,
  onBrowseRequest,
  customCacheDir,
  disabled = false,
  className = '',
}) => {
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const pathInputRef = useRef<HTMLInputElement | null>(null);
  const [isDragging, setIsDragging] = useState<boolean>(false);
  const [isImporting, setIsImporting] = useState<boolean>(false);
  const [manualPath, setManualPath] = useState<string>('');
  const [lastSelectedPath, setLastSelectedPath] = useState<string | null>(null);
  const [manifest, setManifest] = useState<VodManifest | null>(null);
  const [error, setError] = useState<IpcErrorResponse | null>(null);

  const processFilePath = useCallback(
    async (rawPath: string): Promise<void> => {
      const trimmed = rawPath.trim();
      if (!trimmed || disabled || isImporting) return;

      if (!isSupportedVideoPath(trimmed)) {
        setError(createUnsupportedExtensionError(trimmed));
        return;
      }

      setError(null);
      setLastSelectedPath(trimmed);
      if (onSelectFilePath) {
        onSelectFilePath(trimmed);
      }

      setIsImporting(true);
      try {
        const result = await importLocalVideo(trimmed, customCacheDir ?? null);
        setManifest(result);
        setManualPath(trimmed);
        if (onImportSuccess) {
          onImportSuccess(result);
        }
      } catch (err: unknown) {
        setError(parseIpcError(err, 'Không thể nhập tệp video cục bộ'));
      } finally {
        setIsImporting(false);
      }
    },
    [customCacheDir, disabled, isImporting, onImportSuccess, onSelectFilePath],
  );

  const processFilePathRef = useRef(processFilePath);
  processFilePathRef.current = processFilePath;

  // Tauri v2 native window file drag-drop listener (subscribed once per mount)
  useEffect(() => {
    let isMounted = true;
    let unlisten: (() => void) | undefined;

    const setupTauriDragDrop = async (): Promise<void> => {
      try {
        const webviewWindow = getCurrentWebviewWindow();
        const fn = await webviewWindow.onDragDropEvent((event) => {
          if (!isMounted) return;
          if (event.payload.type === 'enter') {
            setIsDragging(true);
          } else if (event.payload.type === 'leave') {
            setIsDragging(false);
          } else if (event.payload.type === 'drop') {
            setIsDragging(false);
            const firstPath = event.payload.paths[0];
            if (typeof firstPath === 'string' && firstPath.trim().length > 0) {
              void processFilePathRef.current(firstPath);
            }
          }
        });
        if (isMounted) {
          unlisten = fn;
        } else {
          fn();
        }
      } catch {
        // Non-Tauri / Vitest jsdom environment: safely ignore
      }
    };

    void setupTauriDragDrop();

    return () => {
      isMounted = false;
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  const handleFileSelection = (file: File): void => {
    const nativePath = extractFilePath(file);
    const targetForExtCheck = nativePath ?? file.name;

    if (!isSupportedVideoPath(targetForExtCheck)) {
      setError(createUnsupportedExtensionError(targetForExtCheck));
      return;
    }

    if (nativePath === null) {
      setLastSelectedPath(null);
      setError(createMissingNativePathError(file.name));
      pathInputRef.current?.focus();
      return;
    }

    void processFilePath(nativePath);
  };

  const handleDragEnter = (e: React.DragEvent<HTMLDivElement>): void => {
    e.preventDefault();
    e.stopPropagation();
    if (!disabled && !isImporting) {
      setIsDragging(true);
    }
  };

  const handleDragOver = (e: React.DragEvent<HTMLDivElement>): void => {
    e.preventDefault();
    e.stopPropagation();
    if (!disabled && !isImporting) {
      setIsDragging(true);
    }
  };

  const handleDragLeave = (e: React.DragEvent<HTMLDivElement>): void => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(false);
  };

  const handleDrop = (e: React.DragEvent<HTMLDivElement>): void => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(false);
    if (disabled || isImporting) return;

    const firstFile = e.dataTransfer?.files?.[0];
    if (firstFile) {
      handleFileSelection(firstFile);
    }
  };

  const handleFileInputChange = (e: React.ChangeEvent<HTMLInputElement>): void => {
    const firstFile = e.target.files?.[0];
    if (firstFile) {
      handleFileSelection(firstFile);
    }
    // Reset input value so selecting the same file again still triggers onChange
    e.target.value = '';
  };

  const handleBrowseClick = (): void => {
    if (disabled || isImporting) return;
    if (onBrowseRequest) {
      void onBrowseRequest()
        .then((selectedPath) => {
          if (typeof selectedPath === 'string' && selectedPath.trim().length > 0) {
            void processFilePath(selectedPath);
          }
        })
        .catch((err: unknown) => {
          setError(parseIpcError(err, 'Không thể mở hộp thoại chọn tệp'));
        });
      return;
    }
    fileInputRef.current?.click();
  };

  const handleManualSubmit = (e: React.FormEvent<HTMLFormElement>): void => {
    e.preventDefault();
    if (!manualPath.trim() || disabled || isImporting) return;
    void processFilePath(manualPath);
  };

  const handleRetry = (): void => {
    if (lastSelectedPath) {
      void processFilePath(lastSelectedPath);
    }
  };

  const localAssetPreviewUrl = manifest?.localVideoPath
    ? toLocalAssetUrl(manifest.localVideoPath)
    : '';

  return (
    <div
      data-testid="local-dropzone-container"
      className={`bg-bg-secondary/80 border border-border-dark rounded-xl p-5 space-y-4 text-slate-100 backdrop-blur-md shadow-xl ${className}`}
    >
      {/* Header */}
      <div className="flex items-center justify-between border-b border-border-dark pb-3">
        <div className="flex items-center space-x-2.5">
          <div className="p-2 rounded-lg bg-accent-neuro/10 text-accent-neuro">
            <HardDrive className="w-5 h-5" />
          </div>
          <div>
            <h2 className="text-sm font-semibold tracking-wide text-slate-100 flex items-center gap-2">
              Nhập Video Cục Bộ (Offline)
              <span className="text-[10px] px-2 py-0.5 rounded-full bg-accent-vedal/10 text-accent-vedal border border-accent-vedal/20">
                100% Offline
              </span>
            </h2>
            <p className="text-xs text-slate-400">
              Kéo thả hoặc chọn tệp video có sẵn trên máy để trích xuất âm thanh 16kHz &amp; phát qua{' '}
              <code className="text-accent-neuro">asset://</code>
            </p>
          </div>
        </div>
      </div>

      {/* Error Banner */}
      {error && (
        <ErrorBanner
          error={error}
          onDismiss={() => setError(null)}
          onRetry={error.recoverable && lastSelectedPath ? handleRetry : undefined}
        />
      )}

      {/* Drag & Drop Surface */}
      <div
        role="button"
        tabIndex={disabled || isImporting ? -1 : 0}
        aria-disabled={disabled || isImporting}
        data-testid="dropzone-area"
        data-dragging={isDragging ? 'true' : 'false'}
        onDragEnter={handleDragEnter}
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
        onClick={handleBrowseClick}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            handleBrowseClick();
          }
        }}
        className={`relative rounded-xl border-2 border-dashed p-6 text-center transition-all cursor-pointer select-none ${
          isDragging
            ? 'border-accent-neuro bg-accent-neuro/10'
            : 'border-border-dark bg-bg-tertiary/40 hover:border-accent-neuro/60 hover:bg-bg-tertiary/70'
        } ${disabled || isImporting ? 'opacity-60 cursor-not-allowed' : ''}`}
      >
        <input
          ref={fileInputRef}
          type="file"
          data-testid="dropzone-file-input"
          accept={SUPPORTED_VIDEO_EXTENSIONS.join(',')}
          onChange={handleFileInputChange}
          disabled={disabled || isImporting}
          className="hidden"
        />

        <div className="flex flex-col items-center justify-center space-y-2.5">
          <div className="p-3 rounded-full bg-bg-secondary border border-border-dark text-accent-neuro">
            {isImporting ? (
              <Loader2 className="w-6 h-6 animate-spin" data-testid="dropzone-loading-icon" />
            ) : (
              <UploadCloud className="w-6 h-6" />
            )}
          </div>

          <div className="space-y-1">
            <p className="text-sm font-medium text-slate-100">
              {isImporting
                ? 'Đang trích xuất âm thanh 16kHz PCM WAV...'
                : isDragging
                  ? 'Thả tệp video vào đây để bắt đầu nhập'
                  : 'Kéo thả tệp video vào đây hoặc nhấn để chọn tệp'}
            </p>
            <p className="text-xs text-slate-400">
              Hỗ trợ định dạng: {SUPPORTED_VIDEO_EXTENSIONS.join(' • ')}
            </p>
          </div>

          <div className="flex items-center justify-center gap-1.5 pt-1">
            {SUPPORTED_VIDEO_EXTENSIONS.map((ext) => (
              <span
                key={ext}
                data-testid={`supported-ext-${ext.slice(1)}`}
                className="text-[11px] font-mono px-2 py-0.5 rounded bg-bg-secondary text-slate-300 border border-border-dark"
              >
                {ext}
              </span>
            ))}
          </div>
        </div>
      </div>

      {/* Direct Path Input Fallback */}
      <form onSubmit={handleManualSubmit} className="flex items-center gap-2">
        <input
          ref={pathInputRef}
          type="text"
          data-testid="dropzone-path-input"
          value={manualPath}
          onChange={(e) => setManualPath(e.target.value)}
          disabled={disabled || isImporting}
          placeholder="Hoặc dán đường dẫn tệp video cục bộ (VD: D:\Videos\neuro_vod.mp4)"
          className="flex-1 bg-bg-primary border border-border-dark rounded-lg px-3 py-2 text-xs text-slate-100 placeholder:text-slate-500 focus:outline-none focus:border-accent-neuro"
        />
        <button
          type="submit"
          data-testid="dropzone-path-submit"
          disabled={disabled || isImporting || manualPath.trim().length === 0}
          className="px-3 py-2 rounded-lg text-xs font-semibold bg-accent-neuro/15 text-accent-neuro border border-accent-neuro/40 hover:bg-accent-neuro/25 disabled:opacity-40 disabled:cursor-not-allowed transition-colors flex items-center gap-1.5"
        >
          <FolderOpen className="w-3.5 h-3.5" />
          <span>Nhập Tệp</span>
        </button>
      </form>

      {/* Imported Manifest Summary */}
      {manifest && (
        <div
          data-testid="dropzone-manifest-card"
          className="rounded-lg bg-bg-primary/80 border border-accent-vedal/30 p-3.5 space-y-2 text-xs"
        >
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2 text-accent-vedal font-semibold">
              <CheckCircle2 className="w-4 h-4" />
              <span>Đã nhập video cục bộ thành công</span>
            </div>
            <span className="font-mono text-[11px] px-2 py-0.5 rounded bg-bg-tertiary text-slate-300 border border-border-dark">
              {manifest.sourceType}
            </span>
          </div>

          <div className="grid grid-cols-1 gap-1.5 text-slate-300">
            <div className="flex items-center gap-2 truncate">
              <FileVideo className="w-3.5 h-3.5 text-accent-neuro flex-shrink-0" />
              <span className="truncate font-medium text-slate-100">{manifest.title}</span>
            </div>
            {manifest.localVideoPath && (
              <div className="text-[11px] font-mono text-slate-400 truncate">
                Video: {manifest.localVideoPath}
              </div>
            )}
            <div className="flex items-center gap-2 text-[11px] font-mono text-slate-400 truncate">
              <FileAudio className="w-3.5 h-3.5 text-accent-vedal flex-shrink-0" />
              <span className="truncate">WAV 16kHz: {manifest.audioPath}</span>
            </div>
            {localAssetPreviewUrl && (
              <div
                data-testid="dropzone-asset-url"
                className="text-[11px] font-mono text-accent-chat truncate"
              >
                Stream: {localAssetPreviewUrl}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
};

export default DropZone;
