import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import { ExportModal } from './ExportModal.tsx';
import * as exportIpc from '../../services/ipc/export.ts';
import { ERROR_CODES } from '../../services/ipc/error.ts';
import { type SubtitleProject, normalizeSubtitleProject } from '../../types/transcript.ts';

vi.mock('../../services/ipc/export.ts', async (importOriginal) => {
  const actual = await importOriginal<typeof exportIpc>();
  return {
    ...actual,
    exportCapCutDraft: vi.fn(),
    exportPremiereXml: vi.fn(),
    exportSubtitlesProjectAss: vi.fn(),
    exportSubtitlesProjectSrt: vi.fn(),
    listenExportProgress: vi.fn().mockResolvedValue(() => {}),
  };
});

describe('ExportModal Component', () => {
  const mockOnClose = vi.fn();
  const mockOnExportSuccess = vi.fn();

  const mockProject: SubtitleProject = normalizeSubtitleProject({
    project_id: 'subproj_test_export',
    project_name: 'Vedal & Neuro Clip',
    vod_id: 'vod_test_123',
    video_file_path: 'C:\\Videos\\clip.mp4',
    target_layout: 'original_16_9',
    segments: [
      {
        segment_id: 'seg_1',
        start_sec: 1.0,
        end_sec: 3.5,
        speaker_id: 'neuro',
        text_en: 'Hello chat!',
        text_vi: 'Chào chat!',
        confidence: 0.98,
        cps_score: 12.0,
        is_flagged_for_review: false,
        words: [],
      },
    ],
    speaker_styles: {
      neuro: {
        color_hex: '#FFB6C1',
        font_name: 'Arial',
        font_size: 54,
        outline_color_hex: '#000000',
      },
    },
    updated_at: '2026-11-20T00:00:00Z',
  });

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(exportIpc.listenExportProgress).mockResolvedValue(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('renders modal dialog with accessible elements when isOpen=true', () => {
    render(<ExportModal isOpen={true} onClose={mockOnClose} project={mockProject} />);

    expect(screen.getByTestId('export-modal')).toBeInTheDocument();
    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(screen.getByTestId('export-modal-title')).toHaveTextContent('Xuất bản Dự án Phụ đề');
    expect(screen.getByTestId('export-format-capcut')).toBeInTheDocument();
    expect(screen.getByTestId('export-format-premiere')).toBeInTheDocument();
    expect(screen.getByTestId('export-format-ass')).toBeInTheDocument();
    expect(screen.getByTestId('export-format-srt')).toBeInTheDocument();
    expect(screen.getByTestId('export-submit-button')).toBeInTheDocument();
  });

  it('does not render anything when isOpen=false', () => {
    render(<ExportModal isOpen={false} onClose={mockOnClose} project={mockProject} />);

    expect(screen.queryByTestId('export-modal')).not.toBeInTheDocument();
  });

  it('highlights Dual Export Safeguard badge (ADR-005) when CapCut format is selected', () => {
    render(<ExportModal isOpen={true} onClose={mockOnClose} project={mockProject} />);

    const badge = screen.getByTestId('dual-safeguard-badge');
    expect(badge).toBeInTheDocument();
    expect(badge).toHaveTextContent('Phòng thủ kép');
    expect(screen.getByText(/Chiến Lược Phòng Thủ Kép \(ADR-005\)/i)).toBeInTheDocument();
    expect(screen.getByTestId('custom-draft-path-input')).toBeInTheDocument();
  });

  it('switches between CapCut, ASS, and SRT formats', () => {
    render(<ExportModal isOpen={true} onClose={mockOnClose} project={mockProject} />);

    // Switch to ASS
    fireEvent.click(screen.getByTestId('export-format-ass'));
    expect(screen.queryByTestId('custom-draft-path-input')).not.toBeInTheDocument();
    const assInput = screen.getByTestId('custom-output-path-input') as HTMLInputElement;
    expect(assInput.value).toContain('.ass');

    // Switch to SRT
    fireEvent.click(screen.getByTestId('export-format-srt'));
    const srtInput = screen.getByTestId('custom-output-path-input') as HTMLInputElement;
    expect(srtInput.value).toContain('.srt');

    // Switch back to CapCut
    fireEvent.click(screen.getByTestId('export-format-capcut'));
    expect(screen.getByTestId('custom-draft-path-input')).toBeInTheDocument();
  });

  it('successfully exports CapCut PC Draft and displays Dual Safeguard confirmation badges', async () => {
    const mockExportCapCut = vi.mocked(exportIpc.exportCapCutDraft);
    mockExportCapCut.mockResolvedValueOnce({
      projectDir: 'C:\\CapCut\\Projects\\NeuroClip_Highlight_20261120_100000',
      projectPath: 'C:\\CapCut\\Projects\\NeuroClip_Highlight_20261120_100000',
      draftName: 'NeuroClip_Highlight_20261120_100000',
      draftContentPath: 'C:\\CapCut\\Projects\\draft_content.json',
      draftMetaInfoPath: 'C:\\CapCut\\Projects\\draft_meta_info.json',
      assBackupPath: 'C:\\CapCut\\Projects\\subtitle.ass',
      srtBackupPath: 'C:\\CapCut\\Projects\\subtitle.srt',
      dualSafeguardFiles: ['subtitle.ass', 'subtitle.srt'],
      durationMs: 25,
    });

    render(
      <ExportModal
        isOpen={true}
        onClose={mockOnClose}
        project={mockProject}
        onExportSuccess={mockOnExportSuccess}
      />,
    );

    fireEvent.click(screen.getByTestId('export-submit-button'));

    await waitFor(() => {
      expect(screen.getByTestId('export-success-banner')).toBeInTheDocument();
    });

    expect(mockExportCapCut).toHaveBeenCalledWith(
      mockProject,
      expect.objectContaining({
        videoFilePath: 'C:\\Videos\\clip.mp4',
      }),
    );
    expect(screen.getByTestId('safeguard-file-ass')).toBeInTheDocument();
    expect(screen.getByTestId('safeguard-file-srt')).toBeInTheDocument();
    expect(mockOnExportSuccess).toHaveBeenCalledTimes(1);

    // Done button closes modal
    fireEvent.click(screen.getByTestId('export-done-button'));
    expect(mockOnClose).toHaveBeenCalledTimes(1);
  });

  it('handles custom draft root directory override', async () => {
    const mockExportCapCut = vi.mocked(exportIpc.exportCapCutDraft);
    mockExportCapCut.mockResolvedValueOnce({
      projectDir: 'D:\\CustomDrafts\\NeuroClip_Highlight_1',
      projectPath: 'D:\\CustomDrafts\\NeuroClip_Highlight_1',
      draftName: 'NeuroClip_Highlight_1',
      draftContentPath: 'D:\\CustomDrafts\\draft_content.json',
      draftMetaInfoPath: 'D:\\CustomDrafts\\draft_meta_info.json',
      assBackupPath: 'D:\\CustomDrafts\\subtitle.ass',
      srtBackupPath: 'D:\\CustomDrafts\\subtitle.srt',
      dualSafeguardFiles: ['subtitle.ass', 'subtitle.srt'],
      durationMs: 15,
    });

    render(<ExportModal isOpen={true} onClose={mockOnClose} project={mockProject} />);

    const input = screen.getByTestId('custom-draft-path-input');
    fireEvent.change(input, { target: { value: 'D:\\CustomDrafts' } });
    fireEvent.click(screen.getByTestId('export-submit-button'));

    await waitFor(() => {
      expect(screen.getByTestId('export-success-banner')).toBeInTheDocument();
    });

    expect(mockExportCapCut).toHaveBeenCalledWith(
      mockProject,
      expect.objectContaining({
        customDraftRoot: 'D:\\CustomDrafts',
      }),
    );
  });

  it('handles CapCut directory not found error (ERR_EXPORT_CAPCUT_NOT_FOUND)', async () => {
    const mockExportCapCut = vi.mocked(exportIpc.exportCapCutDraft);
    mockExportCapCut.mockRejectedValueOnce({
      code: ERROR_CODES.ERR_EXPORT_CAPCUT_NOT_FOUND,
      message: 'Không tìm thấy thư mục CapCut Draft trên máy tính của bạn.',
      recoverable: true,
    });

    render(<ExportModal isOpen={true} onClose={mockOnClose} project={mockProject} />);

    fireEvent.click(screen.getByTestId('export-submit-button'));

    await waitFor(() => {
      expect(screen.getByTestId('export-error-banner')).toBeInTheDocument();
    });

    expect(
      screen.getByText('Không tìm thấy thư mục CapCut Draft trên máy tính của bạn.'),
    ).toBeInTheDocument();
  });

  it('handles file write error (ERR_EXPORT_FILE_WRITE_FAILED)', async () => {
    const mockExportCapCut = vi.mocked(exportIpc.exportCapCutDraft);
    mockExportCapCut.mockRejectedValueOnce({
      code: ERROR_CODES.ERR_EXPORT_FILE_WRITE_FAILED,
      message: 'Không thể ghi tệp dự án hoặc tệp phụ đề ra ổ đĩa.',
      details: 'Access is denied (os error 5)',
      recoverable: true,
    });

    render(<ExportModal isOpen={true} onClose={mockOnClose} project={mockProject} />);

    fireEvent.click(screen.getByTestId('export-submit-button'));

    await waitFor(() => {
      expect(screen.getByTestId('export-error-banner')).toBeInTheDocument();
    });

    expect(
      screen.getByText('Không thể ghi tệp dự án hoặc tệp phụ đề ra ổ đĩa.'),
    ).toBeInTheDocument();
  });

  it('handles standalone ASS export', async () => {
    const mockExportAss = vi.mocked(exportIpc.exportSubtitlesProjectAss);
    mockExportAss.mockResolvedValueOnce('C:\\NeuroClip\\export\\test.ass');

    render(
      <ExportModal
        isOpen={true}
        onClose={mockOnClose}
        project={mockProject}
        onExportSuccess={mockOnExportSuccess}
      />,
    );

    fireEvent.click(screen.getByTestId('export-format-ass'));
    fireEvent.click(screen.getByTestId('export-submit-button'));

    await waitFor(() => {
      expect(screen.getByTestId('export-success-banner')).toBeInTheDocument();
    });

    expect(mockExportAss).toHaveBeenCalledWith(mockProject, expect.stringContaining('.ass'));
    expect(mockOnExportSuccess).toHaveBeenCalledWith('C:\\NeuroClip\\export\\test.ass');
  });

  it('handles standalone SRT export', async () => {
    const mockExportSrt = vi.mocked(exportIpc.exportSubtitlesProjectSrt);
    mockExportSrt.mockResolvedValueOnce('C:\\NeuroClip\\export\\test.srt');

    render(
      <ExportModal
        isOpen={true}
        onClose={mockOnClose}
        project={mockProject}
        onExportSuccess={mockOnExportSuccess}
      />,
    );

    fireEvent.click(screen.getByTestId('export-format-srt'));
    fireEvent.click(screen.getByTestId('export-submit-button'));

    await waitFor(() => {
      expect(screen.getByTestId('export-success-banner')).toBeInTheDocument();
    });

    expect(mockExportSrt).toHaveBeenCalledWith(mockProject, expect.stringContaining('.srt'));
    expect(mockOnExportSuccess).toHaveBeenCalledWith('C:\\NeuroClip\\export\\test.srt');
  });

  it('closes modal when close button, cancel button, or backdrop is clicked', () => {
    render(<ExportModal isOpen={true} onClose={mockOnClose} project={mockProject} />);

    // Close button
    fireEvent.click(screen.getByTestId('export-close-button'));
    expect(mockOnClose).toHaveBeenCalledTimes(1);

    // Cancel button
    fireEvent.click(screen.getByTestId('export-cancel-button'));
    expect(mockOnClose).toHaveBeenCalledTimes(2);

    // Backdrop click
    fireEvent.click(screen.getByTestId('export-modal'));
    expect(mockOnClose).toHaveBeenCalledTimes(3);
  });

  it('closes modal when Escape key is pressed', () => {
    render(<ExportModal isOpen={true} onClose={mockOnClose} project={mockProject} />);

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(mockOnClose).toHaveBeenCalledTimes(1);
  });

  it('copies path to clipboard when copy button is clicked', async () => {
    const mockExportCapCut = vi.mocked(exportIpc.exportCapCutDraft);
    mockExportCapCut.mockResolvedValueOnce({
      projectDir: 'C:\\CapCut\\Projects\\NeuroClip_Highlight_1',
      projectPath: 'C:\\CapCut\\Projects\\NeuroClip_Highlight_1',
      draftName: 'NeuroClip_Highlight_1',
      draftContentPath: 'draft_content.json',
      draftMetaInfoPath: 'draft_meta_info.json',
      assBackupPath: 'subtitle.ass',
      srtBackupPath: 'subtitle.srt',
      dualSafeguardFiles: ['subtitle.ass', 'subtitle.srt'],
      durationMs: 10,
    });

    const writeTextMock = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, {
      clipboard: {
        writeText: writeTextMock,
      },
    });

    render(<ExportModal isOpen={true} onClose={mockOnClose} project={mockProject} />);

    fireEvent.click(screen.getByTestId('export-submit-button'));

    await waitFor(() => {
      expect(screen.getByTestId('copy-export-path-button')).toBeInTheDocument();
    });

    await act(async () => {
      fireEvent.click(screen.getByTestId('copy-export-path-button'));
    });
    expect(writeTextMock).toHaveBeenCalledWith('C:\\CapCut\\Projects\\NeuroClip_Highlight_1');
  });

  it('switches to Premiere Pro XML format with .xml default path and exports successfully', async () => {
    const mockExportPremiere = vi.mocked(exportIpc.exportPremiereXml);
    mockExportPremiere.mockResolvedValueOnce({
      xmlPath: 'C:\\NeuroClip\\export\\Vedal & Neuro Clip_timeline.xml',
      sequenceName: 'Vedal & Neuro Clip',
      width: 1920,
      height: 1080,
      fps: 60,
      markerCount: 1,
      clipCount: 1,
      durationMs: 12,
    });

    render(
      <ExportModal
        isOpen={true}
        onClose={mockOnClose}
        project={mockProject}
        onExportSuccess={mockOnExportSuccess}
      />,
    );

    fireEvent.click(screen.getByTestId('export-format-premiere'));
    const pathInput = screen.getByTestId('custom-output-path-input') as HTMLInputElement;
    expect(pathInput.value).toContain('.xml');

    fireEvent.click(screen.getByTestId('export-submit-button'));

    await waitFor(() => {
      expect(screen.getByTestId('export-success-banner')).toBeInTheDocument();
    });

    expect(mockExportPremiere).toHaveBeenCalledWith(
      mockProject,
      expect.stringContaining('.xml'),
      expect.objectContaining({ videoFilePath: 'C:\\Videos\\clip.mp4' }),
    );
    expect(screen.getByTestId('premiere-export-metadata')).toHaveTextContent('1920×1080');
    expect(screen.getByTestId('premiere-export-metadata')).toHaveTextContent('60fps');
    expect(screen.getByTestId('premiere-export-metadata')).toHaveTextContent('1 markers');
    expect(mockOnExportSuccess).toHaveBeenCalledWith(
      'C:\\NeuroClip\\export\\Vedal & Neuro Clip_timeline.xml',
    );
  });

  it('IT-26.2: renders real-time export progress bar from export:progress and cleans up listener on unmount', async () => {
    const unlistenMock = vi.fn();
    let progressCallback: ((payload: exportIpc.ExportProgressPayload) => void) | undefined;

    vi.mocked(exportIpc.listenExportProgress).mockImplementationOnce(async (cb) => {
      progressCallback = cb;
      return unlistenMock;
    });

    let resolveExport!: (value: exportIpc.PremiereExportResult) => void;
    const pendingExport = new Promise<exportIpc.PremiereExportResult>((resolve) => {
      resolveExport = resolve;
    });
    vi.mocked(exportIpc.exportPremiereXml).mockReturnValueOnce(pendingExport);

    const { unmount } = render(
      <ExportModal isOpen={true} onClose={mockOnClose} project={mockProject} />,
    );

    await waitFor(() => {
      expect(exportIpc.listenExportProgress).toHaveBeenCalledTimes(1);
    });

    fireEvent.click(screen.getByTestId('export-format-premiere'));
    fireEvent.click(screen.getByTestId('export-submit-button'));

    expect(screen.getByTestId('export-progress-container')).toBeInTheDocument();
    expect(screen.getByTestId('export-progress-bar')).toHaveAttribute('aria-valuenow', '0');

    // Emit cross-project event (must be ignored)
    act(() => {
      progressCallback?.({
        projectId: 'other_unrelated_project',
        percent: 99,
        currentStep: 'Wrong project',
      });
    });
    expect(screen.getByTestId('export-progress-bar')).toHaveAttribute('aria-valuenow', '0');

    // Emit matching project progress event
    act(() => {
      progressCallback?.({
        projectId: mockProject.projectId,
        percent: 45,
        currentStep: 'Đang dựng timeline FCP7 XML...',
      });
    });
    expect(screen.getByTestId('export-progress-bar')).toHaveAttribute('aria-valuenow', '45');
    expect(screen.getByTestId('export-progress-percent')).toHaveTextContent('45%');
    expect(screen.getByTestId('export-progress-step')).toHaveTextContent(
      'Đang dựng timeline FCP7 XML...',
    );

    act(() => {
      progressCallback?.({
        projectId: mockProject.projectId,
        percent: 80,
        currentStep: 'Đang ghi tệp project_timeline.xml...',
      });
    });
    expect(screen.getByTestId('export-progress-bar')).toHaveAttribute('aria-valuenow', '80');
    expect(screen.getByTestId('export-progress-percent')).toHaveTextContent('80%');

    // Resolve export -> progress container hides, success banner shows
    await act(async () => {
      resolveExport({
        xmlPath: 'C:\\NeuroClip\\export\\timeline.xml',
        sequenceName: 'Vedal & Neuro Clip',
        width: 1920,
        height: 1080,
        fps: 60,
        markerCount: 1,
        clipCount: 1,
        durationMs: 10,
      });
    });

    await waitFor(() => {
      expect(screen.getByTestId('export-success-banner')).toBeInTheDocument();
    });
    expect(screen.queryByTestId('export-progress-container')).not.toBeInTheDocument();

    unmount();
    expect(unlistenMock).toHaveBeenCalledTimes(1);
  });

  it('sanitizes Windows-illegal characters and traversal sequences in default and blank-fallback output paths', async () => {
    const dirtyProject: SubtitleProject = {
      ...mockProject,
      projectName: 'Neuro: "Clip" <1> | Test? * ../../evil',
    };

    const mockExportPremiere = vi.mocked(exportIpc.exportPremiereXml);
    mockExportPremiere.mockResolvedValue({
      xmlPath: 'C:\\NeuroClip\\export\\sanitized_timeline.xml',
      sequenceName: dirtyProject.projectName,
      width: 1920,
      height: 1080,
      fps: 60,
      markerCount: 1,
      clipCount: 1,
      durationMs: 5,
    });
    const mockExportAss = vi.mocked(exportIpc.exportSubtitlesProjectAss);
    mockExportAss.mockResolvedValue('C:\\NeuroClip\\export\\sanitized.ass');
    const mockExportSrt = vi.mocked(exportIpc.exportSubtitlesProjectSrt);
    mockExportSrt.mockResolvedValue('C:\\NeuroClip\\export\\sanitized.srt');

    const { rerender } = render(
      <ExportModal isOpen={true} onClose={mockOnClose} project={dirtyProject} />,
    );

    fireEvent.click(screen.getByTestId('export-format-premiere'));
    const pathInput = screen.getByTestId('custom-output-path-input') as HTMLInputElement;
    const fileSegment = pathInput.value.replace('C:\\NeuroClip\\export\\', '');
    expect(fileSegment).not.toMatch(/[<>:"/\\|?*]/);
    expect(fileSegment).not.toContain('..');
    expect(fileSegment.endsWith('_timeline.xml')).toBe(true);

    // Clear input -> fallback must also sanitize
    fireEvent.change(pathInput, { target: { value: '   ' } });
    fireEvent.click(screen.getByTestId('export-submit-button'));
    await waitFor(() => {
      expect(mockExportPremiere).toHaveBeenCalledTimes(1);
    });
    const calledPremierePath = mockExportPremiere.mock.calls[0]?.[1] ?? '';
    expect(calledPremierePath.replace('C:\\NeuroClip\\export\\', '')).not.toMatch(/[<>:"/\\|?*]/);
    expect(calledPremierePath).not.toContain('..');

    // Reopen with blank/dots-only projectName & fallback to projectId for ASS & SRT
    const fallbackProject: SubtitleProject = {
      ...mockProject,
      projectId: 'safe:id/01',
      projectName: '  ...  ',
    };
    rerender(<ExportModal isOpen={false} onClose={mockOnClose} project={fallbackProject} />);
    rerender(<ExportModal isOpen={true} onClose={mockOnClose} project={fallbackProject} />);

    fireEvent.click(screen.getByTestId('export-format-ass'));
    const assInput = screen.getByTestId('custom-output-path-input') as HTMLInputElement;
    expect(assInput.value).toBe('C:\\NeuroClip\\export\\safe_id_01.ass');

    fireEvent.change(assInput, { target: { value: '' } });
    fireEvent.click(screen.getByTestId('export-submit-button'));
    await waitFor(() => {
      expect(mockExportAss).toHaveBeenCalledWith(
        fallbackProject,
        'C:\\NeuroClip\\export\\safe_id_01.ass',
      );
    });

    rerender(<ExportModal isOpen={false} onClose={mockOnClose} project={fallbackProject} />);
    rerender(<ExportModal isOpen={true} onClose={mockOnClose} project={fallbackProject} />);

    fireEvent.click(screen.getByTestId('export-format-srt'));
    fireEvent.change(screen.getByTestId('custom-output-path-input'), { target: { value: '' } });
    fireEvent.click(screen.getByTestId('export-submit-button'));
    await waitFor(() => {
      expect(mockExportSrt).toHaveBeenCalledWith(
        fallbackProject,
        'C:\\NeuroClip\\export\\safe_id_01.srt',
      );
    });
  });

  it('preserves active success banner and custom output path across project object reference churn', async () => {
    const mockExportPremiere = vi.mocked(exportIpc.exportPremiereXml);
    mockExportPremiere.mockResolvedValueOnce({
      xmlPath: 'D:\\Custom\\edited_timeline.xml',
      sequenceName: 'Vedal & Neuro Clip',
      width: 1920,
      height: 1080,
      fps: 60,
      markerCount: 1,
      clipCount: 1,
      durationMs: 10,
    });

    const { rerender } = render(
      <ExportModal isOpen={true} onClose={mockOnClose} project={mockProject} />,
    );

    fireEvent.click(screen.getByTestId('export-format-premiere'));
    const pathInput = screen.getByTestId('custom-output-path-input') as HTMLInputElement;
    fireEvent.change(pathInput, { target: { value: 'D:\\Custom\\edited_timeline.xml' } });
    expect(pathInput.value).toBe('D:\\Custom\\edited_timeline.xml');

    // Rerender with fresh project object reference (same projectId) before export
    rerender(
      <ExportModal
        isOpen={true}
        onClose={mockOnClose}
        project={{ ...mockProject, updatedAt: '2026-11-22T12:00:00Z' }}
      />,
    );
    expect((screen.getByTestId('custom-output-path-input') as HTMLInputElement).value).toBe(
      'D:\\Custom\\edited_timeline.xml',
    );

    fireEvent.click(screen.getByTestId('export-submit-button'));
    await waitFor(() => {
      expect(screen.getByTestId('export-success-banner')).toBeInTheDocument();
    });

    // Rerender again with new project object reference while success banner is shown
    rerender(
      <ExportModal
        isOpen={true}
        onClose={mockOnClose}
        project={{ ...mockProject, updatedAt: '2026-11-22T12:05:00Z' }}
      />,
    );
    expect(screen.getByTestId('export-success-banner')).toBeInTheDocument();
    expect(screen.getByTestId('export-success-banner')).toHaveTextContent(
      'D:\\Custom\\edited_timeline.xml',
    );
  });

  it('ignores stale export resolution after modal close and reopen', async () => {
    let resolveStaleExport!: (value: exportIpc.PremiereExportResult) => void;
    const stalePromise = new Promise<exportIpc.PremiereExportResult>((resolve) => {
      resolveStaleExport = resolve;
    });
    vi.mocked(exportIpc.exportPremiereXml).mockReturnValueOnce(stalePromise);

    const { rerender } = render(
      <ExportModal
        isOpen={true}
        onClose={mockOnClose}
        project={mockProject}
        onExportSuccess={mockOnExportSuccess}
      />,
    );

    fireEvent.click(screen.getByTestId('export-format-premiere'));
    fireEvent.click(screen.getByTestId('export-submit-button'));
    expect(screen.getByTestId('export-progress-container')).toBeInTheDocument();

    // Parent closes and reopens modal before pending promise settles
    rerender(
      <ExportModal
        isOpen={false}
        onClose={mockOnClose}
        project={mockProject}
        onExportSuccess={mockOnExportSuccess}
      />,
    );
    rerender(
      <ExportModal
        isOpen={true}
        onClose={mockOnClose}
        project={mockProject}
        onExportSuccess={mockOnExportSuccess}
      />,
    );

    await act(async () => {
      resolveStaleExport({
        xmlPath: 'C:\\stale.xml',
        sequenceName: 'Stale',
        width: 1920,
        height: 1080,
        fps: 60,
        markerCount: 1,
        clipCount: 1,
        durationMs: 5,
      });
    });

    expect(screen.queryByTestId('export-success-banner')).not.toBeInTheDocument();
    expect(mockOnExportSuccess).not.toHaveBeenCalled();
  });

  it('guards NaN progress percent, ignores empty projectId events, and falls back when videoFilePath prop is blank', async () => {
    let progressCallback: ((payload: exportIpc.ExportProgressPayload) => void) | undefined;
    vi.mocked(exportIpc.listenExportProgress).mockImplementationOnce(async (cb) => {
      progressCallback = cb;
      return () => {};
    });

    let resolveExport!: (value: exportIpc.PremiereExportResult) => void;
    vi.mocked(exportIpc.exportPremiereXml).mockReturnValueOnce(
      new Promise<exportIpc.PremiereExportResult>((resolve) => {
        resolveExport = resolve;
      }),
    );

    render(
      <ExportModal isOpen={true} onClose={mockOnClose} project={mockProject} videoFilePath="   " />,
    );

    await waitFor(() => {
      expect(exportIpc.listenExportProgress).toHaveBeenCalledTimes(1);
    });

    fireEvent.click(screen.getByTestId('export-format-premiere'));
    fireEvent.click(screen.getByTestId('export-submit-button'));

    // Empty projectId event must be ignored when project.projectId is non-empty
    act(() => {
      progressCallback?.({
        projectId: '',
        percent: 88,
        currentStep: 'Untagged stream',
      });
    });
    expect(screen.getByTestId('export-progress-percent')).toHaveTextContent('0%');

    // NaN percent must safely clamp to 0% instead of rendering NaN%
    act(() => {
      progressCallback?.({
        projectId: mockProject.projectId,
        percent: Number.NaN,
        currentStep: 'Corrupted percent',
      });
    });
    expect(screen.getByTestId('export-progress-percent')).toHaveTextContent('0%');
    expect(screen.getByTestId('export-progress-bar')).toHaveAttribute('aria-valuenow', '0');

    // Blank videoFilePath prop must fall back to project.videoFilePath
    expect(exportIpc.exportPremiereXml).toHaveBeenCalledWith(mockProject, expect.any(String), {
      videoFilePath: 'C:\\Videos\\clip.mp4',
    });

    await act(async () => {
      resolveExport({
        xmlPath: 'C:\\NeuroClip\\export\\Vedal & Neuro Clip_timeline.xml',
        sequenceName: 'Vedal & Neuro Clip',
        width: 1920,
        height: 1080,
        fps: 60,
        markerCount: 1,
        clipCount: 1,
        durationMs: 5,
      });
    });
  });
});
