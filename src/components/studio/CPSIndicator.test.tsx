/**
 * NeuroClip Subtitle Studio - CPS Indicator & Time Borrowing Test Suite
 *
 * Verifies TASK-M4-23 specifications and QA Checklist from docs/TASK_BOARD.md:
 * - UT-23.1: Badge Color Logic (Green < 17, Yellow 17..20, Pulsating Red > 20)
 * - IT-23.1: 1-Touch Silence Borrowing (Extends end_sec, lowers CPS, clears red warning)
 * - ED-23.1: Boundary & Realtime Limits (<= 16ms typing update, Unicode diacritics,
 *            collision safety gap >= 50ms, duration clamp >= 0.05s)
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { render, screen, fireEvent, renderHook, act } from '@testing-library/react';
import { CPSIndicator } from './CPSIndicator.tsx';
import { SegmentRow } from './SegmentRow.tsx';
import { useStudioStore } from '../../stores/useStudioStore.ts';
import { useStudioHotkeys } from '../../hooks/useStudioHotkeys.ts';
import type { TranscriptSegment, SpeakerRole } from '../../types/transcript.ts';
import { normalizeTranscriptSegment } from '../../types/transcript.ts';
import {
  calculateCps,
  rateCps,
  getUnicodeCharCount,
  evaluateSegmentCps,
  calculateBorrowDuration,
  CPS_GREEN_THRESHOLD,
  CPS_RED_THRESHOLD,
  DEFAULT_MAX_BORROW_MS,
  MIN_SEGMENT_DURATION_SEC,
} from '../../utils/cps.ts';

function createSegment(
  id: string,
  start: number,
  end: number,
  textVi: string,
  textEn = '',
  speaker: SpeakerRole = 'neuro',
): TranscriptSegment {
  return normalizeTranscriptSegment({
    segment_id: id,
    start_sec: start,
    end_sec: end,
    speaker_id: speaker,
    text_en: textEn || textVi,
    text_vi: textVi,
    confidence: 0.95,
    cps_score: calculateCps(textVi, end - start),
  });
}

describe('TASK-M4-23: CPSIndicator & Manual Time Borrowing Engine', () => {
  beforeEach(() => {
    useStudioStore.getState().reset();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  // =========================================================================
  // UT-23.1: Badge Color Logic
  // =========================================================================
  describe('UT-23.1: Badge Color Logic', () => {
    it('rates CPS < 17.0 as Green ("Tốt")', () => {
      // 10 characters in 1.0s = 10.0 CPS
      const seg = createSegment('seg_green', 0, 1.0, 'Xin chào nè');
      expect(seg.cpsScore).toBeLessThan(CPS_GREEN_THRESHOLD);

      render(<CPSIndicator segment={seg} />);

      const indicator = screen.getByTestId('cps-indicator-seg_green');
      expect(indicator).toHaveAttribute('data-rating', 'green');

      const badge = screen.getByTestId('cps-badge-seg_green');
      expect(badge).toHaveTextContent(`${seg.cpsScore.toFixed(1)} CPS`);
      expect(badge).toHaveTextContent('(Tốt)');
      expect(badge.className).toContain('text-emerald-400');
      expect(badge.className).not.toContain('animate-pulse');

      // 1-touch borrow button must NOT be present
      expect(screen.queryByTestId('borrow-silence-btn-seg_green')).toBeNull();
    });

    it('rates boundary CPS = 16.99 as Green', () => {
      expect(rateCps(16.99)).toBe('green');
    });

    it('rates 17.0 <= CPS <= 20.0 as Yellow ("Nhanh")', () => {
      // 18 characters in 1.0s = 18.0 CPS
      const seg = createSegment('seg_yellow', 0, 1.0, 'Một câu nói nhanh');
      expect(seg.cpsScore).toBeGreaterThanOrEqual(CPS_GREEN_THRESHOLD);
      expect(seg.cpsScore).toBeLessThanOrEqual(CPS_RED_THRESHOLD);

      render(<CPSIndicator segment={seg} />);

      const indicator = screen.getByTestId('cps-indicator-seg_yellow');
      expect(indicator).toHaveAttribute('data-rating', 'yellow');

      const badge = screen.getByTestId('cps-badge-seg_yellow');
      expect(badge).toHaveTextContent(`${seg.cpsScore.toFixed(1)} CPS`);
      expect(badge).toHaveTextContent('(Nhanh)');
      expect(badge.className).toContain('text-amber-400');
      expect(badge.className).not.toContain('animate-pulse');

      // 1-touch borrow button must NOT be present
      expect(screen.queryByTestId('borrow-silence-btn-seg_yellow')).toBeNull();
    });

    it('rates exact boundaries 17.0 and 20.0 as Yellow', () => {
      expect(rateCps(17.0)).toBe('yellow');
      expect(rateCps(20.0)).toBe('yellow');
    });

    it('rates CPS > 20.0 as Red pulsating ("Quá nhanh!")', () => {
      // 35 characters in 1.0s = 35.0 CPS (PRD Scenario 2)
      const seg = createSegment(
        'seg_red',
        0,
        1.0,
        'Đây là một câu thoại rất dài đọc nhanh',
      );
      expect(seg.cpsScore).toBeGreaterThan(CPS_RED_THRESHOLD);

      render(<CPSIndicator segment={seg} />);

      const indicator = screen.getByTestId('cps-indicator-seg_red');
      expect(indicator).toHaveAttribute('data-rating', 'red');

      const badge = screen.getByTestId('cps-badge-seg_red');
      expect(badge).toHaveTextContent(`${seg.cpsScore.toFixed(1)} CPS`);
      expect(badge).toHaveTextContent('(Quá nhanh!)');
      expect(badge.className).toContain('text-rose-300');
      expect(badge.className).toContain('animate-pulse');

      // 1-touch borrow button MUST be visible
      const borrowBtn = screen.getByTestId('borrow-silence-btn-seg_red');
      expect(borrowBtn).toBeInTheDocument();
      expect(borrowBtn).toHaveTextContent('Nới thời gian');
    });

    it('rates boundary CPS = 20.01 as Red', () => {
      expect(rateCps(20.01)).toBe('red');
    });
  });

  // =========================================================================
  // IT-23.1: 1-Touch Silence Borrowing Integration
  // =========================================================================
  describe('IT-23.1: 1-Touch Silence Borrowing Integration', () => {
    it('calls onBorrowSilence with stopPropagation when borrow button clicked', () => {
      // Current: 0.0 - 1.0s (35 chars -> 35 CPS)
      // Next: 2.0 - 4.0s (1.0s silence gap)
      const seg1 = createSegment(
        'seg_1',
        0.0,
        1.0,
        'Đây là một câu thoại rất dài đọc nhanh',
      );
      const seg2 = createSegment('seg_2', 2.0, 4.0, 'Câu tiếp theo');
      const onBorrowSilenceMock = vi.fn();

      render(
        <CPSIndicator
          segment={seg1}
          nextSegment={seg2}
          onBorrowSilence={onBorrowSilenceMock}
        />,
      );

      const borrowBtn = screen.getByTestId('borrow-silence-btn-seg_1');
      expect(borrowBtn).toBeInTheDocument();
      expect(borrowBtn).not.toBeDisabled();
      expect(borrowBtn).toHaveTextContent('(+800ms)');

      // Click button and verify stopPropagation
      const clickEvent = new MouseEvent('click', { bubbles: true, cancelable: true });
      const stopPropagationSpy = vi.spyOn(clickEvent, 'stopPropagation');

      fireEvent(borrowBtn, clickEvent);

      expect(stopPropagationSpy).toHaveBeenCalled();
      expect(onBorrowSilenceMock).toHaveBeenCalledTimes(1);
      expect(onBorrowSilenceMock).toHaveBeenCalledWith('seg_1', 0.8);
    });

    it('extends end_sec, lowers CPS to safe range, and clears red warning in useStudioStore', () => {
      // 35 characters in 1.0s (CPS = 35.0) per PRD US5.2 Scenario 2.
      // Next segment starts at 2.0s (silence gap = 1.0s).
      const text35 = 'Câu thoại dài ba mươi lăm ký tự này';
      expect(getUnicodeCharCount(text35)).toBe(35);

      const seg1 = createSegment(
        'seg_1',
        0.0,
        1.0,
        text35,
      );
      const seg2 = createSegment('seg_2', 2.0, 4.0, 'Câu tiếp theo');

      useStudioStore.getState().setSegments([seg1, seg2]);

      // Verify initial state
      let currentSeg1 = useStudioStore.getState().segments[0]!;
      expect(currentSeg1.cpsScore).toBe(35.0);
      expect(rateCps(currentSeg1.cpsScore)).toBe('red');

      // Execute 1-touch borrowSilence action
      const success = useStudioStore.getState().borrowSilence('seg_1');
      expect(success).toBe(true);

      // Verify endSec extended by 800ms
      currentSeg1 = useStudioStore.getState().segments[0]!;
      expect(currentSeg1.endSec).toBe(1.8);
      expect(currentSeg1.end_sec).toBe(1.8);

      // Verify safety gap to seg2 is preserved: 2.0 - 1.8 = 0.2s (200ms >= 50ms)
      const currentSeg2 = useStudioStore.getState().segments[1]!;
      expect(currentSeg2.startSec - currentSeg1.endSec).toBeCloseTo(0.2, 3);

      // Verify CPS lowered below 20.0 (35 / 1.8 ≈ 19.44 CPS)
      expect(currentSeg1.cpsScore).toBeLessThanOrEqual(CPS_RED_THRESHOLD);
      expect(rateCps(currentSeg1.cpsScore)).toBe('yellow');

      // Now re-render CPSIndicator with updated store segment
      const { rerender } = render(
        <CPSIndicator segment={currentSeg1} nextSegment={currentSeg2} />,
      );

      const indicator = screen.getByTestId('cps-indicator-seg_1');
      expect(indicator).toHaveAttribute('data-rating', 'yellow');

      // Red alert and borrow button must now be completely gone
      expect(screen.queryByTestId('borrow-silence-btn-seg_1')).toBeNull();

      rerender(<CPSIndicator segment={currentSeg1} nextSegment={currentSeg2} />);
      expect(screen.queryByTestId('borrow-silence-btn-seg_1')).toBeNull();
    });

    it('borrows smaller available silence gap when gap < 800ms but > 50ms', () => {
      // 30 characters in 1.0s (CPS = 30.0).
      // Next starts at 1.40s.
      // Gap is 400ms. With 50ms buffer, allowable borrow is 350ms (0.35s).
      const seg1 = createSegment('seg_1', 0.0, 1.0, 'Một câu thoại ba mươi ký tự nè');
      const seg2 = createSegment('seg_2', 1.4, 3.0, 'Tiếp theo');

      useStudioStore.getState().setSegments([seg1, seg2]);

      const success = useStudioStore.getState().borrowSilence('seg_1');
      expect(success).toBe(true);

      const updatedSeg1 = useStudioStore.getState().segments[0]!;
      expect(updatedSeg1.endSec).toBe(1.35);

      // Verify minimum safety gap of 50ms is strictly preserved: 1.40 - 1.35 = 0.05s
      expect(seg2.startSec - updatedSeg1.endSec).toBeCloseTo(0.05, 3);
    });

    it('borrows only the needed extension to reach target CPS 17.0 when less than 800ms', () => {
      // 20 characters in 0.9s -> initial CPS = 22.22 (red)
      // Desired duration for 20 chars at 17 CPS = 20 / 17 ≈ 1.17647s
      // Needed extension = 1.17647 - 0.9 = 0.27647s (276ms)
      // Silence gap is huge (3.0s), but it should only borrow ~276ms, not 800ms!
      const text20 = 'Hai mươi ký tự ở đây';
      expect(getUnicodeCharCount(text20)).toBe(20);
      const seg1 = createSegment('seg_exact', 0.0, 0.9, text20);
      const seg2 = createSegment('seg_far', 4.0, 6.0, 'Rất xa');

      useStudioStore.getState().setSegments([seg1, seg2]);

      const success = useStudioStore.getState().borrowSilence('seg_exact');
      expect(success).toBe(true);

      const updatedSeg1 = useStudioStore.getState().segments[0]!;
      expect(updatedSeg1.endSec).toBeCloseTo(1.176, 2);
      expect(updatedSeg1.cpsScore).toBeLessThanOrEqual(CPS_GREEN_THRESHOLD + 0.1);
      expect(rateCps(updatedSeg1.cpsScore)).toBe('yellow');
    });

    it('triggers borrowSilence via Shift + E hotkey on selected segment (PRD US5.2 Scenario 2)', () => {
      const seg1 = createSegment(
        'seg_hotkey',
        0.0,
        1.0,
        'Câu thoại dài ba mươi lăm ký tự này',
      );
      const seg2 = createSegment('seg_next', 3.0, 5.0, 'Đoạn tiếp theo');

      useStudioStore.getState().setSegments([seg1, seg2]);
      useStudioStore.getState().selectSegment('seg_hotkey');

      const { unmount } = renderHook(() => useStudioHotkeys());

      act(() => {
        window.dispatchEvent(
          new KeyboardEvent('keydown', {
            key: 'E',
            code: 'KeyE',
            shiftKey: true,
            bubbles: true,
            cancelable: true,
          }),
        );
      });

      const updatedSeg1 = useStudioStore.getState().segments[0]!;
      expect(updatedSeg1.endSec).toBe(1.8);
      expect(updatedSeg1.cpsScore).toBeLessThanOrEqual(CPS_RED_THRESHOLD);
      expect(rateCps(updatedSeg1.cpsScore)).toBe('yellow');

      unmount();
    });
  });

  // =========================================================================
  // ED-23.1: Boundary & Realtime Limits
  // =========================================================================
  describe('ED-23.1: Boundary & Realtime Limits', () => {
    it('recalculates CPS synchronously within 16ms during realtime textarea typing', () => {
      const seg = createSegment('seg_typing', 0.0, 2.0, 'Ngắn');
      useStudioStore.getState().setSegments([seg]);

      render(<SegmentRow segment={seg} isActive={true} isSelected={true} />);

      const textarea = screen.getByTestId('textarea-vi-seg_typing');
      expect(screen.getByTestId('cps-indicator-seg_typing')).toHaveAttribute(
        'data-rating',
        'green',
      );

      // Measure synchronous typing execution latency
      const t0 = performance.now();
      // Type a very long phrase that pushes CPS > 20
      fireEvent.change(textarea, {
        target: { value: 'Một câu thoại tiếng Việt rất dài khiến tốc độ đọc vượt ngưỡng hai mươi CPS!' },
      });
      const elapsedMs = performance.now() - t0;

      // Must execute synchronously well within 16ms frame budget (60 FPS)
      expect(elapsedMs).toBeLessThan(16);

      // Store updated synchronously
      const updated = useStudioStore.getState().segments[0]!;
      expect(updated.cpsScore).toBeGreaterThan(20);
      expect(rateCps(updated.cpsScore)).toBe('red');
    });

    it('correctly counts Unicode code points for Vietnamese accents and compound emojis', () => {
      // "bánh mì" -> 7 code points
      expect(getUnicodeCharCount('bánh mì')).toBe(7);

      // "ngây thơ chưa biết gì" -> 21 code points
      expect(getUnicodeCharCount('ngây thơ chưa biết gì')).toBe(21);

      // "bánh mì 🥖" -> 9 code points (includes space before emoji)
      expect(getUnicodeCharCount('bánh mì 🥖')).toBe(9);
      // "bánh mì🥖" -> 8 code points (no space before emoji)
      expect(getUnicodeCharCount('bánh mì🥖')).toBe(8);

      // Empty string -> 0
      expect(getUnicodeCharCount('')).toBe(0);
      expect(getUnicodeCharCount('   ')).toBe(0);
    });

    it('enforces collision safety: disables borrow button when silence gap < 50ms', () => {
      // seg1 ends at 1.0s, seg2 starts at 1.03s (gap = 30ms < 50ms min safety buffer)
      const seg1 = createSegment('seg_close_1', 0.0, 1.0, 'Câu này quá dài đọc không kịp');
      const seg2 = createSegment('seg_close_2', 1.03, 3.0, 'Câu dính liền');

      // Math helper returns 0
      const allowable = calculateBorrowDuration(seg1, seg2);
      expect(allowable).toBe(0);

      const onBorrowMock = vi.fn();
      render(
        <CPSIndicator
          segment={seg1}
          nextSegment={seg2}
          onBorrowSilence={onBorrowMock}
        />,
      );

      // Button is rendered because CPS > 20, but it is DISABLED
      const btn = screen.getByTestId('borrow-silence-btn-seg_close_1');
      expect(btn).toBeInTheDocument();
      expect(btn).toBeDisabled();
      expect(btn.className).toContain('cursor-not-allowed');
      expect(btn).toHaveAttribute(
        'title',
        'Không có khoảng lặng khả dụng để mượn thời gian',
      );

      // Clicking disabled button must NOT trigger borrow
      fireEvent.click(btn);
      expect(onBorrowMock).not.toHaveBeenCalled();

      // Store borrowSilence returns false
      useStudioStore.getState().setSegments([seg1, seg2]);
      const res = useStudioStore.getState().borrowSilence('seg_close_1');
      expect(res).toBe(false);
    });

    it('safely clamps duration >= 0.05s to prevent division by zero or NaN', () => {
      // 0 duration
      const cpsZero = calculateCps('Hello world', 0);
      expect(Number.isFinite(cpsZero)).toBe(true);
      expect(cpsZero).toBeGreaterThan(0);

      // Negative duration
      const cpsNegative = calculateCps('Test', -1.5);
      expect(Number.isFinite(cpsNegative)).toBe(true);
      expect(cpsNegative).toBeGreaterThan(0);

      // Segment evaluation with endSec <= startSec
      const zeroSeg = createSegment('seg_zero', 2.0, 2.0, 'Bất thường');
      const evalZero = evaluateSegmentCps(zeroSeg);
      expect(evalZero.durationSec).toBe(MIN_SEGMENT_DURATION_SEC);
      expect(Number.isFinite(evalZero.cps)).toBe(true);
    });

    it('handles last segment in project when nextSegment is undefined', () => {
      // Last segment in clip with CPS > 20
      const lastSeg = createSegment('last_seg', 5.0, 6.0, 'Câu thoại cuối cùng của video rất dài');

      // Without max clip duration: borrows up to DEFAULT_MAX_BORROW_MS (800ms)
      const borrowNoLimit = calculateBorrowDuration(lastSeg, undefined);
      expect(borrowNoLimit).toBe(DEFAULT_MAX_BORROW_MS / 1000);

      // With max clip duration = 6.4s (gap = 400ms to video end)
      const borrowWithClipEnd = calculateBorrowDuration(lastSeg, undefined, 6.4);
      expect(borrowWithClipEnd).toBeCloseTo(0.4, 3);
    });

    it('falls back to English transcript when Vietnamese subtitle is empty', () => {
      const segEn = normalizeTranscriptSegment({
        segment_id: 'seg_en_only',
        start_sec: 0.0,
        end_sec: 1.0,
        speaker_id: 'neuro',
        text_en: 'This is an english transcript only segment',
        text_vi: '',
        confidence: 0.9,
        cps_score: 0,
      });

      const evaluation = evaluateSegmentCps(segEn);
      expect(evaluation.charCount).toBe(
        getUnicodeCharCount('This is an english transcript only segment'),
      );
      expect(evaluation.cps).toBeGreaterThan(CPS_RED_THRESHOLD);
      expect(evaluation.rating).toBe('red');
    });
  });
});
