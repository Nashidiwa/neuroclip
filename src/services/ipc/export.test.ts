import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import {
  exportSubtitlesAss,
  exportSubtitlesSrt,
  exportSubtitleProject,
  exportSubtitlesDual,
  VTUBER_PALETTE_HEX,
  VTUBER_PALETTE_ASS_BGR,
  isSubtitleExportFormat,
  isTargetLayout,
} from './export.ts';
import { ERROR_CODES, type IpcErrorResponse } from './error.ts';
import {
  type TranscriptSegmentsContainer,
  type SubtitleProject,
  normalizeTranscriptSegmentsContainer,
  normalizeSubtitleProject,
} from '../../types/transcript.ts';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

describe('TASK-M4-24: Subtitle Export Engine IPC Client', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  const sampleContainer: TranscriptSegmentsContainer = normalizeTranscriptSegmentsContainer({
    clip_id: 'clip_sample_export',
    segments: [
      {
        segment_id: 'seg_01',
        start_sec: 1.25,
        end_sec: 3.8,
        speaker_id: 'neuro',
        text_en: 'Hello tutel!',
        text_vi: 'Chào tutel!',
        confidence: 0.98,
        cps_score: 12.5,
        is_flagged_for_review: false,
        words: [
          {
            word: 'Hello',
            start_sec: 1.25,
            end_sec: 2.0,
            probability: 0.99,
          },
        ],
      },
      {
        segment_id: 'seg_02',
        start_sec: 4.0,
        end_sec: 6.5,
        speaker_id: 'vedal',
        text_en: 'Vedal response.',
        text_vi: null,
        confidence: 0.94,
        cps_score: 11.0,
        is_flagged_for_review: false,
        words: [],
      },
    ],
  });

  const sampleProject: SubtitleProject = normalizeSubtitleProject({
    project_id: 'subproj_clip_sample_export',
    project_name: 'Vedal and Neuro Banter',
    vod_id: 'vod_sample_123',
    video_file_path: '/path/to/video.mp4',
    target_layout: 'preset_center_crop_9_16',
    segments: [
      {
        segment_id: 'seg_01',
        start_sec: 1.25,
        end_sec: 3.8,
        speaker_id: 'neuro',
        text_en: 'Hello tutel!',
        text_vi: 'Chào tutel!',
        confidence: 0.98,
        cps_score: 12.5,
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
      vedal: {
        color_hex: '#98FB98',
        font_name: 'Arial',
        font_size: 54,
        outline_color_hex: '#000000',
      },
    },
    updated_at: '2026-11-20T00:00:00Z',
  });

  it('UT-24-TS.1: exportSubtitlesAss invokes tauri with transformed snake_case container payload', async () => {
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockResolvedValueOnce('C:\\export\\subtitles.ass');

    const result = await exportSubtitlesAss(sampleContainer, 'C:\\export\\subtitles.ass');

    expect(result).toBe('C:\\export\\subtitles.ass');
    expect(mockInvoke).toHaveBeenCalledTimes(1);
    expect(mockInvoke).toHaveBeenCalledWith('export_subtitles_ass', {
      container: {
        clip_id: 'clip_sample_export',
        segments: expect.arrayContaining([
          expect.objectContaining({
            segment_id: 'seg_01',
            start_sec: 1.25,
            end_sec: 3.8,
            speaker_id: 'neuro',
            text_en: 'Hello tutel!',
            text_vi: 'Chào tutel!',
          }),
        ]),
      },
      outputPath: 'C:\\export\\subtitles.ass',
    });
  });

  it('UT-24-TS.2: exportSubtitlesSrt invokes tauri with transformed snake_case container payload', async () => {
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockResolvedValueOnce('C:\\export\\subtitles.srt');

    const result = await exportSubtitlesSrt(sampleContainer, 'C:\\export\\subtitles.srt');

    expect(result).toBe('C:\\export\\subtitles.srt');
    expect(mockInvoke).toHaveBeenCalledTimes(1);
    expect(mockInvoke).toHaveBeenCalledWith('export_subtitles_srt', {
      container: {
        clip_id: 'clip_sample_export',
        segments: expect.arrayContaining([
          expect.objectContaining({
            segment_id: 'seg_02',
            start_sec: 4.0,
            end_sec: 6.5,
            speaker_id: 'vedal',
            text_en: 'Vedal response.',
          }),
        ]),
      },
      outputPath: 'C:\\export\\subtitles.srt',
    });
  });

  it('UT-24-TS.3: exportSubtitleProject invokes tauri with Schema 5.6 JSON wire structure', async () => {
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockResolvedValueOnce('C:\\export\\project.json');

    const result = await exportSubtitleProject(sampleProject, 'C:\\export\\project.json');

    expect(result).toBe('C:\\export\\project.json');
    expect(mockInvoke).toHaveBeenCalledTimes(1);
    expect(mockInvoke).toHaveBeenCalledWith('export_subtitle_project', {
      project: expect.objectContaining({
        project_id: 'subproj_clip_sample_export',
        project_name: 'Vedal and Neuro Banter',
        vod_id: 'vod_sample_123',
        target_layout: 'preset_center_crop_9_16',
        video_file_path: '/path/to/video.mp4',
        segments: expect.any(Array),
        speaker_styles: expect.objectContaining({
          neuro: {
            color_hex: '#FFB6C1',
            font_name: 'Arial',
            font_size: 54,
            outline_color_hex: '#000000',
          },
        }),
      }),
      outputPath: 'C:\\export\\project.json',
    });
  });

  it('UT-24-TS.4: exportSubtitlesDual executes both ASS and SRT exports simultaneously (ADR-005)', async () => {
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockImplementation(async (cmd: string, args: unknown) => {
      const { outputPath } = args as { outputPath: string };
      if (cmd === 'export_subtitles_ass') return outputPath;
      if (cmd === 'export_subtitles_srt') return outputPath;
      throw new Error(`Unexpected command: ${cmd}`);
    });

    const dualResult = await exportSubtitlesDual(sampleContainer, 'C:\\export\\video_clip');

    expect(dualResult.assPath).toBe('C:\\export\\video_clip.ass');
    expect(dualResult.srtPath).toBe('C:\\export\\video_clip.srt');
    expect(mockInvoke).toHaveBeenCalledTimes(2);
    expect(mockInvoke).toHaveBeenCalledWith('export_subtitles_ass', expect.anything());
    expect(mockInvoke).toHaveBeenCalledWith('export_subtitles_srt', expect.anything());
  });

  it('UT-24-TS.5: error handling parses raw backend error into normalized IpcErrorResponse', async () => {
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockRejectedValueOnce({
      code: ERROR_CODES.ERR_EXPORT_FILE_WRITE_FAILED,
      message: 'Lỗi ghi file dự án NLE',
      details: 'Permission denied (os error 13)',
      recoverable: true,
    });

    try {
      await exportSubtitlesAss(sampleContainer, 'C:\\protected\\subtitles.ass');
      expect.unreachable('Should have thrown an error');
    } catch (err: unknown) {
      const ipcError = err as IpcErrorResponse;
      expect(ipcError.code).toBe(ERROR_CODES.ERR_EXPORT_FILE_WRITE_FAILED);
      expect(ipcError.recoverable).toBe(true);
      expect(ipcError.details).toBe('Permission denied (os error 13)');
    }
  });

  it('UT-24-TS.6: VTUBER_PALETTE_HEX and VTUBER_PALETTE_ASS_BGR match PO Option 1 specifications', () => {
    // Hex RGB palette
    expect(VTUBER_PALETTE_HEX.vedal).toBe('#98FB98');
    expect(VTUBER_PALETTE_HEX.neuro).toBe('#FFB6C1');
    expect(VTUBER_PALETTE_HEX.evil).toBe('#FF0055');
    expect(VTUBER_PALETTE_HEX.guest).toBe('#FFD700');
    expect(VTUBER_PALETTE_HEX.chat).toBe('#87CEEB');
    expect(VTUBER_PALETTE_HEX.unknown).toBe('#FFFFFF');

    // ASS BGR format (&HAABBGGRR&)
    expect(VTUBER_PALETTE_ASS_BGR.vedal).toBe('&H0098FB98&');
    expect(VTUBER_PALETTE_ASS_BGR.neuro).toBe('&H00C1B6FF&');
    expect(VTUBER_PALETTE_ASS_BGR.evil).toBe('&H005500FF&');
    expect(VTUBER_PALETTE_ASS_BGR.guest).toBe('&H0000D7FF&');
    expect(VTUBER_PALETTE_ASS_BGR.chat).toBe('&H00EBCE87&');
    expect(VTUBER_PALETTE_ASS_BGR.unknown).toBe('&H00FFFFFF&');
  });

  it('UT-24-TS.7: validates type guards isSubtitleExportFormat and isTargetLayout', () => {
    expect(isSubtitleExportFormat('ass')).toBe(true);
    expect(isSubtitleExportFormat('srt')).toBe(true);
    expect(isSubtitleExportFormat('project_json')).toBe(true);
    expect(isSubtitleExportFormat('mp4')).toBe(false);
    expect(isSubtitleExportFormat(123)).toBe(false);

    expect(isTargetLayout('original_16_9')).toBe(true);
    expect(isTargetLayout('preset_center_crop_9_16')).toBe(true);
    expect(isTargetLayout('preset_two_stack_9_16')).toBe(true);
    expect(isTargetLayout('invalid_layout')).toBe(false);
  });
});
