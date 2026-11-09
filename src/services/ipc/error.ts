/**
 * NeuroClip Unified IPC Error Protocol & Type Guards
 * Single Source of Truth for Tauri IPC Error Serialization & Client Parsing.
 * Strict alignment with docs/CONVENTIONS.md §5.2 and docs/TASK_BOARD.md TASK-M4-02.
 */

export interface IpcErrorResponse {
  /** Canonical uppercase error code string (e.g. 'ERR_INGEST_DISK_FULL') */
  code: string;
  /** Human-readable friendly message in Vietnamese */
  message: string;
  /** Technical details, stack trace, or raw underlying error if present */
  details?: string | null | undefined;
  /** Flag indicating whether the error is recoverable via retry or fallback */
  recoverable: boolean;
}

export const ERROR_CODES = {
  // Ingest Errors
  INGEST_DISK_FULL: 'ERR_INGEST_DISK_FULL',
  INGEST_BOT_BLOCKED: 'ERR_INGEST_BOT_BLOCKED',
  INGEST_CHAT_UNAVAILABLE: 'ERR_INGEST_CHAT_UNAVAILABLE',
  INGEST_NETWORK_TIMEOUT: 'ERR_INGEST_NETWORK_TIMEOUT',
  INGEST_INVALID_URL: 'ERR_INGEST_INVALID_URL',
  INGEST_EXTRACTION_FAILED: 'ERR_INGEST_EXTRACTION_FAILED',
  INGEST_LIVE_STREAM_UNSUPPORTED: 'ERR_INGEST_LIVE_STREAM_UNSUPPORTED',

  // STT Errors
  STT_RATE_LIMIT: 'ERR_STT_RATE_LIMIT',
  STT_PAYLOAD_TOO_LARGE: 'ERR_STT_PAYLOAD_TOO_LARGE',
  STT_VRAM_INSUFFICIENT: 'ERR_STT_VRAM_INSUFFICIENT',
  STT_INVALID_API_KEY: 'ERR_STT_INVALID_API_KEY',
  STT_VAD_FAILED: 'ERR_STT_VAD_FAILED',

  // Translation Errors
  TRANSLATE_SAFETY_FILTER: 'ERR_TRANSLATE_SAFETY_FILTER',
  TRANSLATE_INVALID_API_KEY: 'ERR_TRANSLATE_INVALID_API_KEY',
  TRANSLATE_API_FAILED: 'ERR_TRANSLATE_API_FAILED',

  // Export Errors
  EXPORT_CAPCUT_NOT_FOUND: 'ERR_EXPORT_CAPCUT_NOT_FOUND',
  EXPORT_FILE_WRITE_FAILED: 'ERR_EXPORT_FILE_WRITE_FAILED',
  EXPORT_INVALID_LAYOUT: 'ERR_EXPORT_INVALID_LAYOUT',

  // Highlight Scoring Errors
  HIGHLIGHT_CHAT_PARSING_FAILED: 'ERR_HIGHLIGHT_CHAT_PARSING_FAILED',
  HIGHLIGHT_AUDIO_ANALYSIS_FAILED: 'ERR_HIGHLIGHT_AUDIO_ANALYSIS_FAILED',
  HIGHLIGHT_NO_HIGHLIGHTS: 'ERR_HIGHLIGHT_NO_HIGHLIGHTS',
  HIGHLIGHT_WORKER_PANICKED: 'ERR_HIGHLIGHT_WORKER_PANICKED',

  // Sidecar & Subprocess Errors
  SIDECAR_BINARY_NOT_FOUND: 'ERR_SIDECAR_BINARY_NOT_FOUND',
  SIDECAR_EXECUTION_FAILED: 'ERR_SIDECAR_EXECUTION_FAILED',
  SIDECAR_UPDATE_FAILED: 'ERR_SIDECAR_UPDATE_FAILED',
  SIDECAR_PROCESS_KILLED: 'ERR_SIDECAR_PROCESS_KILLED',

  // Hardware Errors
  HARDWARE_PROBE_FAILED: 'ERR_HARDWARE_PROBE_FAILED',
  HARDWARE_UNSUPPORTED_GPU: 'ERR_HARDWARE_UNSUPPORTED_GPU',
  HARDWARE_INSUFFICIENT_MEMORY: 'ERR_HARDWARE_INSUFFICIENT_MEMORY',

  // Storage Errors
  STORAGE_DISK_CHECK_FAILED: 'ERR_STORAGE_DISK_CHECK_FAILED',
  STORAGE_DIRECTORY_CREATION_FAILED: 'ERR_STORAGE_DIRECTORY_CREATION_FAILED',
  STORAGE_CACHE_CLEANUP_FAILED: 'ERR_STORAGE_CACHE_CLEANUP_FAILED',
  STORAGE_FILE_NOT_FOUND: 'ERR_STORAGE_FILE_NOT_FOUND',

  // Configuration Errors
  CONFIG_NOT_FOUND: 'ERR_CONFIG_NOT_FOUND',
  CONFIG_INVALID_FORMAT: 'ERR_CONFIG_INVALID_FORMAT',
  CONFIG_SAVE_FAILED: 'ERR_CONFIG_SAVE_FAILED',
  CONFIG_INVALID_VALUE: 'ERR_CONFIG_INVALID_VALUE',

  // System & Lifecycle
  TASK_CANCELLED: 'ERR_TASK_CANCELLED',
  INTERNAL_ERROR: 'ERR_INTERNAL_ERROR',

  // Direct String Aliases for convenience
  ERR_INGEST_DISK_FULL: 'ERR_INGEST_DISK_FULL',
  ERR_INGEST_BOT_BLOCKED: 'ERR_INGEST_BOT_BLOCKED',
  ERR_INGEST_CHAT_UNAVAILABLE: 'ERR_INGEST_CHAT_UNAVAILABLE',
  ERR_INGEST_NETWORK_TIMEOUT: 'ERR_INGEST_NETWORK_TIMEOUT',
  ERR_INGEST_INVALID_URL: 'ERR_INGEST_INVALID_URL',
  ERR_INGEST_EXTRACTION_FAILED: 'ERR_INGEST_EXTRACTION_FAILED',
  ERR_INGEST_LIVE_STREAM_UNSUPPORTED: 'ERR_INGEST_LIVE_STREAM_UNSUPPORTED',
  ERR_STT_RATE_LIMIT: 'ERR_STT_RATE_LIMIT',
  ERR_STT_PAYLOAD_TOO_LARGE: 'ERR_STT_PAYLOAD_TOO_LARGE',
  ERR_STT_VRAM_INSUFFICIENT: 'ERR_STT_VRAM_INSUFFICIENT',
  ERR_STT_INVALID_API_KEY: 'ERR_STT_INVALID_API_KEY',
  ERR_STT_VAD_FAILED: 'ERR_STT_VAD_FAILED',
  ERR_TRANSLATE_SAFETY_FILTER: 'ERR_TRANSLATE_SAFETY_FILTER',
  ERR_TRANSLATE_INVALID_API_KEY: 'ERR_TRANSLATE_INVALID_API_KEY',
  ERR_TRANSLATE_API_FAILED: 'ERR_TRANSLATE_API_FAILED',
  ERR_EXPORT_CAPCUT_NOT_FOUND: 'ERR_EXPORT_CAPCUT_NOT_FOUND',
  ERR_EXPORT_FILE_WRITE_FAILED: 'ERR_EXPORT_FILE_WRITE_FAILED',
  ERR_EXPORT_INVALID_LAYOUT: 'ERR_EXPORT_INVALID_LAYOUT',
  ERR_HIGHLIGHT_CHAT_PARSING_FAILED: 'ERR_HIGHLIGHT_CHAT_PARSING_FAILED',
  ERR_HIGHLIGHT_AUDIO_ANALYSIS_FAILED: 'ERR_HIGHLIGHT_AUDIO_ANALYSIS_FAILED',
  ERR_HIGHLIGHT_NO_HIGHLIGHTS: 'ERR_HIGHLIGHT_NO_HIGHLIGHTS',
  ERR_HIGHLIGHT_WORKER_PANICKED: 'ERR_HIGHLIGHT_WORKER_PANICKED',
  ERR_SIDECAR_BINARY_NOT_FOUND: 'ERR_SIDECAR_BINARY_NOT_FOUND',
  ERR_SIDECAR_EXECUTION_FAILED: 'ERR_SIDECAR_EXECUTION_FAILED',
  ERR_SIDECAR_UPDATE_FAILED: 'ERR_SIDECAR_UPDATE_FAILED',
  ERR_SIDECAR_PROCESS_KILLED: 'ERR_SIDECAR_PROCESS_KILLED',
  ERR_HARDWARE_PROBE_FAILED: 'ERR_HARDWARE_PROBE_FAILED',
  ERR_HARDWARE_UNSUPPORTED_GPU: 'ERR_HARDWARE_UNSUPPORTED_GPU',
  ERR_HARDWARE_INSUFFICIENT_MEMORY: 'ERR_HARDWARE_INSUFFICIENT_MEMORY',
  ERR_STORAGE_DISK_CHECK_FAILED: 'ERR_STORAGE_DISK_CHECK_FAILED',
  ERR_STORAGE_DIRECTORY_CREATION_FAILED: 'ERR_STORAGE_DIRECTORY_CREATION_FAILED',
  ERR_STORAGE_CACHE_CLEANUP_FAILED: 'ERR_STORAGE_CACHE_CLEANUP_FAILED',
  ERR_STORAGE_FILE_NOT_FOUND: 'ERR_STORAGE_FILE_NOT_FOUND',
  ERR_CONFIG_NOT_FOUND: 'ERR_CONFIG_NOT_FOUND',
  ERR_CONFIG_INVALID_FORMAT: 'ERR_CONFIG_INVALID_FORMAT',
  ERR_CONFIG_SAVE_FAILED: 'ERR_CONFIG_SAVE_FAILED',
  ERR_CONFIG_INVALID_VALUE: 'ERR_CONFIG_INVALID_VALUE',
  ERR_TASK_CANCELLED: 'ERR_TASK_CANCELLED',
  ERR_INTERNAL_ERROR: 'ERR_INTERNAL_ERROR',
} as const;

