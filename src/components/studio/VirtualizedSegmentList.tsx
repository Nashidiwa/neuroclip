/**
 * NeuroClip Subtitle Studio - High-Performance Virtualized Subtitle Segment List
 *
 * Renders hundreds to thousands of subtitle segments at 60 FPS while keeping
 * only 15-20 DOM rows in the viewport using virtual windowing.
 *
 * Provides:
 * - Realtime active segment auto-scrolling with manual interaction detection
 * - Zero input lag inline editing
 * - Fallback empty state
 * - Adherence to docs/CONVENTIONS.md §3.5
 */

import React, { useState, useRef, useEffect, useCallback, useMemo } from 'react';
import { MessageSquare, Lock, Unlock } from 'lucide-react';
import type { TranscriptSegment } from '../../types/transcript.ts';
import { useStudioStore } from '../../stores/useStudioStore.ts';
import { SegmentRow } from './SegmentRow.tsx';

export interface VirtualItem {
  readonly index: number;
  readonly start: number;
  readonly size: number;
  readonly end: number;
  readonly key: string | number;
}

export interface VirtualizerOptions {
  readonly count: number;
  readonly getScrollElement: () => HTMLElement | null;
  readonly estimateSize: (index: number) => number;
  readonly overscan?: number;
  readonly getItemKey?: (index: number) => string | number;
}

export interface Virtualizer {
  readonly getVirtualItems: () => readonly VirtualItem[];
  readonly getTotalSize: () => number;
  readonly scrollToIndex: (
    index: number,
    options?: { align?: 'auto' | 'start' | 'center' | 'end'; behavior?: 'auto' | 'smooth' },
  ) => void;
  readonly handleScroll?: ((scrollTop: number) => void) | undefined;
}

/**
 * High-performance virtualizer hook implementing windowing algorithm
 * adhering to docs/CONVENTIONS.md §3.5 ("dùng @tanstack/react-virtual hoặc custom virtualizer").
 */
export function useVirtualizer({
  count,
  getScrollElement,
  estimateSize,
  overscan = 5,
  getItemKey,
}: VirtualizerOptions): Virtualizer {
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportHeight, setViewportHeight] = useState(800);

  const defaultItemHeight = estimateSize(0) || 80;

  // Track viewport height via ResizeObserver or clientHeight
  useEffect(() => {
    const el = getScrollElement();
    if (!el) return;

    const updateHeight = () => {
      const h = el.clientHeight;
      if (h > 0) {
        setViewportHeight(h);
      }
    };

    updateHeight();

    if (typeof window !== 'undefined' && 'ResizeObserver' in window) {
      const ro = new ResizeObserver(() => {
        updateHeight();
      });
      ro.observe(el);
      return () => ro.disconnect();
    }
    return undefined;
  }, [getScrollElement]);

  // Track scroll position
  useEffect(() => {
    const el = getScrollElement();
    if (!el) return;

    const onScroll = () => {
      setScrollTop(el.scrollTop);
    };

    el.addEventListener('scroll', onScroll, { passive: true });
    return () => el.removeEventListener('scroll', onScroll);
  }, [getScrollElement]);

  const virtualItems = useMemo<readonly VirtualItem[]>(() => {
    if (count === 0) return [];

    const effectiveHeight = viewportHeight > 0 ? viewportHeight : 800;
    const startIndex = Math.max(0, Math.floor(scrollTop / defaultItemHeight) - overscan);
    const visibleCount = Math.ceil(effectiveHeight / defaultItemHeight);
    const endIndex = Math.min(count - 1, startIndex + visibleCount + overscan * 2 - 1);

    const items: VirtualItem[] = [];
    for (let i = startIndex; i <= endIndex; i++) {
      const start = i * defaultItemHeight;
      const size = estimateSize(i);
      items.push({
        index: i,
        start,
        size,
        end: start + size,
        key: getItemKey ? getItemKey(i) : i,
      });
    }

    return items;
  }, [count, scrollTop, viewportHeight, defaultItemHeight, overscan, estimateSize, getItemKey]);

  const getTotalSize = useCallback(() => {
    return count * defaultItemHeight;
  }, [count, defaultItemHeight]);

  const scrollToIndex = useCallback(
    (
      index: number,
      options: { align?: 'auto' | 'start' | 'center' | 'end'; behavior?: 'auto' | 'smooth' } = {},
    ) => {
      const el = getScrollElement();
      if (!el || index < 0 || index >= count) return;

      const itemTop = index * defaultItemHeight;
      const itemBottom = itemTop + defaultItemHeight;
      const currentScrollTop = el.scrollTop;
      const currentHeight = el.clientHeight > 0 ? el.clientHeight : viewportHeight;
      const align = options.align ?? 'auto';
      const behavior = options.behavior ?? 'smooth';

      let targetScrollTop = currentScrollTop;

      if (align === 'start') {
        targetScrollTop = itemTop;
      } else if (align === 'end') {
        targetScrollTop = Math.max(0, itemBottom - currentHeight);
      } else if (align === 'center') {
        targetScrollTop = Math.max(0, itemTop - currentHeight / 2 + defaultItemHeight / 2);
      } else {
        // 'auto': only scroll if out of viewport
        if (itemTop < currentScrollTop) {
          targetScrollTop = itemTop;
        } else if (itemBottom > currentScrollTop + currentHeight) {
          targetScrollTop = itemBottom - currentHeight;
        }
      }

      if (Math.abs(targetScrollTop - currentScrollTop) > 1) {
        if (typeof el.scrollTo === 'function') {
          el.scrollTo({ top: targetScrollTop, behavior });
        } else {
          el.scrollTop = targetScrollTop;
        }
      }
    },
    [count, defaultItemHeight, getScrollElement, viewportHeight],
  );

  const handleScroll = useCallback((newScrollTop: number) => {
    if (Number.isFinite(newScrollTop) && newScrollTop >= 0) {
      setScrollTop(newScrollTop);
    }
  }, []);

  return {
    getVirtualItems: () => virtualItems,
    getTotalSize,
    scrollToIndex,
    handleScroll,
  };
}

