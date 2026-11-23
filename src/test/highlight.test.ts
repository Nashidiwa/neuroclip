/**
 * Vitest Test Suite for TASK-M4-12:
 * Multimodal Fusion, Highlight Clustering & HighlightViewer Contracts
 *
 * Complies with docs/CONVENTIONS.md (§7 Testing & QA) and TASK-M4-12 QA checklist.
 */

import React from 'react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, renderHook, act } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import {
  isHighlightClusterJson,
  isHighlightCluster,
  isHighlightClustersContainerJson,
  isHighlightClustersContainer,
  normalizeHighlightCluster,
  normalizeHighlightClustersContainer,
  toHighlightClustersContainerJson,
  calculateMultimodalScore,
  calculateAudioOnlyScore,
  computeHighlightBounds,
  formatTimecode,
  formatScore,
  formatDuration,
  type HighlightCluster,
  type HighlightClusterJson,
  type HighlightClustersContainerJson,
} from '../types/highlight.ts';
import { useHighlightStore, highlightStore } from '../stores/useHighlightStore.ts';
import { ERROR_CODES } from '../services/ipc/error.ts';
import { HighlightViewer } from '../components/highlight/HighlightViewer.tsx';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const mockedInvoke = vi.mocked(invoke);

describe('TASK-M4-12: Schema 5.4 Highlight Contracts & Type Guards', () => {
  const sampleClusterJson: HighlightClusterJson = {
    cluster_id: 'cluster_1',
    start_sec: 5055.5,
    end_sec: 5085.0,
    peak_sec: 5070.0,
    score: 89.0,
    primary_reason: 'Văn hóa Swarm bùng nổ (tutel)',
    chat_z_score: 5.2,
    audio_rms_peak: 0.65,
    top_emotes: ['tutel', 'KEKW'],
    is_bookmarked: false,
  };

  const sampleContainerJson: HighlightClustersContainerJson = {
    vod_id: 'vod_m4_12_test',
    clusters: [sampleClusterJson],
    total_clusters: 1,
    generated_at: '2026-11-15T06:00:00Z',
  };

  it('validates valid HighlightClusterJson against Schema 5.4 type guard', () => {
    expect(isHighlightClusterJson(sampleClusterJson)).toBe(true);

    // Audio-only variant with optional null fields
    const audioOnlyCluster: HighlightClusterJson = {
      cluster_id: 'cluster_2',
      start_sec: 100.0,
      end_sec: 125.0,
      peak_sec: 110.0,
      score: 75.0,
      primary_reason: 'Đỉnh năng lượng âm thanh (Audio Energy Peak)',
      chat_z_score: null,
      audio_rms_peak: 0.75,
      top_emotes: [],
      is_bookmarked: true,
    };
    expect(isHighlightClusterJson(audioOnlyCluster)).toBe(true);
  });

  it('rejects invalid HighlightClusterJson objects', () => {
    expect(isHighlightClusterJson(null)).toBe(false);
    expect(isHighlightClusterJson({})).toBe(false);
    expect(
      isHighlightClusterJson({
        ...sampleClusterJson,
        start_sec: -5.0, // negative timecode violates invariant
      }),
    ).toBe(false);
    expect(
      isHighlightClusterJson({
        ...sampleClusterJson,
        end_sec: 5000.0, // end < start violates invariant
      }),
    ).toBe(false);
    expect(
      isHighlightClusterJson({
        ...sampleClusterJson,
        score: 150.0, // score > 100 violates Schema 5.4
      }),
    ).toBe(false);
  });

  it('normalizes wire JSON to domain model with dual accessors', () => {
    const domain = normalizeHighlightCluster(sampleClusterJson);
    expect(isHighlightCluster(domain)).toBe(true);

    // CamelCase accessors
    expect(domain.clusterId).toBe('cluster_1');
    expect(domain.startSec).toBe(5055.5);
    expect(domain.endSec).toBe(5085.0);
    expect(domain.peakSec).toBe(5070.0);
    expect(domain.score).toBe(89.0);
    expect(domain.primaryReason).toBe('Văn hóa Swarm bùng nổ (tutel)');
    expect(domain.topEmotes).toEqual(['tutel', 'KEKW']);
    expect(domain.isBookmarked).toBe(false);

    // Snake_case wire aliases
    expect(domain.cluster_id).toBe('cluster_1');
    expect(domain.start_sec).toBe(5055.5);
    expect(domain.end_sec).toBe(5085.0);
    expect(domain.peak_sec).toBe(5070.0);
    expect(domain.primary_reason).toBe('Văn hóa Swarm bùng nổ (tutel)');
    expect(domain.top_emotes).toEqual(['tutel', 'KEKW']);
  });

  it('performs round-trip serialization between container and JSON', () => {
    expect(isHighlightClustersContainerJson(sampleContainerJson)).toBe(true);
    const container = normalizeHighlightClustersContainer(sampleContainerJson);
    expect(isHighlightClustersContainer(container)).toBe(true);
    expect(container.vodId).toBe('vod_m4_12_test');
    expect(container.clusters.length).toBe(1);

    const wireJson = toHighlightClustersContainerJson(container);
    expect(wireJson.vod_id).toBe('vod_m4_12_test');
    expect(wireJson.clusters[0]?.cluster_id).toBe('cluster_1');
    expect(wireJson.clusters[0]?.score).toBe(89.0);
  });
});