export type ErrorCode = (typeof ERROR_CODES)[keyof typeof ERROR_CODES];

export const ERROR_MESSAGES_VI: Readonly<Record<string, string>> = Object.freeze({
  [ERROR_CODES.INGEST_DISK_FULL]:
    'Dung lượng ổ đĩa không đủ. Vui lòng giải phóng bộ nhớ hoặc chọn thư mục lưu trữ khác.',
  [ERROR_CODES.INGEST_BOT_BLOCKED]:
    'YouTube/Twitch đã kích hoạt cơ chế chặn bot. Vui lòng nạp Cookie trong Cài đặt hoặc chờ cập nhật yt-dlp.',
  [ERROR_CODES.INGEST_CHAT_UNAVAILABLE]:
    'Twitch Chat Replay không khả dụng. Đang tự động chuyển sang phân tích highlight bằng Năng lượng Âm thanh.',
  [ERROR_CODES.INGEST_NETWORK_TIMEOUT]:
    'Mất kết nối mạng khi tải stream. Đang tự động thử lại hoặc bấm Thử lại để tiếp tục.',
  [ERROR_CODES.INGEST_INVALID_URL]:
    'Đường dẫn URL video không hợp lệ hoặc nền tảng không được hỗ trợ.',
  [ERROR_CODES.INGEST_EXTRACTION_FAILED]:
    'Không thể trích xuất stream từ liên kết này. Vui lòng kiểm tra lại URL.',
  [ERROR_CODES.INGEST_LIVE_STREAM_UNSUPPORTED]:
    'Luồng đang phát trực tiếp không được hỗ trợ trong phiên bản này (chỉ hỗ trợ VOD đã kết thúc).',
  [ERROR_CODES.STT_RATE_LIMIT]:
    'Đã vượt quá giới hạn lượt gọi Groq STT API (Rate Limit 429). Vui lòng thử lại sau giây lát.',
  [ERROR_CODES.STT_PAYLOAD_TOO_LARGE]:
    'Tệp âm thanh vượt quá giới hạn 25MB của Groq API. Hệ thống sẽ tự động cắt nhỏ đoạn âm thanh.',
  [ERROR_CODES.STT_VRAM_INSUFFICIENT]:
    'Bộ nhớ VRAM không đủ để chạy Faster-Whisper cục bộ. Vui lòng chuyển sang Groq API trong Cài đặt.',
  [ERROR_CODES.STT_INVALID_API_KEY]:
    'Groq API Key không hợp lệ hoặc chưa được cấu hình trong Cài đặt.',
  [ERROR_CODES.STT_VAD_FAILED]:
    'Xử lý Silero VAD nhận diện giọng nói thất bại.',
  [ERROR_CODES.TRANSLATE_SAFETY_FILTER]:
    'Bộ lọc an toàn Gemini đã kích hoạt. Đoạn này được giữ nguyên câu gốc tiếng Anh để dịch thủ công.',
  [ERROR_CODES.TRANSLATE_INVALID_API_KEY]:
    'Google Gemini API Key không hợp lệ hoặc chưa được cấu hình.',
  [ERROR_CODES.TRANSLATE_API_FAILED]:
    'Gọi API dịch thuật Gemini thất bại. Vui lòng kiểm tra kết nối mạng và quota tài khoản.',
  [ERROR_CODES.EXPORT_CAPCUT_NOT_FOUND]:
    'Không tìm thấy thư mục CapCut Draft trên máy tính của bạn.',
  [ERROR_CODES.EXPORT_FILE_WRITE_FAILED]:
    'Không thể ghi tệp dự án hoặc tệp phụ đề ra ổ đĩa.',
  [ERROR_CODES.EXPORT_INVALID_LAYOUT]:
    'Layout xuất bản không hợp lệ hoặc chưa được hỗ trợ.',
  [ERROR_CODES.HIGHLIGHT_CHAT_PARSING_FAILED]:
    'Lỗi phân tích cú pháp sự kiện chat trong luồng VOD.',
  [ERROR_CODES.HIGHLIGHT_AUDIO_ANALYSIS_FAILED]:
    'Lỗi phân tích năng lượng âm thanh.',
  [ERROR_CODES.HIGHLIGHT_NO_HIGHLIGHTS]:
    'Không tìm thấy khoảnh khắc highlight nào thỏa mãn tiêu chí.',
  [ERROR_CODES.HIGHLIGHT_WORKER_PANICKED]:
    'Worker tính toán highlight gặp sự cố bất ngờ.',
  [ERROR_CODES.SIDECAR_BINARY_NOT_FOUND]:
    'Không tìm thấy công cụ phụ trợ (ffmpeg hoặc yt-dlp).',
  [ERROR_CODES.SIDECAR_EXECUTION_FAILED]:
    'Tiến trình phụ trợ gặp lỗi khi đang thực thi tác vụ.',
  [ERROR_CODES.SIDECAR_UPDATE_FAILED]:
    'Cập nhật công cụ phụ trợ thất bại.',
  [ERROR_CODES.SIDECAR_PROCESS_KILLED]:
    'Tiến trình con bị hủy do ứng dụng tắt hoặc người dùng dừng.',
  [ERROR_CODES.HARDWARE_PROBE_FAILED]:
    'Không thể đánh giá thông số phần cứng của hệ thống.',
  [ERROR_CODES.HARDWARE_UNSUPPORTED_GPU]:
    'Không tìm thấy GPU tương thích hoặc không hỗ trợ CUDA.',
  [ERROR_CODES.HARDWARE_INSUFFICIENT_MEMORY]:
    'Dung lượng RAM hệ thống không đủ đáp ứng yêu cầu tối thiểu.',
  [ERROR_CODES.STORAGE_DISK_CHECK_FAILED]:
    'Không thể kiểm tra dung lượng ổ đĩa.',
  [ERROR_CODES.STORAGE_DIRECTORY_CREATION_FAILED]:
    'Không thể tạo thư mục lưu trữ trên ổ đĩa.',
  [ERROR_CODES.STORAGE_CACHE_CLEANUP_FAILED]:
    'Dọn dẹp bộ nhớ đệm thất bại.',
  [ERROR_CODES.STORAGE_FILE_NOT_FOUND]:
    'Không tìm thấy tệp tin được yêu cầu.',
  [ERROR_CODES.CONFIG_NOT_FOUND]:
    'Không tìm thấy tệp cấu hình ứng dụng.',
  [ERROR_CODES.CONFIG_INVALID_FORMAT]:
    'Không thể đọc hoặc giải mã cấu hình.',
  [ERROR_CODES.CONFIG_SAVE_FAILED]:
    'Không thể lưu tệp cấu hình ra đĩa.',
  [ERROR_CODES.CONFIG_INVALID_VALUE]:
    'Giá trị cấu hình không hợp lệ.',
  [ERROR_CODES.TASK_CANCELLED]:
    'Tác vụ đã được hủy bởi người dùng.',
  [ERROR_CODES.INTERNAL_ERROR]:
    'Đã xảy ra lỗi hệ thống nội bộ không xác định.',
});

