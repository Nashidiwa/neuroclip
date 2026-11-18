/**
 * NeuroClip Multimodal Highlight Viewer Component
 *
 * Displays ranked highlight clusters adhering to Schema 5.4.
 * Features timecodes (mm:ss), scores (0-100), primary_reason badges, top_emotes tags,
 * peak indicators with 4.5s latency offset, bookmark toggle, and selection handling.
 *
 * Strict adherence to docs/CONVENTIONS.md (zero-any, accessible UI).
 */

import React, { useState, useMemo } from 'react';
import {
  Sparkles,
  Clock,
  Flame,
  Bookmark,
  BookmarkCheck,
  Scissors,
  Search,
  ArrowUpDown,
  RefreshCw,
  Volume2,
  MessageSquare,
} from 'lucide-react';
import {
  type HighlightCluster,
  formatTimecode,
  formatScore,
  formatDuration,
} from '../../types/highlight.ts';

export interface HighlightViewerProps {
  readonly vodId?: string | undefined;
  readonly clusters: readonly HighlightCluster[];
  readonly selectedClusterId?: string | null | undefined;
  readonly onSelectCluster?: ((cluster: HighlightCluster) => void) | undefined;
  readonly onToggleBookmark?: ((clusterId: string, nextBookmarked: boolean) => void) | undefined;
  readonly onRefresh?: (() => void) | undefined;
  readonly isLoading?: boolean | undefined;
  readonly className?: string | undefined;
}

type SortField = 'score' | 'time' | 'duration';

