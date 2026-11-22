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
    exportSubtitlesProjectAss: vi.fn(),
    exportSubtitlesProjectSrt: vi.fn(),
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
});
