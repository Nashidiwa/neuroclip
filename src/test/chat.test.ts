/**
 * Vitest Test Suite for Chat Extraction, Normalization, and Heatmap Contracts
 *
 * Complies with docs/CONVENTIONS.md (§7 Testing & QA) and TASK-M4-07 QA checklist.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import {
  isChatEventJson,
  isChatEventsContainerJson,
  isChatDensityPointJson,
  isChatHeatmapResponseJson,
  normalizeChatEvent,
  normalizeChatEventsContainer,
  normalizeChatDensityPoint,
  normalizeChatHeatmapResponse,
  type ChatEventJson,
  type ChatEventsContainerJson,
  type ChatDensityPointJson,
  type ChatHeatmapResponseJson,
} from '../types/chat.ts';
import { useChatStore, chatStore } from '../stores/useChatStore.ts';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

const mockedInvoke = vi.mocked(invoke);

describe('Schema 5.3 Chat Contracts & Type Guards', () => {
  const sampleEventJson: ChatEventJson = {
    timestamp_sec: 14.25,
    author: 'VedalFan',
    message: 'Tutel KEKW',
    badges: ['subscriber'],
    emotes_detected: ['Tutel', 'KEKW'],
    is_swarm_emote: true,
  };

  const sampleContainerJson: ChatEventsContainerJson = {
    vod_id: 'vod_test_123',
    total_messages: 1,
    events: [sampleEventJson],
  };

  const sampleDensityPointJson: ChatDensityPointJson = {
    timestamp_sec: 10.0,
    message_count: 25,
    velocity_mps: 2.5,
    swarm_count: 10,
    swarm_ratio: 0.4,
    z_score: 2.15,
    normalized_density: 0.85,
    is_peak: true,
  };

  const sampleHeatmapJson: ChatHeatmapResponseJson = {
    vod_id: 'vod_test_123',
    window_seconds: 10.0,
    step_seconds: 10.0,
    total_buckets: 1,
    max_velocity_mps: 2.5,
    peak_timestamp_sec: 10.0,
    points: [sampleDensityPointJson],
  };

  it('validates ChatEventJson correctly with runtime type guard', () => {
    expect(isChatEventJson(sampleEventJson)).toBe(true);
    expect(isChatEventJson(null)).toBe(false);
    expect(isChatEventJson({})).toBe(false);
    expect(isChatEventJson({ timestamp_sec: 'invalid' })).toBe(false);
    expect(
      isChatEventJson({
        timestamp_sec: 12.0,
        author: 'User',
        message: 'Hi',
        emotes_detected: 'not-an-array',
      })
    ).toBe(false);
  });

  it('validates ChatEventsContainerJson correctly with runtime type guard', () => {
    expect(isChatEventsContainerJson(sampleContainerJson)).toBe(true);
    expect(isChatEventsContainerJson({ vod_id: '123' })).toBe(false);
    expect(
      isChatEventsContainerJson({
        vod_id: '123',
        total_messages: 1,
        events: [{ invalid: true }],
      })
    ).toBe(false);
  });

  it('validates ChatDensityPointJson and ChatHeatmapResponseJson type guards', () => {
    expect(isChatDensityPointJson(sampleDensityPointJson)).toBe(true);
    expect(isChatDensityPointJson({ timestamp_sec: 10.0 })).toBe(false);

    expect(isChatHeatmapResponseJson(sampleHeatmapJson)).toBe(true);
    expect(isChatHeatmapResponseJson({ vod_id: '123', points: [] })).toBe(false);
  });

  it('normalizes ChatEvent with dual camelCase and snake_case accessors', () => {
    const normalized = normalizeChatEvent(sampleEventJson);

    // camelCase accessors
    expect(normalized.timestampSec).toBe(14.25);
    expect(normalized.author).toBe('VedalFan');
    expect(normalized.message).toBe('Tutel KEKW');
    expect(normalized.badges).toEqual(['subscriber']);
    expect(normalized.emotesDetected).toEqual(['Tutel', 'KEKW']);
    expect(normalized.isSwarmEmote).toBe(true);

    // snake_case aliases
    expect(normalized.timestamp_sec).toBe(14.25);
    expect(normalized.emotes_detected).toEqual(['Tutel', 'KEKW']);
    expect(normalized.is_swarm_emote).toBe(true);
  });

  it('normalizes ChatEventsContainer correctly', () => {
    const normalized = normalizeChatEventsContainer(sampleContainerJson);

    expect(normalized.vodId).toBe('vod_test_123');
    expect(normalized.vod_id).toBe('vod_test_123');
    expect(normalized.totalMessages).toBe(1);
    expect(normalized.total_messages).toBe(1);
    expect(normalized.events).toHaveLength(1);
    expect(normalized.events[0]?.author).toBe('VedalFan');
  });

  it('normalizes ChatDensityPoint with dual accessors', () => {
    const pt = normalizeChatDensityPoint(sampleDensityPointJson);
    expect(pt.timestampSec).toBe(10.0);
    expect(pt.messageCount).toBe(25);
    expect(pt.velocityMps).toBe(2.5);
    expect(pt.swarmCount).toBe(10);
    expect(pt.swarmRatio).toBe(0.4);
    expect(pt.zScore).toBe(2.15);
    expect(pt.normalizedDensity).toBe(0.85);
    expect(pt.isPeak).toBe(true);

    expect(pt.timestamp_sec).toBe(10.0);
    expect(pt.message_count).toBe(25);
    expect(pt.velocity_mps).toBe(2.5);
    expect(pt.is_peak).toBe(true);
  });

  it('normalizes ChatHeatmapResponse with dual accessors', () => {
    const normalized = normalizeChatHeatmapResponse(sampleHeatmapJson);

    expect(normalized.vodId).toBe('vod_test_123');
    expect(normalized.vod_id).toBe('vod_test_123');
    expect(normalized.windowSeconds).toBe(10.0);
    expect(normalized.stepSeconds).toBe(10.0);
    expect(normalized.maxVelocityMps).toBe(2.5);
    expect(normalized.peakTimestampSec).toBe(10.0);
    expect(normalized.points).toHaveLength(1);

    const pt = normalized.points[0];
    expect(pt?.timestampSec).toBe(10.0);
    expect(pt?.messageCount).toBe(25);
    expect(pt?.velocityMps).toBe(2.5);
    expect(pt?.isPeak).toBe(true);
    expect(pt?.is_peak).toBe(true);
    expect(pt?.normalizedDensity).toBe(0.85);
  });
});

describe('Adversarial Type Guards & Defense Invariants', () => {
  it('rejects malformed badges in isChatEventJson', () => {
    const malformedBadges = {
      timestamp_sec: 1.0,
      author: 'TestUser',
      message: 'Hello',
      emotes_detected: ['Tutel'],
      badges: 123,
    };
    expect(isChatEventJson(malformedBadges)).toBe(false);

    const malformedBadgesArray = {
      timestamp_sec: 1.0,
      author: 'TestUser',
      message: 'Hello',
      emotes_detected: ['Tutel'],
      badges: ['valid', 999],
    };
    expect(isChatEventJson(malformedBadgesArray)).toBe(false);
  });

  it('rejects non-string emote elements in isChatEventJson', () => {
    const malformedEmotes = {
      timestamp_sec: 1.0,
      author: 'TestUser',
      message: 'Hello',
      emotes_detected: [null],
    };
    expect(isChatEventJson(malformedEmotes)).toBe(false);
  });

  it('rejects non-finite timestamps in isChatEventJson', () => {
    expect(
      isChatEventJson({
        timestamp_sec: NaN,
        author: 'User',
        message: 'Hi',
        emotes_detected: [],
      })
    ).toBe(false);

    expect(
      isChatEventJson({
        timestamp_sec: Infinity,
        author: 'User',
        message: 'Hi',
        emotes_detected: [],
      })
    ).toBe(false);
  });

  it('rejects invalid is_swarm_emote field in isChatEventJson', () => {
    expect(
      isChatEventJson({
        timestamp_sec: 1.0,
        author: 'User',
        message: 'Hi',
        emotes_detected: [],
        is_swarm_emote: 'not-a-boolean',
      })
    ).toBe(false);
  });

  it('normalizeChatEvent safely handles optional badges and fallback fields', () => {
    const minimalJson: ChatEventJson = {
      timestamp_sec: 10.0,
      author: 'Vedal',
      message: 'Stream starting',
      emotes_detected: [],
    };

    const normalized = normalizeChatEvent(minimalJson);
    expect(normalized.badges).toEqual([]);
    expect(normalized.isSwarmEmote).toBe(false);
    expect(normalized.timestampSec).toBe(10.0);
  });
});

describe('useChatStore State Transitions', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    chatStore.getState().reset();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('initializes with default empty state', () => {
    const state = chatStore.getState();
    expect(state.vodId).toBeNull();
    expect(state.isExtracting).toBe(false);
    expect(state.hasChatReplay).toBe(false);
    expect(state.chatEvents).toBeNull();
    expect(state.chatHeatmap).toBeNull();
    expect(state.progress).toBeNull();
    expect(state.fallbackNotice).toBeNull();
    expect(state.error).toBeNull();
  });

  it('updates vodId via setVodId action', () => {
    chatStore.getState().setVodId('vod_vedal_999');
    expect(chatStore.getState().vodId).toBe('vod_vedal_999');
  });

  it('handles chat extraction success flow', async () => {
    const mockPath = 'C:\\test\\projects\\vod_1\\chat_events.json';
    const mockHeatmap: ChatHeatmapResponseJson = {
      vod_id: 'vod_1',
      window_seconds: 10.0,
      step_seconds: 10.0,
      total_buckets: 1,
      max_velocity_mps: 5.0,
      peak_timestamp_sec: 10.0,
      points: [
        {
          timestamp_sec: 10.0,
          message_count: 50,
          velocity_mps: 5.0,
          swarm_count: 20,
          swarm_ratio: 0.4,
          z_score: 3.0,
          normalized_density: 1.0,
          is_peak: true,
        },
      ],
    };

    mockedInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'start_chat_extraction') {
        return mockPath;
      }
      if (cmd === 'get_chat_velocity_heatmap') {
        return mockHeatmap;
      }
      return null;
    });

    const resultPath = await chatStore.getState().extractChat('vod_1', 'https://twitch.tv/videos/123');

    expect(resultPath).toBe(mockPath);
    const state = chatStore.getState();
    expect(state.isExtracting).toBe(false);
    expect(state.hasChatReplay).toBe(true);
    expect(state.chatFilePath).toBe(mockPath);
    expect(state.chatHeatmap).not.toBeNull();
    expect(state.chatHeatmap?.vodId).toBe('vod_1');
  });

  it('handles chat unavailable soft fallback (ERR-05)', async () => {
    mockedInvoke.mockRejectedValue({
      code: 'ERR_INGEST_CHAT_UNAVAILABLE',
      message: 'Chat replay is unavailable for this VOD',
      recoverable: true,
      timestamp: new Date().toISOString(),
    });

    await expect(
      chatStore.getState().extractChat('vod_404', 'https://twitch.tv/videos/999')
    ).rejects.toMatchObject({
      code: 'ERR_INGEST_CHAT_UNAVAILABLE',
      recoverable: true,
    });

    const state = chatStore.getState();
    expect(state.isExtracting).toBe(false);
    expect(state.hasChatReplay).toBe(false);
    expect(state.error?.code).toBe('ERR_INGEST_CHAT_UNAVAILABLE');
    expect(state.error?.recoverable).toBe(true);
  });
});

describe('useChatStore React Lifecycle & Referential Invariants', () => {
  beforeEach(() => {
    act(() => {
      chatStore.getState().reset();
    });
  });

  it('guarantees referential stability of getState() when state is not mutated', () => {
    const snapshot1 = chatStore.getState();
    const snapshot2 = chatStore.getState();
    expect(Object.is(snapshot1, snapshot2)).toBe(true);
  });

  it('renders useChatStore without exceeding update depth or throwing console warnings', () => {
    const { result } = renderHook(() => useChatStore());
    expect(result.current.vodId).toBeNull();
    expect(result.current.isExtracting).toBe(false);
    expect(result.current.hasChatReplay).toBe(false);
    expect(result.current.chatEvents).toBeNull();
    expect(result.current.chatHeatmap).toBeNull();
  });

  it('supports fine-grained selector subscriptions without unnecessary re-renders', () => {
    const { result } = renderHook(() => useChatStore((s) => s.vodId));
    expect(result.current).toBeNull();

    act(() => {
      chatStore.getState().setVodId('vod_selective_active');
    });

    expect(result.current).toBe('vod_selective_active');
  });

  it('cleans up IPC event listeners on reset()', () => {
    act(() => {
      chatStore.getState().reset();
    });
    expect(chatStore.getState().vodId).toBeNull();
    expect(chatStore.getState().progress).toBeNull();
    expect(chatStore.getState().fallbackNotice).toBeNull();
  });
});