export const HighlightViewer: React.FC<HighlightViewerProps> = ({
  vodId,
  clusters,
  selectedClusterId,
  onSelectCluster,
  onToggleBookmark,
  onRefresh,
  isLoading = false,
  className = '',
}) => {
  const [searchQuery, setSearchQuery] = useState<string>('');
  const [filterBookmarkedOnly, setFilterBookmarkedOnly] = useState<boolean>(false);
  const [sortBy, setSortBy] = useState<SortField>('score');

  // Filter & Sort clusters
  const filteredClusters = useMemo(() => {
    let result = [...clusters];

    if (filterBookmarkedOnly) {
      result = result.filter((c) => c.isBookmarked);
    }

    if (searchQuery.trim().length > 0) {
      const q = searchQuery.toLowerCase().trim();
      result = result.filter(
        (c) =>
          c.primaryReason.toLowerCase().includes(q) ||
          c.topEmotes.some((e) => e.toLowerCase().includes(q)),
      );
    }

    result.sort((a, b) => {
      if (sortBy === 'score') {
        return b.score - a.score;
      }
      if (sortBy === 'time') {
        return a.startSec - b.startSec;
      }
      if (sortBy === 'duration') {
        return b.endSec - b.startSec - (a.endSec - a.startSec);
      }
      return 0;
    });

    return result;
  }, [clusters, filterBookmarkedOnly, searchQuery, sortBy]);

  const getScoreBadgeClass = (score: number): string => {
    if (score >= 80) {
      return 'bg-accent-neuro/20 text-accent-neuro border-accent-neuro/40';
    }
    if (score >= 60) {
      return 'bg-accent-chat/20 text-accent-chat border-accent-chat/40';
    }
    return 'bg-bg-tertiary text-slate-300 border-border-dark';
  };

  const getReasonIcon = (reason: string): React.ReactElement => {
    const lower = reason.toLowerCase();
    if (lower.includes('audio') || lower.includes('rms') || lower.includes('âm thanh')) {
      return <Volume2 className="mr-1 h-3 w-3 shrink-0 text-accent-vedal" />;
    }
    if (lower.includes('chat') || lower.includes('emote') || lower.includes('kekw')) {
      return <MessageSquare className="mr-1 h-3 w-3 shrink-0 text-accent-chat" />;
    }
    return <Sparkles className="mr-1 h-3 w-3 shrink-0 text-accent-neuro" />;
  };

  return (
    <section
      role="region"
      aria-label="Khung xem highlight đa phương thức"
      className={`flex h-full flex-col overflow-hidden rounded-xl border border-border-dark bg-bg-secondary shadow-acrylic ${className}`}
    >
      {/* Header Toolbar */}
      <header className="flex flex-wrap items-center justify-between gap-3 border-b border-border-dark bg-bg-primary/50 p-4">
        <div className="flex items-center space-x-2">
          <Sparkles className="h-5 w-5 text-accent-neuro" />
          <h2 className="text-base font-semibold text-white">
            Khoảnh Khắc Highlight Đa Phương Thức
          </h2>
          <span className="rounded border border-border-dark bg-bg-tertiary px-2 py-0.5 font-mono text-xs text-slate-300">
            {clusters.length} cụm
          </span>
          {vodId && <span className="font-mono text-xs text-slate-400">({vodId})</span>}
        </div>

        {/* Controls */}
        <div className="flex items-center space-x-2">
          {/* Search Box */}
          <div className="relative">
            <Search className="pointer-events-none absolute left-2.5 top-2.5 h-3.5 w-3.5 text-slate-400" />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Lọc emote / lý do..."
              aria-label="Lọc emote hoặc lý do highlight"
              className="rounded-lg border border-border-dark bg-bg-tertiary py-1 pl-8 pr-3 text-xs text-white placeholder-slate-500 focus:border-accent-neuro focus:outline-none"
            />
          </div>

          {/* Bookmarked Filter Toggle */}
          <button
            type="button"
            onClick={() => setFilterBookmarkedOnly((prev) => !prev)}
            aria-pressed={filterBookmarkedOnly}
            title="Chỉ hiển thị cụm đã đánh dấu"
            className={`flex items-center space-x-1 rounded-lg border px-2.5 py-1 text-xs transition-colors ${
              filterBookmarkedOnly
                ? 'border-accent-guest bg-accent-guest/20 text-accent-guest'
                : 'border-border-dark bg-bg-tertiary text-slate-400 hover:text-white'
            }`}
          >
            <Bookmark className="h-3.5 w-3.5" />
            <span>Đã ghim</span>
          </button>

          {/* Sort Selector */}
          <div className="flex items-center space-x-1 rounded-lg border border-border-dark bg-bg-tertiary px-2 py-0.5 text-xs">
            <ArrowUpDown className="h-3 w-3 text-slate-400" />
            <select
              value={sortBy}
              onChange={(e) => setSortBy(e.target.value as SortField)}
              aria-label="Sắp xếp cụm highlight"
              className="cursor-pointer bg-transparent text-xs text-slate-300 focus:outline-none"
            >
              <option value="score">Điểm cao nhất</option>
              <option value="time">Mốc thời gian</option>
              <option value="duration">Thời lượng dài</option>
            </select>
          </div>

          {/* Refresh Button */}
          {onRefresh && (
            <button
              type="button"
              onClick={onRefresh}
              disabled={isLoading}
              title="Phân tích lại highlight"
              className="rounded-lg bg-bg-tertiary p-1.5 text-slate-300 transition-colors hover:bg-slate-700 disabled:opacity-50"
            >
              <RefreshCw
                className={`h-3.5 w-3.5 ${isLoading ? 'animate-spin text-accent-neuro' : ''}`}
              />
            </button>
          )}
        </div>
      </header>

      {/* Cluster List Body */}
      <div className="flex-1 space-y-3 overflow-y-auto p-4">
        {isLoading ? (
          <div className="animate-pulse space-y-3" data-testid="highlight-loading-skeleton">
            <div className="h-20 rounded-lg border border-border-dark bg-bg-tertiary" />
            <div className="h-20 rounded-lg border border-border-dark bg-bg-tertiary" />
            <div className="h-20 rounded-lg border border-border-dark bg-bg-tertiary" />
          </div>
        ) : filteredClusters.length === 0 ? (
          <div className="flex h-48 flex-col items-center justify-center space-y-2 text-center">
            <Sparkles className="h-8 w-8 text-slate-600" />
            <p className="text-sm text-slate-400">Không tìm thấy cụm highlight phù hợp.</p>
            <p className="text-xs text-slate-500">
              {clusters.length === 0
                ? 'Hãy kích hoạt phân tích đa phương thức để quét đỉnh chat và âm lượng.'
                : 'Thử xóa bộ lọc tìm kiếm để xem tất cả cụm.'}
            </p>
          </div>
        ) : (
          <ul role="list" className="space-y-2.5">
            {filteredClusters.map((cluster, index) => {
              const isSelected = selectedClusterId === cluster.clusterId;
              const durationSec = cluster.endSec - cluster.startSec;

              return (
                <li
                  key={cluster.clusterId}
                  role="listitem"
                  tabIndex={0}
                  onClick={() => onSelectCluster?.(cluster)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' || e.key === ' ') {
                      e.preventDefault();
                      onSelectCluster?.(cluster);
                    }
                  }}
                  className={`group relative cursor-pointer select-none rounded-xl border p-3.5 transition-all ${
                    isSelected
                      ? 'border-accent-neuro bg-bg-tertiary/90 shadow-lg ring-1 ring-accent-neuro'
                      : 'border-border-dark bg-bg-secondary/60 hover:border-slate-600 hover:bg-bg-tertiary/60'
                  }`}
                  data-testid={`cluster-item-${cluster.clusterId}`}
                >
                  <div className="flex items-start justify-between gap-3">
                    {/* Left: Rank, Timecodes, Peak */}
                    <div className="min-w-0 flex-1 space-y-1.5">
                      <div className="flex flex-wrap items-center gap-2 text-xs">
                        <span className="font-mono font-semibold text-slate-400">#{index + 1}</span>

                        {/* Range Timecode */}
                        <div className="flex items-center space-x-1 rounded border border-border-dark bg-bg-primary px-2 py-0.5 font-mono font-medium text-white">
                          <Clock className="h-3.5 w-3.5 text-accent-chat" />
                          <span>
                            {formatTimecode(cluster.startSec)} &rarr;{' '}
                            {formatTimecode(cluster.endSec)}
                          </span>
                        </div>

                        {/* Duration Pill */}
                        <span className="font-mono text-[11px] text-slate-400">
                          ({formatDuration(durationSec)})
                        </span>

                        {/* Peak Indicator */}
                        <div
                          className="flex items-center space-x-1 rounded border border-accent-evil/30 bg-accent-evil/10 px-2 py-0.5 font-mono text-[11px] text-accent-evil"
                          title="Đỉnh chat bù trễ -4.5s livestream"
                        >
                          <Flame className="h-3 w-3 text-accent-evil" />
                          <span>Đỉnh: {formatTimecode(cluster.peakSec)}</span>
                          <span className="text-[10px] text-slate-400">(-4.5s)</span>
                        </div>
                      </div>

                      {/* Reasons & Top Emotes */}
                      <div className="flex flex-wrap items-center gap-1.5 pt-1">
                        {/* Primary Reason Badge */}
                        <span className="inline-flex items-center rounded-md border border-border-dark bg-bg-primary px-2 py-0.5 text-xs text-slate-300">
                          {getReasonIcon(cluster.primaryReason)}
                          <span>{cluster.primaryReason}</span>
                        </span>

                        {/* Emote Badges */}
                        {cluster.topEmotes.map((emote) => (
                          <span
                            key={emote}
                            className="rounded border border-accent-chat/30 bg-accent-chat/10 px-1.5 py-0.5 font-mono text-[11px] text-accent-chat"
                          >
                            {emote}
                          </span>
                        ))}
                      </div>
                    </div>

                    {/* Right: Score Badge & Actions */}
                    <div className="flex shrink-0 flex-col items-end space-y-2">
                      <div className="flex items-center space-x-2">
                        {/* Score Badge */}
                        <div
                          className={`flex items-center rounded-lg border px-2.5 py-1 font-mono text-xs font-bold tracking-wide ${getScoreBadgeClass(
                            cluster.score,
                          )}`}
                          title={`Điểm hợp nhất đa phương thức: ${cluster.score}/100`}
                        >
                          <span>{formatScore(cluster.score)}</span>
                          <span className="ml-0.5 text-[10px] font-normal text-slate-400">
                            /100
                          </span>
                        </div>

                        {/* Bookmark Button */}
                        <button
                          type="button"
                          onClick={(e) => {
                            e.stopPropagation();
                            onToggleBookmark?.(cluster.clusterId, !cluster.isBookmarked);
                          }}
                          aria-label={`Bookmark cluster ${cluster.clusterId}`}
                          title={cluster.isBookmarked ? 'Bỏ đánh dấu' : 'Đánh dấu clip này'}
                          className={`rounded-lg border p-1.5 transition-colors ${
                            cluster.isBookmarked
                              ? 'border-accent-guest/40 bg-accent-guest/20 text-accent-guest'
                              : 'border-border-dark bg-bg-primary text-slate-400 hover:text-white'
                          }`}
                        >
                          {cluster.isBookmarked ? (
                            <BookmarkCheck className="h-3.5 w-3.5 fill-accent-guest text-accent-guest" />
                          ) : (
                            <Bookmark className="h-3.5 w-3.5" />
                          )}
                        </button>
                      </div>

                      {/* Quick Actions (Range Slicer Preparation) */}
                      <div className="flex items-center space-x-1 opacity-80 transition-opacity group-hover:opacity-100">
                        <button
                          type="button"
                          onClick={(e) => {
                            e.stopPropagation();
                            onSelectCluster?.(cluster);
                          }}
                          title="Cắt phân đoạn (TASK-M4-13 Range Slicer)"
                          className="flex items-center space-x-1 rounded border border-border-dark bg-bg-primary px-2 py-0.5 text-[11px] text-slate-300 hover:bg-slate-700"
                        >
                          <Scissors className="h-3 w-3 text-accent-vedal" />
                          <span>Cắt Clip</span>
                        </button>
                      </div>
                    </div>
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </section>
  );
};
