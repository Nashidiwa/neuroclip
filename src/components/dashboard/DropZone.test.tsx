import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act, render, screen, fireEvent, waitFor } from '@testing-library/react';
import { DropZone, isSupportedVideoPath } from './DropZone';

const mockInvoke = vi.fn();
const mockConvertFileSrc = vi.fn(
  (path: string) => `http://asset.localhost/${encodeURIComponent(path)}`,
);
const mockOnDragDropEvent = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
  convertFileSrc: (path: string) => mockConvertFileSrc(path),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock('@tauri-apps/api/webviewWindow', () => ({
  getCurrentWebviewWindow: () => ({
    onDragDropEvent: (...args: unknown[]) => mockOnDragDropEvent(...args),
  }),
}));

function buildMockManifest(filePath: string) {
  return {
    vod_id: 'local_neuro_vod_1234',
    source_type: 'local_file' as const,
    original_url: null,
    title: 'neuro_vod',
    streamer: null,
    duration_seconds: 185.5,
    audio_path: 'C:\\NeuroClip\\cache\\local_neuro_vod_1234_audio_cache.wav',
    chat_file_path: null,
    video_stream_url: null,
    local_video_path: filePath,
    created_at: '2026-11-17T10:00:00Z',
  };
}

describe('DropZone (TASK-M4-07)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockOnDragDropEvent.mockResolvedValue(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('renders dropzone UI, supported extensions (.mp4, .mkv, .webm, .mov, .ts, .flv), and file input accept mask', () => {
    render(<DropZone />);

    expect(screen.getByTestId('local-dropzone-container')).toBeInTheDocument();
    expect(screen.getByTestId('dropzone-area')).toBeInTheDocument();

    for (const ext of ['mp4', 'mkv', 'webm', 'mov', 'ts', 'flv']) {
      expect(screen.getByTestId(`supported-ext-${ext}`)).toBeInTheDocument();
    }

    const fileInput = screen.getByTestId('dropzone-file-input') as HTMLInputElement;
    expect(fileInput.accept).toBe('.mp4,.mkv,.webm,.mov,.ts,.flv');
  });

  it('validates supported extensions (.mp4, .mkv, .webm, .mov, .ts, .flv) case-insensitively and rejects unsupported formats', () => {
    expect(isSupportedVideoPath('D:\\Videos\\clip.mp4')).toBe(true);
    expect(isSupportedVideoPath('D:\\Videos\\clip.MKV')).toBe(true);
    expect(isSupportedVideoPath('/home/user/stream.webm')).toBe(true);
    expect(isSupportedVideoPath('record.MoV')).toBe(true);
    expect(isSupportedVideoPath('D:\\OBS\\recording.ts')).toBe(true);
    expect(isSupportedVideoPath('D:\\OBS\\recording.FLV')).toBe(true);

    expect(isSupportedVideoPath('notes.txt')).toBe(false);
    expect(isSupportedVideoPath('payload.exe')).toBe(false);
    expect(isSupportedVideoPath('archive.zip')).toBe(false);
    expect(isSupportedVideoPath('')).toBe(false);
  });

  it('rejects .txt, .exe, .zip files with ErrorBanner without invoking import_local_video', async () => {
    render(<DropZone />);

    const dropArea = screen.getByTestId('dropzone-area');

    for (const badName of ['readme.txt', 'malware.exe', 'backup.zip']) {
      const badFile = new File(['invalid'], badName, { type: 'application/octet-stream' });
      fireEvent.drop(dropArea, {
        dataTransfer: { files: [badFile] },
      });

      await waitFor(() => {
        expect(screen.getByTestId('error-banner')).toBeInTheDocument();
      });
      expect(screen.getByTestId('error-code-badge')).toHaveTextContent('ERR_INGEST_INVALID_URL');
      expect(mockInvoke).not.toHaveBeenCalled();
    }
  });

  it('imports valid .mp4/.mkv/.webm/.mov via file drop and invokes onImportSuccess', async () => {
    const onImportSuccess = vi.fn();
    const targetPath = 'D:\\Videos\\neuro_singing.mkv';
    mockInvoke.mockResolvedValueOnce(buildMockManifest(targetPath));

    render(<DropZone onImportSuccess={onImportSuccess} customCacheDir={'D:\\Cache'} />);

    const fileInput = screen.getByTestId('dropzone-file-input');
    const validFile = new File(['video-bytes'], 'neuro_singing.mkv', {
      type: 'video/x-matroska',
    });
    Object.defineProperty(validFile, 'path', { value: targetPath });

    fireEvent.change(fileInput, { target: { files: [validFile] } });

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('import_local_video', {
        filePath: targetPath,
        customCacheDir: 'D:\\Cache',
        taskId: null,
      });
    });

    await waitFor(() => {
      expect(screen.getByTestId('dropzone-manifest-card')).toBeInTheDocument();
    });
    expect(screen.getByTestId('dropzone-asset-url')).toHaveTextContent('http://asset.localhost/');
    expect(onImportSuccess).toHaveBeenCalledTimes(1);
  });

  it('handles Tauri native onDragDropEvent enter/leave/drop payloads', async () => {
    let capturedHandler: ((event: { payload: unknown }) => void) | undefined;
    mockOnDragDropEvent.mockImplementationOnce(
      async (cb: (event: { payload: unknown }) => void) => {
        capturedHandler = cb;
        return () => {};
      },
    );

    const droppedPath = 'C:\\Users\\Taine\\Videos\\stream.webm';
    mockInvoke.mockResolvedValueOnce(buildMockManifest(droppedPath));

    render(<DropZone />);

    await waitFor(() => {
      expect(capturedHandler).toBeDefined();
    });

    const dropArea = screen.getByTestId('dropzone-area');
    fireEvent.dragEnter(dropArea);
    expect(dropArea).toHaveAttribute('data-dragging', 'true');
    fireEvent.dragLeave(dropArea);
    expect(dropArea).toHaveAttribute('data-dragging', 'false');

    act(() => {
      capturedHandler?.({
        payload: { type: 'enter', paths: [droppedPath], position: { x: 10, y: 10 } },
      });
    });
    await waitFor(() => {
      expect(dropArea).toHaveAttribute('data-dragging', 'true');
    });

    await act(async () => {
      capturedHandler?.({
        payload: { type: 'drop', paths: [droppedPath], position: { x: 10, y: 10 } },
      });
    });

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('import_local_video', {
        filePath: droppedPath,
        customCacheDir: null,
        taskId: null,
      });
    });
  });

  it('displays recoverable IPC error in ErrorBanner and supports retry/dismiss', async () => {
    const targetPath = 'D:\\Videos\\clip.mov';
    mockInvoke
      .mockRejectedValueOnce({
        code: 'ERR_INGEST_DISK_FULL',
        message: 'Dung lượng ổ đĩa không đủ',
        details: 'Free: 2000MB < 5120MB',
        recoverable: true,
      })
      .mockResolvedValueOnce(buildMockManifest(targetPath));

    render(<DropZone />);

    const pathInput = screen.getByTestId('dropzone-path-input');
    fireEvent.change(pathInput, { target: { value: targetPath } });
    fireEvent.click(screen.getByTestId('dropzone-path-submit'));

    await waitFor(() => {
      expect(screen.getByTestId('error-banner')).toBeInTheDocument();
    });
    expect(screen.getByTestId('error-code-badge')).toHaveTextContent('ERR_INGEST_DISK_FULL');

    fireEvent.click(screen.getByTestId('error-retry-btn'));

    await waitFor(() => {
      expect(screen.getByTestId('dropzone-manifest-card')).toBeInTheDocument();
    });
    expect(screen.queryByTestId('error-banner')).not.toBeInTheDocument();
  });

  it('blocks standard DOM File without native .path from invoking import_local_video with bare file.name and focuses path input', async () => {
    render(<DropZone />);

    const fileInput = screen.getByTestId('dropzone-file-input');
    const bareFile = new File(['video-bytes'], 'only_filename.mp4', { type: 'video/mp4' });
    fireEvent.change(fileInput, { target: { files: [bareFile] } });

    await waitFor(() => {
      expect(screen.getByTestId('error-banner')).toBeInTheDocument();
    });
    expect(screen.getByTestId('error-code-badge')).toHaveTextContent('ERR_STORAGE_FILE_NOT_FOUND');
    expect(mockInvoke).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(screen.getByTestId('dropzone-path-input'));
  });

  it('supports optional onBrowseRequest native dialog hook for .ts and .flv files', async () => {
    const selectedPath = 'D:\\OBS\\recording.ts';
    const onBrowseRequest = vi.fn().mockResolvedValueOnce(selectedPath);
    mockInvoke.mockResolvedValueOnce(buildMockManifest(selectedPath));

    render(<DropZone onBrowseRequest={onBrowseRequest} />);
    fireEvent.click(screen.getByTestId('dropzone-area'));

    await waitFor(() => {
      expect(onBrowseRequest).toHaveBeenCalledTimes(1);
      expect(mockInvoke).toHaveBeenCalledWith('import_local_video', {
        filePath: selectedPath,
        customCacheDir: null,
        taskId: null,
      });
    });
  });

  it('subscribes to onDragDropEvent only once across import state transitions', async () => {
    const mockUnlisten = vi.fn();
    mockOnDragDropEvent.mockResolvedValue(mockUnlisten);

    let resolveImport!: (val: ReturnType<typeof buildMockManifest>) => void;
    mockInvoke.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveImport = resolve;
        }),
    );

    const targetPath = 'D:\\OBS\\recording.flv';
    render(<DropZone />);

    await waitFor(() => {
      expect(mockOnDragDropEvent).toHaveBeenCalledTimes(1);
    });

    fireEvent.change(screen.getByTestId('dropzone-path-input'), {
      target: { value: targetPath },
    });
    fireEvent.click(screen.getByTestId('dropzone-path-submit'));

    await waitFor(() => {
      expect(screen.getByTestId('dropzone-loading-icon')).toBeInTheDocument();
    });

    await act(async () => {
      resolveImport(buildMockManifest(targetPath));
    });

    await waitFor(() => {
      expect(screen.getByTestId('dropzone-manifest-card')).toBeInTheDocument();
    });

    expect(mockOnDragDropEvent).toHaveBeenCalledTimes(1);
    expect(mockUnlisten).not.toHaveBeenCalled();
  });

  it('triggers hidden file input click exactly once without recursive bubbling when clicking dropzone-area', () => {
    const clickSpy = vi.spyOn(HTMLInputElement.prototype, 'click').mockImplementation(() => {});

    try {
      render(<DropZone />);
      const dropzoneArea = screen.getByTestId('dropzone-area');
      const fileInput = screen.getByTestId('dropzone-file-input');

      fireEvent.click(dropzoneArea);
      expect(clickSpy).toHaveBeenCalledTimes(1);

      // Direct click on nested input must not bubble back up to dropzone-area
      fireEvent.click(fileInput);
      expect(clickSpy).toHaveBeenCalledTimes(1);
    } finally {
      clickSpy.mockRestore();
    }
  });
});
