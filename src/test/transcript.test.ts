/**
 * Vitest Test Suite for Whisper STT, Transcript Normalization, and Subtitle Contracts
 *
 * Complies with docs/CONVENTIONS.md (§7 Testing & QA) and TASK-M4-08 QA checklist.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
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
import {
  STT_CHUNK_EVENT,
  STT_PROGRESS_EVENT,
  startFasterWhisperTranscription,
} from '../services/ipc/stt.ts';
import { ERROR_CODES } from '../services/ipc/error.ts';
import { useTranscriptStore } from '../stores/useTranscriptStore.ts';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

const mockedInvoke = vi.mocked(invoke);
const mockedListen = vi.mocked(listen);

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
    const { result } = renderHook(() => useTranscriptStore((state) => state.isTranscribing));
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
    expect(clean.streamedChunks).toEqual([]);
    expect(clean.isTranslating).toBe(false);
    expect(clean.translationProgress).toBeNull();
    expect(clean.isTestingGeminiConnection).toBe(false);
    expect(clean.isGeminiConnected).toBeNull();
  });

  it('invokes startFasterWhisperTranscription with M2 IPC contract and guardrailPolicy', async () => {
    const sampleResponse: TranscriptSegmentsContainerJson = {
      clip_id: 'clip_fw_1',
      segments: [
        {
          segment_id: 'seg_fw_1',
          start_sec: 0.0,
          end_sec: 2.0,
          speaker_id: 'vedal',
          text_en: 'Faster whisper test',
          text_vi: null,
          confidence: 0.94,
          cps_score: 9.5,
          is_flagged_for_review: false,
          words: [],
        },
      ],
    };

    mockedInvoke.mockResolvedValueOnce(sampleResponse);
    const direct = await startFasterWhisperTranscription('clip_fw_1', 'C:/audio.wav');
    expect(direct.clipId).toBe('clip_fw_1');
    expect(mockedInvoke).toHaveBeenCalledWith('start_faster_whisper_transcription', {
      clipId: 'clip_fw_1',
      audioPath: 'C:/audio.wav',
      model: null,
      quantization: null,
      device: null,
      guardrailPolicy: null,
    });

    mockedInvoke.mockResolvedValueOnce(sampleResponse);
    const { result } = renderHook(() => useTranscriptStore());
    await act(async () => {
      await result.current.transcribeFasterWhisper('clip_fw_1', 'C:/audio.wav', {
        model: 'medium',
        quantization: 'int8',
        device: 'cuda',
        guardrailPolicy: 'strict_reject',
      });
    });

    expect(mockedInvoke).toHaveBeenLastCalledWith('start_faster_whisper_transcription', {
      clipId: 'clip_fw_1',
      audioPath: 'C:/audio.wav',
      model: 'medium',
      quantization: 'int8',
      device: 'cuda',
      guardrailPolicy: 'strict_reject',
    });
    expect(result.current.container?.segments[0]?.segmentId).toBe('seg_fw_1');
  });

  it('streams stt:chunk and stt:progress payloads during transcribeClip', async () => {
    const eventCallbacks = new Map<string, (event: { payload: unknown }) => void>();
    mockedListen.mockImplementation(async (eventName, cb) => {
      eventCallbacks.set(eventName, cb as (event: { payload: unknown }) => void);
      return () => {};
    });

    let resolveInvoke!: (val: TranscriptSegmentsContainerJson) => void;
    mockedInvoke.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveInvoke = resolve;
        }),
    );

    const { result } = renderHook(() => useTranscriptStore());
    let pendingPromise!: Promise<unknown>;

    await act(async () => {
      pendingPromise = result.current.transcribeClip('clip_groq_1', 'C:/clip.wav', {
        apiKey: 'gsk_123',
        isMusicMode: true,
      });
      await Promise.resolve();
    });

    expect(mockedInvoke).toHaveBeenCalledWith('transcribe_highlight_clip', {
      clipId: 'clip_groq_1',
      audioPath: 'C:/clip.wav',
      apiKey: 'gsk_123',
      isMusicMode: true,
    });

    const chunkCb = eventCallbacks.get(STT_CHUNK_EVENT);
    const progressCb = eventCallbacks.get(STT_PROGRESS_EVENT);
    expect(chunkCb).toBeDefined();
    expect(progressCb).toBeDefined();

    act(() => {
      progressCb?.({
        payload: {
          taskId: 'clip_groq_1',
          stage: 'transcribing',
          percent: 50,
          currentChunk: 1,
          totalChunks: 2,
        },
      });
      chunkCb?.({
        payload: {
          clip_id: 'clip_groq_1',
          chunk_index: 0,
          segment: {
            segment_id: 'seg_stream_1',
            start_sec: 0.0,
            end_sec: 1.5,
            speaker_id: 'neuro',
            text_en: 'Streamed line',
            confidence: 0.96,
            cps_score: 8.6,
          },
        },
      });
    });

    expect(result.current.progress?.percent).toBe(50);
    expect(result.current.streamedChunks.length).toBe(1);
    expect(result.current.streamedChunks[0]?.segmentId).toBe('seg_stream_1');

    await act(async () => {
      resolveInvoke({
        clip_id: 'clip_groq_1',
        segments: [
          {
            segment_id: 'seg_stream_1',
            start_sec: 0.0,
            end_sec: 1.5,
            speaker_id: 'neuro',
            text_en: 'Streamed line',
            confidence: 0.96,
            cps_score: 8.6,
          },
        ],
      });
      await pendingPromise;
    });

    expect(result.current.isTranscribing).toBe(false);
    expect(result.current.container?.clipId).toBe('clip_groq_1');
  });

  it('wires translate, translation:progress, testGeminiConnection, and syncSpeakerRole', async () => {
    const eventCallbacks = new Map<string, (event: { payload: unknown }) => void>();
    mockedListen.mockImplementation(async (eventName, cb) => {
      eventCallbacks.set(eventName, cb as (event: { payload: unknown }) => void);
      return () => {};
    });

    const { result } = renderHook(() => useTranscriptStore());

    // Translating with null container must fail with ERR_TRANSLATE_API_FAILED
    await act(async () => {
      await expect(result.current.translate()).rejects.toMatchObject({
        code: ERROR_CODES.TRANSLATE_API_FAILED,
        recoverable: true,
      });
    });

    // Test Gemini connection success and failure
    mockedInvoke.mockResolvedValueOnce(true);
    await act(async () => {
      const ok = await result.current.testGeminiConnection('AIzaTestKey');
      expect(ok).toBe(true);
    });
    expect(mockedInvoke).toHaveBeenCalledWith('test_gemini_connection', {
      apiKey: 'AIzaTestKey',
    });
    expect(result.current.isGeminiConnected).toBe(true);

    mockedInvoke.mockRejectedValueOnce({
      code: ERROR_CODES.TRANSLATE_INVALID_API_KEY,
      message: 'Invalid Gemini key',
      recoverable: false,
    });
    await act(async () => {
      await expect(result.current.testGeminiConnection('bad')).rejects.toMatchObject({
        code: ERROR_CODES.TRANSLATE_INVALID_API_KEY,
      });
    });
    expect(result.current.isGeminiConnected).toBe(false);

    // Seed container and translate + syncSpeakerRole
    const baseContainer = normalizeTranscriptSegmentsContainer({
      clip_id: 'clip_tr_1',
      segments: [
        {
          segment_id: 'seg_tr_1',
          start_sec: 0.0,
          end_sec: 2.0,
          speaker_id: 'neuro',
          text_en: 'Heart!',
          text_vi: null,
          confidence: 0.99,
          cps_score: 3.0,
        },
      ],
    });

    act(() => {
      useTranscriptStore.setState({
        clipId: 'clip_tr_1',
        container: baseContainer,
        error: null,
      });
    });

    let resolveTranslate!: (val: TranscriptSegmentsContainerJson) => void;
    mockedInvoke.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveTranslate = resolve;
        }),
    );

    let translatePromise!: Promise<unknown>;
    await act(async () => {
      translatePromise = result.current.translate({
        apiKey: 'AIzaTestKey',
        model: 'gemini-1.5-flash',
      });
      await Promise.resolve();
    });

    act(() => {
      eventCallbacks.get('translation:progress')?.({
        payload: {
          clipId: 'clip_tr_1',
          translatedCount: 1,
          totalCount: 1,
        },
      });
    });
    expect(result.current.translationProgress?.percent).toBe(100);

    await act(async () => {
      resolveTranslate({
        clip_id: 'clip_tr_1',
        segments: [
          {
            segment_id: 'seg_tr_1',
            start_sec: 0.0,
            end_sec: 2.0,
            speaker_id: 'neuro',
            text_en: 'Heart!',
            text_vi: 'Tim nè!',
            confidence: 0.99,
            cps_score: 3.5,
          },
        ],
      });
      await translatePromise;
    });

    expect(result.current.isTranslating).toBe(false);
    expect(result.current.container?.segments[0]?.textVi).toBe('Tim nè!');

    mockedInvoke.mockResolvedValueOnce({
      clip_id: 'clip_tr_1',
      segments: [
        {
          segment_id: 'seg_tr_1',
          start_sec: 0.0,
          end_sec: 2.0,
          speaker_id: 'evil',
          text_en: 'Heart!',
          text_vi: 'Tim nè!',
          confidence: 0.99,
          cps_score: 3.5,
        },
      ],
    });

    await act(async () => {
      await result.current.syncSpeakerRole('seg_tr_1', 'evil');
    });
    expect(mockedInvoke).toHaveBeenLastCalledWith('update_speaker_role', {
      container: expect.objectContaining({ clip_id: 'clip_tr_1' }),
      segmentId: 'seg_tr_1',
      newSpeaker: 'evil',
    });
    expect(result.current.container?.segments[0]?.speakerId).toBe('evil');
  });

  it('detaches listeners, skips IPC, and blocks late events when reset() runs while listen() is pending', async () => {
    const capturedCallbacks = new Map<string, (event: { payload: unknown }) => void>();
    const unlistenSpies: Array<ReturnType<typeof vi.fn>> = [];
    const resolveListens: Array<(unlisten: () => void) => void> = [];

    mockedListen.mockImplementation((eventName, cb) => {
      capturedCallbacks.set(eventName, cb as (event: { payload: unknown }) => void);
      const unlisten = vi.fn();
      unlistenSpies.push(unlisten);
      return new Promise<() => void>((resolve) => {
        resolveListens.push(resolve);
      });
    });

    const { result } = renderHook(() => useTranscriptStore());
    let pendingTranscribe!: Promise<unknown>;

    act(() => {
      pendingTranscribe = result.current.transcribeClip('clip_race', 'C:/race.wav');
    });

    expect(mockedListen).toHaveBeenCalledTimes(3);
    expect(mockedInvoke).toHaveBeenCalledTimes(0);

    act(() => {
      result.current.reset();
    });

    await act(async () => {
      resolveListens.forEach((resolve, idx) => {
        const spy = unlistenSpies[idx];
        if (spy) {
          resolve(spy);
        }
      });
      await pendingTranscribe;
    });

    expect(unlistenSpies).toHaveLength(3);
    for (const spy of unlistenSpies) {
      expect(spy).toHaveBeenCalledTimes(1);
    }
    expect(mockedInvoke).toHaveBeenCalledTimes(0);

    // Emit late stt:progress and stt:chunk events after reset()
    act(() => {
      capturedCallbacks.get(STT_PROGRESS_EVENT)?.({
        payload: {
          taskId: 'clip_race',
          stage: 'transcribing',
          percent: 42,
          currentChunk: 1,
          totalChunks: 2,
        },
      });
      capturedCallbacks.get(STT_CHUNK_EVENT)?.({
        payload: {
          clip_id: 'clip_race',
          chunk_index: 0,
          segment: {
            segment_id: 'seg_late_1',
            start_sec: 0.0,
            end_sec: 1.0,
            speaker_id: 'neuro',
            text_en: 'Late chunk',
            confidence: 0.9,
            cps_score: 5.0,
          },
        },
      });
    });

    expect(result.current.clipId).toBeNull();
    expect(result.current.isTranscribing).toBe(false);
    expect(result.current.progress).toBeNull();
    expect(result.current.streamedChunks.length).toBe(0);
  });

  it('coalesces concurrent transcribeClip + translate listener setup to 3 listeners and keeps per-op loading flags independent', async () => {
    const unlistenSpies: Array<ReturnType<typeof vi.fn>> = [];
    const resolveListens: Array<(unlisten: () => void) => void> = [];

    mockedListen.mockImplementation(() => {
      const unlisten = vi.fn();
      unlistenSpies.push(unlisten);
      return new Promise<() => void>((resolve) => {
        resolveListens.push(resolve);
      });
    });

    const seedContainer = normalizeTranscriptSegmentsContainer({
      clip_id: 'clip_conc_1',
      segments: [
        {
          segment_id: 'seg_c1',
          start_sec: 0.0,
          end_sec: 1.5,
          speaker_id: 'neuro',
          text_en: 'Concurrent test',
          text_vi: null,
          confidence: 0.95,
          cps_score: 7.0,
        },
      ],
    });

    act(() => {
      useTranscriptStore.setState({
        clipId: 'clip_conc_1',
        container: seedContainer,
      });
    });

    let resolveTranscribe!: (val: TranscriptSegmentsContainerJson) => void;
    let resolveTranslate!: (val: TranscriptSegmentsContainerJson) => void;

    mockedInvoke.mockImplementation((cmd) => {
      if (cmd === 'transcribe_highlight_clip') {
        return new Promise((resolve) => {
          resolveTranscribe = resolve;
        });
      }
      if (cmd === 'translate_vietsub') {
        return new Promise((resolve) => {
          resolveTranslate = resolve;
        });
      }
      return Promise.resolve(null);
    });

    const { result } = renderHook(() => useTranscriptStore());
    let transcribePromise!: Promise<unknown>;
    let translatePromise!: Promise<unknown>;

    act(() => {
      transcribePromise = result.current.transcribeClip('clip_conc_1', 'C:/conc.wav');
      translatePromise = result.current.translate();
    });

    expect(mockedListen).toHaveBeenCalledTimes(3);

    await act(async () => {
      resolveListens.forEach((resolve, idx) => {
        const spy = unlistenSpies[idx];
        if (spy) {
          resolve(spy);
        }
      });
      await Promise.resolve();
    });

    await act(async () => {
      resolveTranscribe({
        clip_id: 'clip_conc_1',
        segments: [
          {
            segment_id: 'seg_c1',
            start_sec: 0.0,
            end_sec: 1.5,
            speaker_id: 'neuro',
            text_en: 'Concurrent test',
            text_vi: null,
            confidence: 0.95,
            cps_score: 7.0,
          },
        ],
      });
      await transcribePromise;
    });

    expect(result.current.isTranscribing).toBe(false);
    expect(result.current.isTranslating).toBe(true);

    await act(async () => {
      resolveTranslate({
        clip_id: 'clip_conc_1',
        segments: [
          {
            segment_id: 'seg_c1',
            start_sec: 0.0,
            end_sec: 1.5,
            speaker_id: 'neuro',
            text_en: 'Concurrent test',
            text_vi: 'Thử nghiệm đồng thời',
            confidence: 0.95,
            cps_score: 7.0,
          },
        ],
      });
      await translatePromise;
    });

    expect(result.current.isTranslating).toBe(false);
    expect(result.current.container?.segments[0]?.textVi).toBe('Thử nghiệm đồng thời');

    act(() => {
      result.current.reset();
    });

    expect(unlistenSpies).toHaveLength(3);
    for (const spy of unlistenSpies) {
      expect(spy).toHaveBeenCalledTimes(1);
    }
  });

  it('ignores verifyModel and testGeminiConnection results that resolve after reset()', async () => {
    let resolveVerify!: (val: ModelInfoJson) => void;
    let resolveGemini!: (val: boolean) => void;

    mockedInvoke.mockImplementation((cmd) => {
      if (cmd === 'verify_stt_model') {
        return new Promise((resolve) => {
          resolveVerify = resolve;
        });
      }
      if (cmd === 'test_gemini_connection') {
        return new Promise((resolve) => {
          resolveGemini = resolve;
        });
      }
      return Promise.resolve(null);
    });

    const { result } = renderHook(() => useTranscriptStore());
    let verifyPromise!: Promise<unknown>;
    let geminiPromise!: Promise<unknown>;

    act(() => {
      verifyPromise = result.current.verifyModel('base.en-q5_k');
      geminiPromise = result.current.testGeminiConnection('AIzaStaleKey');
    });

    act(() => {
      result.current.reset();
    });

    await act(async () => {
      resolveVerify({
        modelName: 'base.en-q5_k',
        filename: 'ggml-base.en-q5_k.bin',
        path: 'C:/models/ggml-base.en-q5_k.bin',
        sizeBytes: 115000000,
        sha256: 'abc',
        isVerified: true,
      });
      resolveGemini(true);
      await Promise.all([verifyPromise, geminiPromise]);
    });

    expect(result.current.isModelVerified).toBe(false);
    expect(result.current.isGeminiConnected).toBeNull();
    expect(result.current.isTestingGeminiConnection).toBe(false);
  });
});
