import React, { useState, useEffect, useCallback, useRef } from 'react';
import {
  X,
  Share2,
  ShieldCheck,
  CheckCircle2,
  RefreshCw,
  Folder,
  FileText,
  Copy,
  Check,
  Film,
  Layers,
  Clapperboard,
} from 'lucide-react';
import {
  exportCapCutDraft,
  exportPremiereXml,
  exportSubtitlesProjectAss,
  exportSubtitlesProjectSrt,
  listenExportProgress,
} from '../../services/ipc/export.ts';
import { parseIpcError, type IpcErrorResponse } from '../../services/ipc/error.ts';
import { ErrorBanner } from '../common/ErrorBanner.tsx';
import type { SubtitleProject } from '../../types/transcript.ts';
import type {
  CapCutExportResult,
  PremiereExportResult,
  ExportProgressPayload,
} from '../../types/export.ts';

export type ExportFormat = 'capcut' | 'premiere_xml' | 'ass' | 'srt';

export interface ExportModalProps {
  readonly isOpen: boolean;
  readonly onClose: () => void;
  readonly project: SubtitleProject;
  readonly videoFilePath?: string | undefined;
  readonly defaultDraftRoot?: string | undefined;
  readonly onExportSuccess?: ((result: CapCutExportResult | string) => void) | undefined;
}

function stripUnsafeFilenameChars(value: string): string {
  let replaced = '';
  for (const ch of value) {
    const code = ch.charCodeAt(0);
    if (code < 0x20 || '<>:"/\\|?*'.includes(ch)) {
      replaced += '_';
    } else {
      replaced += ch;
    }
  }
  return replaced.replace(/\.{2,}/g, '').replace(/^[\s.]+|[\s.]+$/g, '');
}

export function sanitizeFilenameSegment(rawName: string, fallback = 'neuroclip_subtitles'): string {
  const primary = stripUnsafeFilenameChars(rawName);
  if (primary.length > 0) {
    return primary;
  }

  const secondary = stripUnsafeFilenameChars(fallback);
  if (secondary.length > 0) {
    return secondary;
  }

  return 'neuroclip_subtitles';
}

function resolveDefaultOutputPath(defaultName: string, format: ExportFormat): string {
  const safeName = sanitizeFilenameSegment(defaultName);
  if (format === 'premiere_xml') {
    return `C:\\NeuroClip\\export\\${safeName}_timeline.xml`;
  }
  if (format === 'ass') {
    return `C:\\NeuroClip\\export\\${safeName}.ass`;
  }
  return `C:\\NeuroClip\\export\\${safeName}.srt`;
}