describe('TASK-M4-12: Mathematical Formulas & Latency Offsets', () => {
  it('UT-12.1: calculates Full Multimodal Weighting Score: Score = 0.6*Z + 0.25*E + 0.15*RMS', () => {
    // Z_adj = 10.0 -> Z_norm = 1.0, E = 0.8, RMS = 0.6
    // 0.6(1.0) + 0.25(0.8) + 0.15(0.6) = 0.60 + 0.20 + 0.09 = 0.89 -> 89.0
    const score = calculateMultimodalScore(1.0, 0.8, 0.6);
    expect(score).toBeCloseTo(89.0, 1);
  });

  it('UT-12.2: calculates Audio-Only Fallback Score: Score = 1.0 * RMS', () => {
    const score = calculateAudioOnlyScore(0.75);
    expect(score).toBeCloseTo(75.0, 1);

    const silenceScore = calculateAudioOnlyScore(0.0);
    expect(silenceScore).toBe(0.0);
  });

  it('UT-12.3: applies 4.5s latency compensation & buffers: peak=5070s -> start=5055.5s, end=5085s', () => {
    // Total pre-offset = 4.5s latency + 10.0s pre-roll = 14.5s
    // start = 5070.0 - 14.5 = 5055.5s, end = 5070.0 + 15.0 = 5085.0s, length = 29.5s
    const bounds = computeHighlightBounds(5070.0, 7200.0);
    expect(bounds.startSec).toBeCloseTo(5055.5, 1);
    expect(bounds.endSec).toBeCloseTo(5085.0, 1);
    expect(bounds.durationSec).toBeCloseTo(29.5, 1);
  });

  it('ED-12.1: clamps negative start timecode to 0.0s without negative values', () => {
    // Peak at 8.0s -> 8.0 - 14.5 = -6.5s -> clamped to 0.0s
    const bounds = computeHighlightBounds(8.0, 3600.0);
    expect(bounds.startSec).toBe(0.0);
    expect(bounds.endSec).toBeCloseTo(23.0, 1);
    expect(bounds.durationSec).toBeCloseTo(23.0, 1);
  });

  it('ED-12.3: applies Dynamic Padding near EOF to ensure duration >= 15.0s', () => {
    // Peak at 9992.0s on 10000.0s VOD:
    // end = 10000.0s, start = 9977.5s, duration = 22.5s >= 15.0s
    const bounds = computeHighlightBounds(9992.0, 10000.0);
    expect(bounds.endSec).toBe(10000.0);
    expect(bounds.startSec).toBeCloseTo(9977.5, 1);
    expect(bounds.durationSec).toBeGreaterThanOrEqual(15.0);

    // Audio-only peak at 9998.0s on 10000.0s VOD (pre-roll 10s, post-roll 15s)
    // Normally [9988, 10000] = 12s < 15s -> dynamic padding expands pre-roll to guarantee length >= 15.0s
    const boundsNear = computeHighlightBounds(9998.0, 10000.0, { isAudioOnly: true });
    expect(boundsNear.endSec).toBe(10000.0);
    expect(boundsNear.durationSec).toBeGreaterThanOrEqual(15.0);
    expect(boundsNear.startSec).toBeLessThanOrEqual(boundsNear.endSec - 15.0);
  });

  it('formats timecodes, duration, and scores cleanly', () => {
    expect(formatTimecode(0)).toBe('00:00');
    expect(formatTimecode(75)).toBe('01:15');
    expect(formatTimecode(3665)).toBe('01:01:05');
    expect(formatTimecode(5070, { forceHours: true })).toBe('01:24:30');

    expect(formatScore(89.234)).toBe('89.2');
    expect(formatScore(100)).toBe('100.0');
    expect(formatScore(0)).toBe('0.0');

    expect(formatDuration(22.5)).toBe('22.5s');
    expect(formatDuration(125)).toBe('2m 5s');
  });
});