/**
 * Type guard verifying if an unknown value satisfies the IpcErrorResponse interface.
 * Zero `any` guarantee.
 */
export function isIpcError(err: unknown): err is IpcErrorResponse {
  if (typeof err !== 'object' || err === null) {
    return false;
  }

  const record = err as Record<string, unknown>;
  const hasCode = typeof record['code'] === 'string';
  const hasMessage = typeof record['message'] === 'string';
  const hasRecoverable = typeof record['recoverable'] === 'boolean';
  const details = record['details'];
  const hasValidDetails =
    details === undefined || details === null || typeof details === 'string';

  return hasCode && hasMessage && hasRecoverable && hasValidDetails;
}

/**
 * Parses an unknown error (from Tauri IPC rejection, string, or standard Error)
 * into a standardized IpcErrorResponse envelope.
 */
export function parseIpcError(
  err: unknown,
  fallbackMessage: string = 'Đã xảy ra lỗi hệ thống không xác định',
): IpcErrorResponse {
  if (isIpcError(err)) {
    return err;
  }

  // Handle JSON stringified error from Tauri invoke rejections
  if (typeof err === 'string') {
    const trimmed = err.trim();
    if (trimmed.startsWith('{') && trimmed.endsWith('}')) {
      try {
        const parsed: unknown = JSON.parse(trimmed);
        if (isIpcError(parsed)) {
          return parsed;
        }
      } catch {
        // Fall back to plain string message
      }
    }

    return {
      code: ERROR_CODES.INTERNAL_ERROR,
      message: trimmed.length > 0 ? trimmed : fallbackMessage,
      details: null,
      recoverable: false,
    };
  }

  if (err instanceof Error) {
    const trimmed = err.message.trim();
    if (trimmed.startsWith('{') && trimmed.endsWith('}')) {
      try {
        const parsed: unknown = JSON.parse(trimmed);
        if (isIpcError(parsed)) {
          return parsed;
        }
      } catch {
        // Fall back to Error instance handling
      }
    }

    return {
      code: ERROR_CODES.INTERNAL_ERROR,
      message: err.message.length > 0 ? err.message : fallbackMessage,
      details: err.stack ?? null,
      recoverable: false,
    };
  }

  if (typeof err === 'object' && err !== null) {
    const record = err as Record<string, unknown>;
    const rawMessage = record['message'];
    const messageStr = typeof rawMessage === 'string' ? rawMessage : fallbackMessage;
    const rawCode = record['code'];
    const codeStr = typeof rawCode === 'string' ? rawCode : ERROR_CODES.INTERNAL_ERROR;
    const rawRecoverable = record['recoverable'];
    const recoverable = typeof rawRecoverable === 'boolean' ? rawRecoverable : false;
    const rawDetails = record['details'];
    const details = typeof rawDetails === 'string' ? rawDetails : null;

    return {
      code: codeStr,
      message: messageStr,
      details,
      recoverable,
    };
  }

  return {
    code: ERROR_CODES.INTERNAL_ERROR,
    message: fallbackMessage,
    details: err !== undefined && err !== null ? String(err) : null,
    recoverable: false,
  };
}

/**
 * Returns a user-friendly Vietnamese explanation for a given error code or IpcErrorResponse.
 */
export function getFriendlyErrorMessage(errorOrCode: unknown): string {
  if (typeof errorOrCode === 'string') {
    const directLookup = ERROR_MESSAGES_VI[errorOrCode];
    if (directLookup !== undefined) {
      return directLookup;
    }
  }

  if (isIpcError(errorOrCode)) {
    const codeLookup = ERROR_MESSAGES_VI[errorOrCode.code];
    if (codeLookup !== undefined) {
      return codeLookup;
    }
    return errorOrCode.message;
  }

  return 'Đã xảy ra lỗi không xác định.';
}

/**
 * Alias for getFriendlyErrorMessage for API compatibility.
 */
export function getVietnameseMessage(errorOrCode: unknown): string {
  return getFriendlyErrorMessage(errorOrCode);
}

/**
 * Checks whether an unknown error is marked as recoverable.
 */
export function isRecoverableError(err: unknown): boolean {
  if (isIpcError(err)) {
    return err.recoverable;
  }
  return false;
}