export interface VirtualizedSegmentListProps {
  readonly segments?: readonly TranscriptSegment[] | undefined;
  readonly onSegmentSelect?: ((segment: TranscriptSegment) => void) | undefined;
  readonly onUpdateText?:
    | ((
        segmentId: string,
        updates: {
          text?: string;
          text_vi?: string | null;
          text_en?: string;
          textVi?: string | null;
          textEn?: string;
        },
      ) => void)
    | undefined;
  readonly onNudgeTime?:
    | ((segmentId: string, boundary: 'start' | 'end', deltaSec: number) => void)
    | undefined;
  readonly className?: string | undefined;
}

export const VirtualizedSegmentList: React.FC<VirtualizedSegmentListProps> = ({
  segments: propSegments,
  onSegmentSelect,
  onUpdateText,
  onNudgeTime,
  className = '',
}) => {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const isUserInteractingRef = useRef(false);
  const userInteractionTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastScrolledIdRef = useRef<string | null>(null);

  const [autoFollowEnabled, setAutoFollowEnabled] = useState(true);

  // Store bindings
  const storeSegments = useStudioStore((state) => state.segments);
  const activeSegmentIds = useStudioStore((state) => state.activeSegmentIds);
  const selectedSegmentId = useStudioStore((state) => state.selectedSegmentId);
  const selectSegment = useStudioStore((state) => state.selectSegment);
  const seekTo = useStudioStore((state) => state.seekTo);

  const effectiveSegments = propSegments !== undefined ? propSegments : storeSegments;

  const virtualizer = useVirtualizer({
    count: effectiveSegments.length,
    getScrollElement: () => containerRef.current,
    estimateSize: () => 80,
    overscan: 5,
    getItemKey: (index) => {
      const seg = effectiveSegments[index];
      return seg?.segmentId ?? seg?.segment_id ?? index;
    },
  });

  // User scroll interaction detection to avoid fighting manual scrolls
  const handleUserScrollInteraction = useCallback(() => {
    isUserInteractingRef.current = true;
    if (userInteractionTimerRef.current) {
      clearTimeout(userInteractionTimerRef.current);
    }
    userInteractionTimerRef.current = setTimeout(() => {
      isUserInteractingRef.current = false;
    }, 3000);
  }, []);

  // Auto-scroll to primary active segment
  useEffect(() => {
    if (!autoFollowEnabled) {
      lastScrolledIdRef.current = null;
      return;
    }

    if (isUserInteractingRef.current || activeSegmentIds.length === 0) {
      return;
    }

    const firstActiveId = activeSegmentIds[0];
    if (!firstActiveId || firstActiveId === lastScrolledIdRef.current) return;

    const activeIndex = effectiveSegments.findIndex(
      (s) => (s.segmentId ?? s.segment_id) === firstActiveId,
    );

    if (activeIndex >= 0) {
      lastScrolledIdRef.current = firstActiveId;
      virtualizer.scrollToIndex(activeIndex, { align: 'auto', behavior: 'smooth' });
    }
  }, [activeSegmentIds, autoFollowEnabled, effectiveSegments, virtualizer]);

  const handleRowClick = useCallback(
    (segment: TranscriptSegment) => {
      const segId = segment.segmentId ?? segment.segment_id;
      const start = segment.startSec ?? segment.start_sec;

      selectSegment(segId);
      seekTo(start);

      if (onSegmentSelect) {
        onSegmentSelect(segment);
      }
    },
    [onSegmentSelect, seekTo, selectSegment],
  );

  return (
    <div className={`flex flex-col h-full w-full overflow-hidden ${className}`}>
      {/* List Toolbar / Auto-Follow Status */}
      <div className="flex items-center justify-between border-b border-border-dark/60 bg-bg-secondary/90 px-3 py-1.5 text-xs text-slate-400 select-none">
        <span className="font-mono text-[11px] text-slate-400">
          Hiển thị {virtualizer.getVirtualItems().length} / {effectiveSegments.length} dòng
        </span>

        <button
          type="button"
          onClick={() => setAutoFollowEnabled((prev) => !prev)}
          className={`inline-flex items-center space-x-1 rounded px-2 py-0.5 text-[11px] font-medium transition-colors ${
            autoFollowEnabled
              ? 'text-accent-neuro bg-accent-neuro/10 hover:bg-accent-neuro/20'
              : 'text-slate-400 bg-bg-tertiary hover:text-slate-200'
          }`}
          title={
            autoFollowEnabled
              ? 'Tự động cuộn theo video đang bật'
              : 'Tự động cuộn theo video đang tắt'
          }
        >
          {autoFollowEnabled ? (
            <>
              <Lock className="h-3 w-3" />
              <span>Cuộn theo video</span>
            </>
          ) : (
            <>
              <Unlock className="h-3 w-3" />
              <span>Dừng cuộn</span>
            </>
          )}
        </button>
      </div>

      {/* Virtualized Container */}
      <div
        ref={containerRef}
        data-testid="studio-segment-list"
        onScroll={(e) => {
          virtualizer.handleScroll?.(e.currentTarget.scrollTop);
        }}
        onWheel={handleUserScrollInteraction}
        onTouchMove={handleUserScrollInteraction}
        className="flex-1 overflow-y-auto p-2"
        style={{ contain: 'strict' }}
      >
        {effectiveSegments.length === 0 ? (
          <div
            data-testid="studio-empty-segments"
            className="flex h-full flex-col items-center justify-center space-y-2 p-8 text-center text-slate-400"
          >
            <MessageSquare className="h-8 w-8 text-slate-500/70" />
            <p className="text-sm font-medium">Chưa có phân đoạn phụ đề</p>
            <p className="text-xs text-slate-500 max-w-[220px]">
              Nạp tệp transcript_segments.json hoặc chạy Whisper STT để tạo phụ đề
            </p>
          </div>
        ) : (
          <div
            style={{
              height: `${virtualizer.getTotalSize()}px`,
              width: '100%',
              position: 'relative',
            }}
          >
            {virtualizer.getVirtualItems().map((virtualRow) => {
              const segment = effectiveSegments[virtualRow.index];
              if (!segment) return null;

              const segId = segment.segmentId ?? segment.segment_id;
              const isActive = activeSegmentIds.includes(segId);
              const isSelected = selectedSegmentId === segId;

              return (
                <div
                  key={virtualRow.key}
                  data-index={virtualRow.index}
                  style={{
                    position: 'absolute',
                    top: 0,
                    left: 0,
                    width: '100%',
                    transform: `translateY(${virtualRow.start}px)`,
                  }}
                >
                  <SegmentRow
                    segment={segment}
                    isActive={isActive}
                    isSelected={isSelected}
                    onSelect={handleRowClick}
                    onUpdateText={onUpdateText}
                    onNudgeTime={onNudgeTime}
                  />
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
};