describe('TASK-M4-12: Reactive Highlight Store', () => {
  beforeEach(() => {
    act(() => {
      highlightStore.setState({
        vodId: null,
        isLoading: false,
        clustersContainer: null,
        clusters: [],
        selectedClusterId: null,
        error: null,
      });
    });
    vi.clearAllMocks();
  });

  it('detects highlights for VOD and updates store state', async () => {
    const mockContainer: HighlightClustersContainerJson = {
      vod_id: 'vod_test_run',
      clusters: [
        {
          cluster_id: 'cluster_1',
          start_sec: 100.0,
          end_sec: 130.0,
          peak_sec: 114.5,
          score: 85.0,
          primary_reason: 'Spam KEKW / LUL bùng nổ',
          top_emotes: ['KEKW'],
          is_bookmarked: false,
        },
      ],
    };

    mockedInvoke.mockResolvedValueOnce(mockContainer);

    const { result } = renderHook(() => useHighlightStore());

    await act(async () => {
      await result.current.detectHighlightsForVod('vod_test_run', 3600.0);
    });

    expect(result.current.vodId).toBe('vod_test_run');
    expect(result.current.isLoading).toBe(false);
    expect(result.current.clusters.length).toBe(1);
    expect(result.current.clusters[0]?.clusterId).toBe('cluster_1');
    expect(result.current.clusters[0]?.score).toBe(85.0);
  });

  it('toggles bookmark on a cluster and updates state', async () => {
    const initialCluster: HighlightCluster = normalizeHighlightCluster({
      cluster_id: 'cluster_1',
      start_sec: 100.0,
      end_sec: 130.0,
      peak_sec: 114.5,
      score: 85.0,
      primary_reason: 'Spam KEKW / LUL bùng nổ',
      top_emotes: ['KEKW'],
      is_bookmarked: false,
    });

    act(() => {
      highlightStore.setState({
        vodId: 'vod_test_run',
        clusters: [initialCluster],
        clustersContainer: {
          vodId: 'vod_test_run',
          clusters: [initialCluster],
          vod_id: 'vod_test_run',
        },
      });
    });

    const updatedClusterJson: HighlightClusterJson = {
      cluster_id: 'cluster_1',
      start_sec: 100.0,
      end_sec: 130.0,
      peak_sec: 114.5,
      score: 85.0,
      primary_reason: 'Spam KEKW / LUL bùng nổ',
      top_emotes: ['KEKW'],
      is_bookmarked: true,
    };

    mockedInvoke.mockResolvedValueOnce(updatedClusterJson);

    const { result } = renderHook(() => useHighlightStore());

    await act(async () => {
      await result.current.toggleBookmark('cluster_1', true);
    });

    expect(result.current.clusters[0]?.isBookmarked).toBe(true);
  });

  it('rejects toggleBookmark with ERROR_CODES.INTERNAL_ERROR when vodId is null', async () => {
    const { result } = renderHook(() => useHighlightStore());
    expect(result.current.vodId).toBeNull();

    await act(async () => {
      await expect(result.current.toggleBookmark('cluster_1', true)).rejects.toMatchObject({
        code: ERROR_CODES.INTERNAL_ERROR,
        recoverable: false,
      });
    });

    expect(result.current.error?.code).toBe(ERROR_CODES.INTERNAL_ERROR);
    expect(result.current.error?.recoverable).toBe(false);
    expect(mockedInvoke).not.toHaveBeenCalled();
  });

  it('rejects toggleBookmark with ERROR_CODES.INTERNAL_ERROR when vodId is whitespace-only without invoking IPC', async () => {
    act(() => {
      highlightStore.setState({ vodId: '   ' });
    });

    const { result } = renderHook(() => useHighlightStore());
    expect(result.current.vodId).toBe('   ');

    await act(async () => {
      await expect(result.current.toggleBookmark('cluster_1', true)).rejects.toMatchObject({
        code: ERROR_CODES.INTERNAL_ERROR,
        recoverable: false,
      });
    });

    expect(result.current.error?.code).toBe(ERROR_CODES.INTERNAL_ERROR);
    expect(result.current.error?.recoverable).toBe(false);
    expect(mockedInvoke).toHaveBeenCalledTimes(0);
  });

  it('merges concurrent toggleBookmark calls on different clusters without clobbering', async () => {
    const cluster1 = normalizeHighlightCluster({
      cluster_id: 'cluster_1',
      start_sec: 10.0,
      end_sec: 40.0,
      peak_sec: 25.0,
      score: 90.0,
      primary_reason: 'Peak 1',
      top_emotes: ['KEKW'],
      is_bookmarked: false,
    });
    const cluster2 = normalizeHighlightCluster({
      cluster_id: 'cluster_2',
      start_sec: 60.0,
      end_sec: 90.0,
      peak_sec: 75.0,
      score: 82.0,
      primary_reason: 'Peak 2',
      top_emotes: ['LUL'],
      is_bookmarked: false,
    });

    act(() => {
      highlightStore.setState({
        vodId: 'vod_concurrent_bm',
        clusters: [cluster1, cluster2],
        clustersContainer: {
          vodId: 'vod_concurrent_bm',
          clusters: [cluster1, cluster2],
          vod_id: 'vod_concurrent_bm',
        },
      });
    });

    let resolveFirst!: (val: HighlightClusterJson) => void;
    let resolveSecond!: (val: HighlightClusterJson) => void;
    mockedInvoke
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveFirst = resolve;
          }),
      )
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveSecond = resolve;
          }),
      );

    const p1 = useHighlightStore.getState().toggleBookmark('cluster_1', true);
    const p2 = useHighlightStore.getState().toggleBookmark('cluster_2', true);

    await act(async () => {
      resolveFirst({
        cluster_id: 'cluster_1',
        start_sec: 10.0,
        end_sec: 40.0,
        peak_sec: 25.0,
        score: 90.0,
        primary_reason: 'Peak 1',
        top_emotes: ['KEKW'],
        is_bookmarked: true,
      });
      await p1;

      resolveSecond({
        cluster_id: 'cluster_2',
        start_sec: 60.0,
        end_sec: 90.0,
        peak_sec: 75.0,
        score: 82.0,
        primary_reason: 'Peak 2',
        top_emotes: ['LUL'],
        is_bookmarked: true,
      });
      await p2;
    });

    const finalClusters = useHighlightStore.getState().clusters;
    expect(finalClusters[0]?.isBookmarked).toBe(true);
    expect(finalClusters[1]?.isBookmarked).toBe(true);
  });

  it('discards out-of-order detectHighlightsForVod responses across VOD switches', async () => {
    let resolveOld!: (val: HighlightClustersContainerJson) => void;
    let resolveNew!: (val: HighlightClustersContainerJson) => void;

    mockedInvoke
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveOld = resolve;
          }),
      )
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveNew = resolve;
          }),
      );

    const pOld = useHighlightStore.getState().detectHighlightsForVod('vod_old', 100);
    const pNew = useHighlightStore.getState().detectHighlightsForVod('vod_new', 200);

    await act(async () => {
      resolveNew({
        vod_id: 'vod_new',
        clusters: [
          {
            cluster_id: 'c_new',
            start_sec: 20,
            end_sec: 45,
            peak_sec: 30,
            score: 88,
            primary_reason: 'New Peak',
            top_emotes: ['NeuroHeart'],
            is_bookmarked: false,
          },
        ],
      });
      await pNew;

      resolveOld({
        vod_id: 'vod_old',
        clusters: [
          {
            cluster_id: 'c_old',
            start_sec: 5,
            end_sec: 25,
            peak_sec: 15,
            score: 60,
            primary_reason: 'Old Stale Peak',
            top_emotes: [],
            is_bookmarked: false,
          },
        ],
      });
      await pOld;
    });

    const state = useHighlightStore.getState();
    expect(state.vodId).toBe('vod_new');
    expect(state.clusters.map((c) => c.clusterId)).toEqual(['c_new']);
  });

  it('resets isLoading on setVodId switch so superseded detectHighlightsForVod or fetchClustersForVod cannot strand isLoading', async () => {
    let resolveDetect!: (val: HighlightClustersContainerJson) => void;
    let rejectFetch!: (err: unknown) => void;

    mockedInvoke
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveDetect = resolve;
          }),
      )
      .mockImplementationOnce(
        () =>
          new Promise((_resolve, reject) => {
            rejectFetch = reject;
          }),
      );

    const pDetect = useHighlightStore.getState().detectHighlightsForVod('vod_1', 300);
    expect(useHighlightStore.getState().isLoading).toBe(true);

    act(() => {
      useHighlightStore.getState().setVodId('vod_2');
    });
    expect(useHighlightStore.getState().vodId).toBe('vod_2');
    expect(useHighlightStore.getState().isLoading).toBe(false);

    await act(async () => {
      resolveDetect({
        vod_id: 'vod_1',
        clusters: [
          {
            cluster_id: 'c_stale',
            start_sec: 10,
            end_sec: 30,
            peak_sec: 20,
            score: 80,
            primary_reason: 'Stale',
            top_emotes: [],
            is_bookmarked: false,
          },
        ],
      });
      await pDetect;
    });

    expect(useHighlightStore.getState().vodId).toBe('vod_2');
    expect(useHighlightStore.getState().isLoading).toBe(false);
    expect(useHighlightStore.getState().clusters).toEqual([]);

    const pFetch = useHighlightStore.getState().fetchClustersForVod('vod_2');
    expect(useHighlightStore.getState().isLoading).toBe(true);

    act(() => {
      useHighlightStore.getState().setVodId('vod_3');
    });
    expect(useHighlightStore.getState().isLoading).toBe(false);

    await act(async () => {
      rejectFetch({
        code: ERROR_CODES.INTERNAL_ERROR,
        message: 'Stale fetch failure',
        recoverable: true,
      });
      await expect(pFetch).rejects.toMatchObject({ code: ERROR_CODES.INTERNAL_ERROR });
    });

    expect(useHighlightStore.getState().vodId).toBe('vod_3');
    expect(useHighlightStore.getState().isLoading).toBe(false);
    expect(useHighlightStore.getState().error).toBeNull();
  });
});

