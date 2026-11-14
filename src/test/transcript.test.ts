/**
 * Vitest Test Suite for Whisper STT, Transcript Normalization, and Subtitle Contracts
 *
 * Complies with docs/CONVENTIONS.md (§7 Testing & QA) and TASK-M4-08 QA checklist.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import {
  isSpeakerRole,
  isWordTimingJson,
  isTranscriptSegmentJson,
  isTranscriptSegmentsContainerJson,
  isSpeakerStyleJson,
  isSubtitleProjectJson,
  isModelInfoJson,
  isSttProgressPayloadJson,
  normalizeWordTiming,
  normalizeTranscriptSegment,
  normalizeTranscriptSegmentsContainer,
  normalizeSubtitleProject,
  toTranscriptSegmentsContainerJson,
  type WordTimingJson,
  type TranscriptSegmentJson,
  type TranscriptSegmentsContainerJson,
  type SubtitleProjectJson,
  type ModelInfoJson,
} from '../types/transcript.ts';
import { useTranscriptStore } from '../stores/useTranscriptStore.ts';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

const mockedInvoke = vi.mocked(invoke);

describe('Schema 5.5 & 5.6 Transcript Contracts & Type Guards', () => {
  const sampleWordJson: WordTimingJson = {
    word: 'Hello',
    start_sec: 1.0,
    end_sec: 1.5,
    probability: 0.98,
  };

  const sampleSegmentJson: TranscriptSegmentJson = {
    segment_id: 'seg_0001',
    start_sec: 1.0,
    end_sec: 3.5,
    speaker_id: 'neuro',
    text_en: 'Hello Vedal tutel',
    text_vi: 'Chào Vedal tutel',
    confidence: 0.95,
    cps_score: 12.0,
    is_flagged_for_review: false,
    words: [sampleWordJson],
  };

  const sampleContainerJson: TranscriptSegmentsContainerJson = {
    clip_id: 'clip_sample_123',
    segments: [sampleSegmentJson],
  };

  const sampleProjectJson: SubtitleProjectJson = {
    project_id: 'subproj_clip_sample_123',
    project_name: 'Highlight Banter',
    vod_id: 'vod_12345',
    video_file_path: null,
    target_layout: 'original_16_9',
    segments: [sampleSegmentJson],
    speaker_styles: {
      neuro: {
        color_hex: '#FFB6C1',
        font_name: 'Arial',
        font_size: 54,
        outline_color_hex: '#000000',
      },
    },
    updated_at: '2026-11-14T14:40:00Z',
  };

  const sampleModelJson: ModelInfoJson = {
    modelName: 'base.en-q5_k',
    filename: 'ggml-base.en-q5_k.bin',
    path: 'C:/models/ggml-base.en-q5_k.bin',
    sizeBytes: 115000000,
    sha256: '587e9140a5a544c688319f07742d4a6c8e3ca638977ca310ab285ccb6b3e7bc2',
    isVerified: true,
  };

  it('validates SpeakerRole correctly', () => {
    expect(isSpeakerRole('neuro')).toBe(true);
    expect(isSpeakerRole('vedal')).toBe(true);
    expect(isSpeakerRole('evil')).toBe(true);
    expect(isSpeakerRole('guest')).toBe(true);
    expect(isSpeakerRole('chat')).toBe(true);
    expect(isSpeakerRole('unknown')).toBe(true);
    expect(isSpeakerRole('alien')).toBe(false);
    expect(isSpeakerRole(null)).toBe(false);
  });

  it('validates WordTimingJson correctly with runtime type guard', () => {
    expect(isWordTimingJson(sampleWordJson)).toBe(true);
    expect(isWordTimingJson(null)).toBe(false);
    expect(isWordTimingJson({})).toBe(false);
    expect(isWordTimingJson({ word: 123, start_sec: 1.0, end_sec: 2.0 })).toBe(false);
    expect(isWordTimingJson({ word: 'hi', start_sec: 'invalid', end_sec: 2.0 })).toBe(false);
  });

  it('validates TranscriptSegmentJson correctly with runtime type guard', () => {
    expect(isTranscriptSegmentJson(sampleSegmentJson)).toBe(true);
    expect(isTranscriptSegmentJson(null)).toBe(false);
    expect(isTranscriptSegmentJson({ segment_id: 'seg_1' })).toBe(false);
    expect(
      isTranscriptSegmentJson({
        ...sampleSegmentJson,
        confidence: 'not_a_number',
      }),
    ).toBe(false);
  });

  it('validates TranscriptSegmentsContainerJson correctly', () => {
    expect(isTranscriptSegmentsContainerJson(sampleContainerJson)).toBe(true);
    expect(isTranscriptSegmentsContainerJson(null)).toBe(false);
    expect(isTranscriptSegmentsContainerJson({ clip_id: '123' })).toBe(false);
    expect(
      isTranscriptSegmentsContainerJson({
        clip_id: '123',
        segments: [{ invalid: true }],
      }),
    ).toBe(false);
  });

  it('validates SubtitleProjectJson and ModelInfoJson correctly', () => {
    expect(isSpeakerStyleJson(sampleProjectJson.speaker_styles['neuro'])).toBe(true);
    expect(isSpeakerStyleJson(null)).toBe(false);
    expect(isSubtitleProjectJson(sampleProjectJson)).toBe(true);
    expect(isSubtitleProjectJson(null)).toBe(false);
    expect(isModelInfoJson(sampleModelJson)).toBe(true);
    expect(isModelInfoJson(null)).toBe(false);
    expect(
      isSttProgressPayloadJson({
        taskId: 'task_1',
        stage: 'transcribing',
        percent: 45.0,
      }),
    ).toBe(true);
    expect(isSttProgressPayloadJson(null)).toBe(false);
  });

  it('normalizes domain models with dual camelCase and snake_case properties', () => {
    const singleWordNorm = normalizeWordTiming(sampleWordJson);
    expect(singleWordNorm.word).toBe('Hello');
    expect(singleWordNorm.startSec).toBe(1.0);
    expect(singleWordNorm.start_sec).toBe(1.0);
    expect(singleWordNorm.probability).toBe(0.98);

    const norm = normalizeTranscriptSegment(sampleSegmentJson);
    expect(norm.segmentId).toBe('seg_0001');
    expect(norm.segment_id).toBe('seg_0001');
    expect(norm.startSec).toBe(1.0);
    expect(norm.start_sec).toBe(1.0);
    expect(norm.speakerId).toBe('neuro');
    expect(norm.speaker_id).toBe('neuro');
    expect(norm.textEn).toBe('Hello Vedal tutel');
    expect(norm.text_en).toBe('Hello Vedal tutel');
    expect(norm.textVi).toBe('Chào Vedal tutel');
    expect(norm.text_vi).toBe('Chào Vedal tutel');
    expect(norm.cpsScore).toBe(12.0);
    expect(norm.cps_score).toBe(12.0);
    expect(norm.words.length).toBe(1);
    expect(norm.words[0]?.startSec).toBe(1.0);
    expect(norm.words[0]?.start_sec).toBe(1.0);

    const containerNorm = normalizeTranscriptSegmentsContainer(sampleContainerJson);
    expect(containerNorm.clipId).toBe('clip_sample_123');
    expect(containerNorm.clip_id).toBe('clip_sample_123');
    expect(containerNorm.segments.length).toBe(1);

    const wireJson = toTranscriptSegmentsContainerJson(containerNorm);
    expect(wireJson.clip_id).toBe('clip_sample_123');
    expect(wireJson.segments[0]?.segment_id).toBe('seg_0001');
    expect(wireJson.segments[0]?.text_en).toBe('Hello Vedal tutel');
  });

  it('normalizes SubtitleProject correctly', () => {
    const normProject = normalizeSubtitleProject(sampleProjectJson);
    expect(normProject.projectId).toBe('subproj_clip_sample_123');
    expect(normProject.project_id).toBe('subproj_clip_sample_123');
    expect(normProject.targetLayout).toBe('original_16_9');
    expect(normProject.target_layout).toBe('original_16_9');
    expect(normProject.speakerStyles['neuro']?.colorHex).toBe('#FFB6C1');
    expect(normProject.speakerStyles['neuro']?.color_hex).toBe('#FFB6C1');
  });
});

describe('useTranscriptStore Reactive Store Invariants', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    act(() => {
      useTranscriptStore.getState().reset();
    });
  });

  afterEach(() => {
    act(() => {
      useTranscriptStore.getState().reset();
    });
  });

  it('returns stable reference from getState()', () => {
    const s1 = useTranscriptStore.getState();
    const s2 = useTranscriptStore.getState();
    expect(Object.is(s1, s2)).toBe(true);
  });

  it('renders hook with useSyncExternalStore without infinite loop', () => {
    const { result } = renderHook(() => useTranscriptStore());
    expect(result.current.clipId).toBeNull();
    expect(result.current.isTranscribing).toBe(false);
    expect(result.current.container).toBeNull();
  });

  it('executes fine-grained selector subscription updates', () => {
    const { result } = renderHook(() =>
      useTranscriptStore((state) => state.isTranscribing),
    );
    expect(result.current).toBe(false);

    act(() => {
      useTranscriptStore.setState({ isTranscribing: true });
    });

    expect(result.current).toBe(true);
  });

  it('handles transcribe action lifecycle successfully', async () => {
    const sampleResponse: TranscriptSegmentsContainerJson = {
      clip_id: 'clip_777',
      segments: [
        {
          segment_id: 'seg_0001',
          start_sec: 0.5,
          end_sec: 2.5,
          speaker_id: 'neuro',
          text_en: 'Hello everyone!',
          text_vi: null,
          confidence: 0.99,
          cps_score: 8.0,
          is_flagged_for_review: false,
          words: [],
        },
      ],
    };

    mockedInvoke.mockResolvedValueOnce(sampleResponse);

    const { result } = renderHook(() => useTranscriptStore());

    await act(async () => {
      await result.current.transcribe('clip_777', 'C:/audio.wav', 'base.en-q5_k');
    });

    expect(mockedInvoke).toHaveBeenCalledWith('start_whisper_transcription', {
      clipId: 'clip_777',
      audioPath: 'C:/audio.wav',
      modelName: 'base.en-q5_k',
    });

    expect(result.current.isTranscribing).toBe(false);
    expect(result.current.clipId).toBe('clip_777');
    expect(result.current.container).not.toBeNull();
    expect(result.current.container?.segments.length).toBe(1);
    expect(result.current.activeSegmentId).toBe('seg_0001');
  });

  it('allows updating segment Vietnamese translation and speaker', async () => {
    const sampleResponse: TranscriptSegmentsContainerJson = {
      clip_id: 'clip_edit_1',
      segments: [
        {
          segment_id: 'seg_0001',
          start_sec: 0.0,
          end_sec: 2.0,
          speaker_id: 'unknown',
          text_en: 'Tutel time',
          text_vi: null,
          confidence: 0.95,
          cps_score: 5.0,
          is_flagged_for_review: false,
          words: [],
        },
      ],
    };

    mockedInvoke.mockResolvedValueOnce(sampleResponse);

    const { result } = renderHook(() => useTranscriptStore());

    await act(async () => {
      await result.current.transcribe('clip_edit_1', 'C:/audio.wav');
    });

    act(() => {
      result.current.updateSegmentText('seg_0001', 'Đến giờ tutel');
      result.current.updateSegmentSpeaker('seg_0001', 'vedal');
    });

    const updatedSeg = result.current.container?.segments[0];
    expect(updatedSeg?.textVi).toBe('Đến giờ tutel');
    expect(updatedSeg?.speakerId).toBe('vedal');
  });

  it('exports ASS and SRT subtitles successfully', async () => {
    const sampleResponse: TranscriptSegmentsContainerJson = {
      clip_id: 'clip_exp_1',
      segments: [
        {
          segment_id: 'seg_0001',
          start_sec: 1.0,
          end_sec: 3.0,
          speaker_id: 'neuro',
          text_en: 'Export subtitle test',
          text_vi: null,
          confidence: 0.95,
          cps_score: 9.0,
          is_flagged_for_review: false,
          words: [],
        },
      ],
    };

    mockedInvoke.mockResolvedValueOnce(sampleResponse);
    mockedInvoke.mockResolvedValueOnce('C:/subtitles.ass');
    mockedInvoke.mockResolvedValueOnce('C:/subtitles.srt');

    const { result } = renderHook(() => useTranscriptStore());

    await act(async () => {
      await result.current.transcribe('clip_exp_1', 'C:/audio.wav');
    });

    await act(async () => {
      const assPath = await result.current.exportAss('C:/subtitles.ass');
      expect(assPath).toBe('C:/subtitles.ass');
    });

    await act(async () => {
      const srtPath = await result.current.exportSrt('C:/subtitles.srt');
      expect(srtPath).toBe('C:/subtitles.srt');
    });
  });

  it('resets state completely to baseline', () => {
    act(() => {
      useTranscriptStore.setState({
        clipId: 'dirty_clip',
        isTranscribing: true,
        activeSegmentId: 'seg_123',
      });
    });

    expect(useTranscriptStore.getState().clipId).toBe('dirty_clip');

    act(() => {
      useTranscriptStore.getState().reset();
    });

    const clean = useTranscriptStore.getState();
    expect(clean.clipId).toBeNull();
    expect(clean.isTranscribing).toBe(false);
    expect(clean.activeSegmentId).toBeNull();
    expect(clean.container).toBeNull();
  });
});