export const ExportModal: React.FC<ExportModalProps> = ({
  isOpen,
  onClose,
  project,
  videoFilePath: propVideoFilePath,
  defaultDraftRoot,
  onExportSuccess,
}) => {
  const [format, setFormat] = useState<ExportFormat>('capcut');
  const [customDraftRoot, setCustomDraftRoot] = useState<string>(defaultDraftRoot ?? '');
  const [customOutputPath, setCustomOutputPath] = useState<string>('');
  const [isExporting, setIsExporting] = useState<boolean>(false);
  const [exportProgress, setExportProgress] = useState<ExportProgressPayload | null>(null);
  const [exportError, setExportError] = useState<IpcErrorResponse | null>(null);
  const [capcutResult, setCapcutResult] = useState<CapCutExportResult | null>(null);
  const [premiereResult, setPremiereResult] = useState<PremiereExportResult | null>(null);
  const [standaloneResultPath, setStandaloneResultPath] = useState<string | null>(null);
  const [isCopied, setIsCopied] = useState<boolean>(false);

  const mountedRef = useRef<boolean>(true);
  const isOpenRef = useRef<boolean>(isOpen);
  const exportRunIdRef = useRef<number>(0);
  const copyTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const prevIsOpenRef = useRef<boolean>(false);
  const prevProjectIdRef = useRef<string>(project.projectId);
  const prevFormatRef = useRef<ExportFormat>(format);

  isOpenRef.current = isOpen;

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      exportRunIdRef.current += 1;
      if (copyTimerRef.current !== null) {
        clearTimeout(copyTimerRef.current);
        copyTimerRef.current = null;
      }
    };
  }, []);

  // Reset modal session state only when opening or switching projectId
  useEffect(() => {
    const wasOpen = prevIsOpenRef.current;
    const prevProjectId = prevProjectIdRef.current;
    prevIsOpenRef.current = isOpen;
    prevProjectIdRef.current = project.projectId;

    if (!isOpen) {
      if (wasOpen) {
        exportRunIdRef.current += 1;
        setIsExporting(false);
      }
      return;
    }

    const justOpened = !wasOpen;
    const projectChanged = prevProjectId !== project.projectId;
    if (justOpened || projectChanged) {
      if (projectChanged) {
        exportRunIdRef.current += 1;
      }
      setIsExporting(false);
      setExportError(null);
      setExportProgress(null);
      setCapcutResult(null);
      setPremiereResult(null);
      setStandaloneResultPath(null);
      setIsCopied(false);
      if (defaultDraftRoot !== undefined) {
        setCustomDraftRoot(defaultDraftRoot);
      }
      const safeName = sanitizeFilenameSegment(project.projectName, project.projectId);
      setCustomOutputPath(resolveDefaultOutputPath(safeName, format));
      prevFormatRef.current = format;
    }
  }, [isOpen, defaultDraftRoot, project.projectId, project.projectName, format]);

  // Update default output path suffix when format changes while modal is open
  useEffect(() => {
    if (!isOpen) {
      prevFormatRef.current = format;
      return;
    }
    if (prevFormatRef.current !== format) {
      prevFormatRef.current = format;
      const safeName = sanitizeFilenameSegment(project.projectName, project.projectId);
      setCustomOutputPath(resolveDefaultOutputPath(safeName, format));
    }
  }, [isOpen, format, project.projectName, project.projectId]);

  // Subscribe to real-time export:progress events while modal is open
  useEffect(() => {
    if (!isOpen) return;

    let isMounted = true;
    let unlistenFn: (() => void) | undefined;

    void Promise.resolve(
      listenExportProgress((payload) => {
        if (!isMounted) return;
        const trimmedProjectId = project.projectId.trim();
        const matchesProject =
          trimmedProjectId.length > 0
            ? payload.projectId === project.projectId
            : payload.projectId.length === 0;
        if (matchesProject) {
          setExportProgress(payload);
        }
      }),
    )
      .then((unlisten) => {
        if (typeof unlisten !== 'function') return;
        if (!isMounted) {
          unlisten();
        } else {
          unlistenFn = unlisten;
        }
      })
      .catch(() => {
        // Non-Tauri / degraded listener fallback
      });

    return () => {
      isMounted = false;
      unlistenFn?.();
    };
  }, [isOpen, project.projectId]);

  // Handle escape key
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent): void => {
      if (e.key === 'Escape' && isOpen && !isExporting) {
        onClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, isExporting, onClose]);

  const handleCopyPath = async (textToCopy: string): Promise<void> => {
    try {
      if (navigator.clipboard && typeof navigator.clipboard.writeText === 'function') {
        await navigator.clipboard.writeText(textToCopy);
        if (!mountedRef.current || !isOpenRef.current) return;
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
      // Fallback
    }
  };

  const handleExport = useCallback(async (): Promise<void> => {
    const runId = exportRunIdRef.current + 1;
    exportRunIdRef.current = runId;
    const isStaleRun = (): boolean =>
      !mountedRef.current || !isOpenRef.current || exportRunIdRef.current !== runId;

    setIsExporting(true);
    setExportError(null);
    setExportProgress({
      projectId: project.projectId,
      percent: 0,
      currentStep: 'Đang chuẩn bị xuất bản...',
    });

    const effectiveVideoPath =
      propVideoFilePath?.trim() || project.videoFilePath?.trim() || undefined;
    const fallbackName = sanitizeFilenameSegment(project.projectName, project.projectId);

    try {
      if (format === 'capcut') {
        const result = await exportCapCutDraft(project, {
          videoFilePath: effectiveVideoPath,
          customDraftRoot: customDraftRoot.trim() ? customDraftRoot.trim() : undefined,
        });
        if (isStaleRun()) return;
        setCapcutResult(result);
        onExportSuccess?.(result);
      } else if (format === 'premiere_xml') {
        const targetPath =
          customOutputPath.trim() || resolveDefaultOutputPath(fallbackName, 'premiere_xml');
        const result = await exportPremiereXml(
          project,
          targetPath,
          effectiveVideoPath ? { videoFilePath: effectiveVideoPath } : undefined,
        );
        if (isStaleRun()) return;
        setPremiereResult(result);
        onExportSuccess?.(result.xmlPath);
      } else if (format === 'ass') {
        const targetPath = customOutputPath.trim() || resolveDefaultOutputPath(fallbackName, 'ass');
        const resultPath = await exportSubtitlesProjectAss(project, targetPath);
        if (isStaleRun()) return;
        setStandaloneResultPath(resultPath);
        onExportSuccess?.(resultPath);
      } else if (format === 'srt') {
        const targetPath = customOutputPath.trim() || resolveDefaultOutputPath(fallbackName, 'srt');
        const resultPath = await exportSubtitlesProjectSrt(project, targetPath);
        if (isStaleRun()) return;
        setStandaloneResultPath(resultPath);
        onExportSuccess?.(resultPath);
      }
    } catch (err: unknown) {
      if (isStaleRun()) return;
      setExportError(parseIpcError(err));
    } finally {
      if (!isStaleRun()) {
        setIsExporting(false);
      }
    }
  }, [format, project, propVideoFilePath, customDraftRoot, customOutputPath, onExportSuccess]);

  if (!isOpen) return null;

  const hasResult = Boolean(capcutResult || premiereResult || standaloneResultPath);
  const displayedResultPath =
    capcutResult?.projectPath || premiereResult?.xmlPath || standaloneResultPath || '';
  const rawPercent = exportProgress?.percent;
  const progressPercent =
    rawPercent !== undefined && Number.isFinite(rawPercent)
      ? Math.min(100, Math.max(0, Math.round(rawPercent)))
      : 0;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="export-modal-title"
      data-testid="export-modal"
      className="animate-in fade-in fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 backdrop-blur-sm"
      onClick={(e) => {
        if (e.target === e.currentTarget && !isExporting) {
          onClose();
        }
      }}
    >
      <div className="flex max-h-[90vh] w-full max-w-xl flex-col gap-5 overflow-y-auto rounded-xl border border-border-dark bg-bg-secondary p-6 text-white shadow-acrylic">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-border-dark pb-4">
          <div className="flex items-center space-x-3">
            <div className="rounded-lg border border-accent-neuro/40 bg-accent-neuro/20 p-2 text-accent-neuro">
              <Share2 className="h-5 w-5" />
            </div>
            <div>
              <h2
                id="export-modal-title"
                data-testid="export-modal-title"
                className="text-base font-semibold tracking-wide text-white"
              >
                Xuất bản Dự án Phụ đề
              </h2>
              <p className="text-xs text-slate-400">
                {project.projectName || project.projectId} &bull; {project.segments.length} câu
                thoại
              </p>
            </div>
          </div>
          <button
            type="button"
            data-testid="export-close-button"
            aria-label="Đóng hộp thoại"
            disabled={isExporting}
            onClick={onClose}
            className="rounded p-1 text-slate-400 transition-colors hover:bg-bg-tertiary hover:text-white disabled:opacity-50"
          >
            <X className="h-5 w-5" />
          </button>
        </div>

        {/* Error Banner */}
        {exportError && (
          <div data-testid="export-error-banner">
            <ErrorBanner
              error={exportError}
              onRetry={exportError.recoverable ? handleExport : undefined}
              onDismiss={() => setExportError(null)}
            />
          </div>
        )}

        {/* Real-time Export Progress Bar (export:progress) */}
        {isExporting && (
          <div
            data-testid="export-progress-container"
            className="flex flex-col gap-2 rounded-lg border border-accent-neuro/30 bg-bg-tertiary/40 p-3.5"
          >
            <div className="flex items-center justify-between text-xs">
              <span data-testid="export-progress-step" className="font-medium text-slate-200">
                {exportProgress?.currentStep || 'Đang xử lý xuất bản...'}
              </span>
              <span
                data-testid="export-progress-percent"
                className="font-mono font-semibold text-accent-neuro"
              >
                {progressPercent}%
              </span>
            </div>
            <div
              role="progressbar"
              aria-label="Tiến độ xuất bản"
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={progressPercent}
              data-testid="export-progress-bar"
              className="h-2 w-full overflow-hidden rounded-full bg-bg-primary"
            >
              <div
                className="h-full rounded-full bg-accent-neuro transition-all duration-200"
                style={{ width: `${progressPercent}%` }}
              />
            </div>
          </div>
        )}

        {/* Success Banner */}
        {hasResult && (
          <div
            data-testid="export-success-banner"
            className="flex flex-col gap-3 rounded-lg border border-accent-vedal/40 bg-accent-vedal/10 p-4 text-slate-100"
          >
            <div className="flex items-start space-x-3">
              <CheckCircle2 className="mt-0.5 h-5 w-5 shrink-0 text-accent-vedal" />
              <div className="min-w-0 flex-1">
                <h3 className="text-sm font-semibold text-accent-vedal">Xuất bản thành công!</h3>
                <p className="mt-0.5 text-xs text-slate-300">
                  {capcutResult
                    ? 'Dự án đã sẵn sàng trong CapCut PC. Mở CapCut lên để thấy ngay dự án hiển thị trên trang chủ.'
                    : premiereResult
                      ? 'Timeline FCP7 XML (<xmeml version="4">) đã sẵn sàng để import trực tiếp vào Adobe Premiere Pro hoặc DaVinci Resolve.'
                      : 'Tệp phụ đề đã được ghi thành công với encoding UTF-8 có BOM.'}
                </p>
              </div>
            </div>

            {/* Path Display with Copy */}
            <div className="flex items-center justify-between rounded border border-border-dark bg-bg-primary/80 px-3 py-2 font-mono text-xs">
              <span className="truncate text-slate-200" title={displayedResultPath}>
                {displayedResultPath}
              </span>
              <button
                type="button"
                data-testid="copy-export-path-button"
                onClick={() => handleCopyPath(displayedResultPath)}
                className="ml-2 flex shrink-0 items-center space-x-1 text-accent-neuro hover:text-white"
              >
                {isCopied ? (
                  <Check className="h-3.5 w-3.5 text-accent-vedal" />
                ) : (
                  <Copy className="h-3.5 w-3.5" />
                )}
                <span className="text-[11px]">{isCopied ? 'Đã chép' : 'Sao chép'}</span>
              </button>
            </div>

            {/* Premiere XML Metadata Summary */}
            {premiereResult && (
              <div
                data-testid="premiere-export-metadata"
                className="mt-1 flex flex-wrap gap-2 border-t border-accent-vedal/20 pt-2 font-mono text-[11px]"
              >
                <span className="rounded border border-border-dark bg-bg-tertiary px-2 py-0.5 text-slate-200">
                  {premiereResult.width}&times;{premiereResult.height}
                </span>
                <span className="rounded border border-border-dark bg-bg-tertiary px-2 py-0.5 text-slate-200">
                  {premiereResult.fps}fps
                </span>
                <span className="rounded border border-accent-neuro/40 bg-accent-neuro/20 px-2 py-0.5 font-semibold text-accent-neuro">
                  {premiereResult.markerCount} markers
                </span>
                <span className="rounded border border-border-dark bg-bg-tertiary px-2 py-0.5 text-slate-300">
                  {premiereResult.clipCount} clips
                </span>
              </div>
            )}

            {/* CapCut Dual Safeguard Confirmation */}
            {capcutResult && (
              <div className="mt-1 flex flex-col gap-1.5 border-t border-accent-vedal/20 pt-2">
                <div className="flex items-center space-x-1.5 text-xs font-medium text-accent-neuro">
                  <ShieldCheck className="h-3.5 w-3.5" />
                  <span>Xác nhận tệp đã tạo & Chiến lược Phòng thủ Kép (ADR-005):</span>
                </div>
                <div className="flex flex-wrap gap-2 font-mono text-[11px]">
                  <span className="rounded border border-border-dark bg-bg-tertiary px-2 py-0.5 text-slate-300">
                    &bull; draft_content.json
                  </span>
                  <span className="rounded border border-border-dark bg-bg-tertiary px-2 py-0.5 text-slate-300">
                    &bull; draft_meta_info.json
                  </span>
                  <span
                    data-testid="safeguard-file-ass"
                    className="rounded border border-accent-neuro/40 bg-accent-neuro/20 px-2 py-0.5 font-semibold text-accent-neuro"
                  >
                    ✓ subtitle.ass (Dự phòng Aegisub)
                  </span>
                  <span
                    data-testid="safeguard-file-srt"
                    className="rounded border border-accent-vedal/40 bg-accent-vedal/20 px-2 py-0.5 font-semibold text-accent-vedal"
                  >
                    ✓ subtitle.srt (Dự phòng SubRip)
                  </span>
                </div>
              </div>
            )}
          </div>
        )}

        {/* Format Selector */}
        {!hasResult && (
          <div className="flex flex-col gap-4">
            <label className="text-xs font-medium text-slate-300">Chọn định dạng xuất bản:</label>

            <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
              {/* Option 1: CapCut PC Draft */}
              <button
                type="button"
                data-testid="export-format-capcut"
                disabled={isExporting}
                onClick={() => setFormat('capcut')}
                className={`flex flex-col rounded-lg border p-3.5 text-left transition-all ${
                  format === 'capcut'
                    ? 'border-accent-neuro bg-accent-neuro/10 shadow-sm'
                    : 'border-border-dark bg-bg-tertiary/40 text-slate-400 hover:bg-bg-tertiary/70'
                }`}
              >
                <div className="mb-1 flex w-full items-center justify-between">
                  <span className="flex items-center space-x-1.5 text-xs font-semibold text-white">
                    <Layers className="h-4 w-4 text-accent-neuro" />
                    <span>CapCut PC Draft</span>
                  </span>
                  <span
                    data-testid="dual-safeguard-badge"
                    className="py-0.2 rounded border border-accent-neuro/40 bg-accent-neuro/20 px-1.5 font-mono text-[10px] font-bold uppercase text-accent-neuro"
                  >
                    Phòng thủ kép
                  </span>
                </div>
                <p className="line-clamp-2 text-[11px] text-slate-400">
                  Tạo dự án CapCut PC hoàn chỉnh kèm tự động sao lưu 2 file độc lập .ass và .srt
                </p>
              </button>

              {/* Option 2: Premiere Pro FCP7 XML */}
              <button
                type="button"
                data-testid="export-format-premiere"
                disabled={isExporting}
                onClick={() => setFormat('premiere_xml')}
                className={`flex flex-col rounded-lg border p-3.5 text-left transition-all ${
                  format === 'premiere_xml'
                    ? 'border-accent-neuro bg-accent-neuro/10 shadow-sm'
                    : 'border-border-dark bg-bg-tertiary/40 text-slate-400 hover:bg-bg-tertiary/70'
                }`}
              >
                <div className="mb-1 flex w-full items-center justify-between">
                  <span className="flex items-center space-x-1.5 text-xs font-semibold text-white">
                    <Clapperboard className="h-4 w-4 text-accent-neuro" />
                    <span>Premiere Pro XML (.xml)</span>
                  </span>
                  <span className="py-0.2 rounded border border-accent-vedal/40 bg-accent-vedal/20 px-1.5 font-mono text-[10px] font-bold uppercase text-accent-vedal">
                    FCP7 XML
                  </span>
                </div>
                <p className="line-clamp-2 text-[11px] text-slate-400">
                  Timeline &lt;xmeml version=&quot;4&quot;&gt; 60fps kèm marker phụ đề cho Premiere
                  Pro &amp; DaVinci Resolve
                </p>
              </button>

              {/* Option 3: Standalone ASS */}
              <button
                type="button"
                data-testid="export-format-ass"
                disabled={isExporting}
                onClick={() => setFormat('ass')}
                className={`flex flex-col rounded-lg border p-3.5 text-left transition-all ${
                  format === 'ass'
                    ? 'border-accent-neuro bg-accent-neuro/10 shadow-sm'
                    : 'border-border-dark bg-bg-tertiary/40 text-slate-400 hover:bg-bg-tertiary/70'
                }`}
              >
                <div className="mb-1 flex items-center space-x-1.5">
                  <FileText className="h-4 w-4 text-accent-vedal" />
                  <span className="text-xs font-semibold text-white">Aegisub (.ass)</span>
                </div>
                <p className="line-clamp-2 text-[11px] text-slate-400">
                  Giữ 100% màu sắc nhân vật Vedal, Neuro, Evil, kích thước và font chữ (UTF-8 BOM)
                </p>
              </button>

              {/* Option 4: Standalone SRT */}
              <button
                type="button"
                data-testid="export-format-srt"
                disabled={isExporting}
                onClick={() => setFormat('srt')}
                className={`flex flex-col rounded-lg border p-3.5 text-left transition-all ${
                  format === 'srt'
                    ? 'border-accent-neuro bg-accent-neuro/10 shadow-sm'
                    : 'border-border-dark bg-bg-tertiary/40 text-slate-400 hover:bg-bg-tertiary/70'
                }`}
              >
                <div className="mb-1 flex items-center space-x-1.5">
                  <Film className="h-4 w-4 text-accent-chat" />
                  <span className="text-xs font-semibold text-white">SubRip (.srt)</span>
                </div>
                <p className="line-clamp-2 text-[11px] text-slate-400">
                  Định dạng phụ đề quốc dân tương thích mọi trình dựng video và YouTube
                </p>
              </button>
            </div>

            {/* CapCut Options & Dual Export Notice */}
            {format === 'capcut' && (
              <div className="flex flex-col gap-3 rounded-lg border border-border-dark bg-bg-tertiary/30 p-3.5">
                {/* Dual Safeguard Callout */}
                <div className="flex items-start space-x-2.5 text-xs text-slate-300">
                  <ShieldCheck className="mt-0.5 h-4 w-4 shrink-0 text-accent-neuro" />
                  <div>
                    <span className="font-semibold text-accent-neuro">
                      Chiến Lược Phòng Thủ Kép (ADR-005):{' '}
                    </span>
                    <span>
                      Khi xuất dự án CapCut, hệ thống luôn tự động copy kèm file{' '}
                      <code>subtitle.ass</code> và <code>subtitle.srt</code> vào thư mục dự án. Nếu
                      phiên bản CapCut mới có thay đổi cấu trúc JSON nội bộ, bạn vẫn luôn có phụ đề
                      để import thủ công.
                    </span>
                  </div>
                </div>

                {/* Custom Draft Root Override (Optional) */}
                <div className="flex flex-col gap-1 border-t border-border-dark/60 pt-2">
                  <label
                    htmlFor="custom-draft-path"
                    className="text-[11px] font-medium text-slate-400"
                  >
                    Thư mục CapCut Projects (Tùy chọn ghi đè):
                  </label>
                  <input
                    id="custom-draft-path"
                    data-testid="custom-draft-path-input"
                    type="text"
                    disabled={isExporting}
                    value={customDraftRoot}
                    onChange={(e) => setCustomDraftRoot(e.target.value)}
                    placeholder="%LOCALAPPDATA%/CapCut/User Data/Projects/com.lveditor.draft/ (Tự động phát hiện)"
                    className="rounded border border-border-dark bg-bg-primary px-3 py-1.5 font-mono text-xs text-white placeholder-slate-500 focus:border-accent-neuro focus:outline-none"
                  />
                  <span className="text-[10px] text-slate-500">
                    Để trống để ứng dụng tự động dò tìm vị trí mặc định của CapCut PC trên Windows.
                  </span>
                </div>
              </div>
            )}

            {/* Standalone / Premiere Path Option */}
            {format !== 'capcut' && (
              <div className="flex flex-col gap-1.5 rounded-lg border border-border-dark bg-bg-tertiary/30 p-3.5">
                <label
                  htmlFor="custom-output-path"
                  className="flex items-center space-x-1.5 text-xs font-medium text-slate-300"
                >
                  <Folder className="h-3.5 w-3.5 text-accent-neuro" />
                  <span>Đường dẫn tệp xuất ra:</span>
                </label>
                <input
                  id="custom-output-path"
                  data-testid="custom-output-path-input"
                  type="text"
                  disabled={isExporting}
                  value={customOutputPath}
                  onChange={(e) => setCustomOutputPath(e.target.value)}
                  className="rounded border border-border-dark bg-bg-primary px-3 py-1.5 font-mono text-xs text-white focus:border-accent-neuro focus:outline-none"
                />
                {format === 'premiere_xml' && (
                  <span className="text-[10px] text-slate-400">
                    Xuất timeline FCP7 XML 60fps kèm video/audio tracks và sequence markers tiếng
                    Việt (UTF-8 không BOM).
                  </span>
                )}
              </div>
            )}
          </div>
        )}

        {/* Footer Actions */}
        <div className="flex items-center justify-end space-x-3 border-t border-border-dark pt-4">
          {hasResult ? (
            <button
              type="button"
              data-testid="export-done-button"
              onClick={onClose}
              className="rounded-lg bg-accent-neuro px-4 py-2 text-xs font-semibold text-bg-primary shadow-sm transition-colors hover:bg-accent-neuro/90"
            >
              Hoàn tất & Đóng
            </button>
          ) : (
            <>
              <button
                type="button"
                data-testid="export-cancel-button"
                disabled={isExporting}
                onClick={onClose}
                className="rounded-lg border border-border-dark bg-bg-tertiary/50 px-4 py-2 text-xs font-medium text-slate-300 transition-colors hover:bg-bg-tertiary hover:text-white disabled:opacity-50"
              >
                Hủy
              </button>

              <button
                type="button"
                data-testid="export-submit-button"
                disabled={isExporting}
                onClick={handleExport}
                className="flex items-center space-x-2 rounded-lg bg-accent-neuro px-4 py-2 text-xs font-semibold text-bg-primary shadow-sm transition-colors hover:bg-accent-neuro/90 disabled:opacity-50"
              >
                {isExporting ? (
                  <>
                    <RefreshCw className="h-3.5 w-3.5 animate-spin" />
                    <span data-testid="export-loading-state">Đang xuất bản...</span>
                  </>
                ) : (
                  <>
                    <Share2 className="h-3.5 w-3.5" />
                    <span>Xuất dự án</span>
                  </>
                )}
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
};