describe('TASK-M4-12: HighlightViewer Component Rendering & Interactions', () => {
  const sampleClusters: HighlightCluster[] = [
    normalizeHighlightCluster({
      cluster_id: 'cluster_1',
      start_sec: 5055.5,
      end_sec: 5085.0,
      peak_sec: 5070.0,
      score: 89.0,
      primary_reason: 'Văn hóa Swarm bùng nổ (tutel)',
      top_emotes: ['tutel', 'KEKW'],
      is_bookmarked: false,
    }),
    normalizeHighlightCluster({
      cluster_id: 'cluster_2',
      start_sec: 100.0,
      end_sec: 125.0,
      peak_sec: 110.0,
      score: 75.0,
      primary_reason: 'Vedal hét to / Đỉnh năng lượng âm thanh',
      top_emotes: ['Aware'],
      is_bookmarked: true,
    }),
  ];

  it('renders clusters list with formatted timecodes, score badges, and emotes', () => {
    render(
      React.createElement(HighlightViewer, {
        vodId: 'vod_viewer_test',
        clusters: sampleClusters,
      }),
    );

    // Title and cluster count
    expect(screen.getByText('Khoảnh Khắc Highlight Đa Phương Thức')).toBeInTheDocument();
    expect(screen.getByText('2 cụm')).toBeInTheDocument();

    // Timecodes
    expect(screen.getByText(/01:24:15 → 01:24:45/)).toBeInTheDocument();
    expect(screen.getByText(/01:40 → 02:05/)).toBeInTheDocument();

    // Scores
    expect(screen.getByText('89.0')).toBeInTheDocument();
    expect(screen.getByText('75.0')).toBeInTheDocument();

    // Primary reasons & emotes
    expect(screen.getByText('Văn hóa Swarm bùng nổ (tutel)')).toBeInTheDocument();
    expect(screen.getByText('tutel')).toBeInTheDocument();
    expect(screen.getByText('KEKW')).toBeInTheDocument();
    expect(screen.getByText('Aware')).toBeInTheDocument();
  });

  it('filters clusters by search query in real time', () => {
    render(
      React.createElement(HighlightViewer, {
        vodId: 'vod_viewer_test',
        clusters: sampleClusters,
      }),
    );

    const searchInput = screen.getByLabelText('Lọc emote hoặc lý do highlight');
    fireEvent.change(searchInput, { target: { value: 'Aware' } });

    // Cluster 2 contains Aware -> visible
    expect(screen.getByText('Vedal hét to / Đỉnh năng lượng âm thanh')).toBeInTheDocument();
    // Cluster 1 doesn't -> not visible
    expect(screen.queryByText('Văn hóa Swarm bùng nổ (tutel)')).not.toBeInTheDocument();
  });

  it('filters bookmarked clusters when bookmark filter toggle is clicked', () => {
    render(
      React.createElement(HighlightViewer, {
        vodId: 'vod_viewer_test',
        clusters: sampleClusters,
      }),
    );

    const bookmarkToggleBtn = screen.getByTitle('Chỉ hiển thị cụm đã đánh dấu');
    fireEvent.click(bookmarkToggleBtn);

    // Only Cluster 2 is bookmarked -> visible
    expect(screen.getByText('Vedal hét to / Đỉnh năng lượng âm thanh')).toBeInTheDocument();
    expect(screen.queryByText('Văn hóa Swarm bùng nổ (tutel)')).not.toBeInTheDocument();
  });

  it('calls onSelectCluster when cluster card or cut button is clicked', () => {
    const handleSelect = vi.fn();
    render(
      React.createElement(HighlightViewer, {
        vodId: 'vod_viewer_test',
        clusters: sampleClusters,
        onSelectCluster: handleSelect,
      }),
    );

    const card = screen.getByTestId('cluster-item-cluster_1');
    fireEvent.click(card);
    expect(handleSelect).toHaveBeenCalledWith(sampleClusters[0]);
  });

  it('calls onToggleBookmark when bookmark icon button is clicked', () => {
    const handleToggleBookmark = vi.fn();
    render(
      React.createElement(HighlightViewer, {
        vodId: 'vod_viewer_test',
        clusters: sampleClusters,
        onToggleBookmark: handleToggleBookmark,
      }),
    );

    const bookmarkBtn = screen.getByLabelText('Bookmark cluster cluster_1');
    fireEvent.click(bookmarkBtn);
    expect(handleToggleBookmark).toHaveBeenCalledWith('cluster_1', true);
  });

  it('renders loading skeleton when isLoading is true', () => {
    render(
      React.createElement(HighlightViewer, {
        vodId: 'vod_viewer_test',
        clusters: [],
        isLoading: true,
      }),
    );

    expect(screen.getByTestId('highlight-loading-skeleton')).toBeInTheDocument();
  });
});
