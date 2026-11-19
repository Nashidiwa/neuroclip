/**
 * Subtitle Studio Global Hotkeys & Multi-Selection Test Suite
 *
 * Covers 100% of Acceptance Criteria and QA Test Checklists for TASK-M4-22:
 * - AC-22.1: Hotkey 1-5 character reassignment completes in <= 50ms.
 * - AC-22.2: Batch reassignment succeeds for >= 20 rows simultaneously without state corruption.
 * - UT-22.1: Key '1' assigns speaker_id = 'vedal', text style green #98FB98.
 * - UT-22.2: Key '2' assigns speaker_id = 'neuro', text style pastel pink #FFB6C1.
 * - UT-22.3: Key '3' assigns speaker_id = 'evil', text style neon red #FF0055.
 * - UT-22.4: Key '4' assigns speaker_id = 'guest', text style gold #FFD700.
 * - UT-22.5: Key '5' assigns speaker_id = 'chat', text style sky blue #87CEEB.
 * - IT-22.1: Multi-select 20 rows, press key '2' assigns all 20 rows to Neuro in 1 keystroke.
 * - ED-22.1: Keystroke color switch latency benchmark <= 50ms.
 * - Edge Cases: Active input/textarea isolation, modifier guard, IME composition guard,
 *   key repeat guard, numpad support, store multi-select actions.
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import {
  useStudioStore,
  getSpeakerColor,
  SPEAKER_COLORS,
} from '../stores/useStudioStore.ts';
import {
  useStudioHotkeys,
  isInputFocused,
  HOTKEY_SPEAKER_MAP,
  NUMPAD_CODE_MAP,
} from './useStudioHotkeys.ts';
import type { TranscriptSegment, SpeakerRole } from '../types/transcript.ts';
import { normalizeTranscriptSegment } from '../types/transcript.ts';

function createSegment(
  id: string,
  start: number,
  end: number,
  speaker: SpeakerRole = 'unknown',
): TranscriptSegment {
  return normalizeTranscriptSegment({
    segment_id: id,
    start_sec: start,
    end_sec: end,
    speaker_id: speaker,
    text_en: `English dialogue for ${id}`,
    text_vi: `Lời thoại tiếng Việt cho ${id}`,
    confidence: 0.95,
    cps_score: 14.5,
  });
}

function generateSegments(count: number): TranscriptSegment[] {
  const list: TranscriptSegment[] = [];
  for (let i = 0; i < count; i++) {
    list.push(createSegment(`seg_${String(i).padStart(4, '0')}`, i * 2, i * 2 + 1.8));
  }
  return list;
}

describe('TASK-M4-22: Subtitle Studio Hotkeys & Multi-Selection Reassignment', () => {
  beforeEach(() => {
    useStudioStore.getState().reset();
    document.body.innerHTML = '';
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  // -------------------------------------------------------------------------
  // Unit Tests: UT-22.1 to UT-22.5
  // -------------------------------------------------------------------------
  describe('UT-22.1 to UT-22.5: 1-Touch Hotkeys (1-5) Character Reassignment', () => {
    it('UT-22.1: Key "1" assigns speaker_id = "vedal" with color #98FB98', () => {
      const segments = [createSegment('seg_001', 0, 2, 'unknown')];
      useStudioStore.getState().setSegments(segments);
      useStudioStore.getState().selectSegment('seg_001');

      const { unmount } = renderHook(() => useStudioHotkeys());

      act(() => {
        window.dispatchEvent(
          new KeyboardEvent('keydown', { key: '1', bubbles: true, cancelable: true }),
        );
      });

      const updated = useStudioStore.getState().segments[0]!;
      expect(updated.speakerId).toBe('vedal');
      expect(updated.speaker_id).toBe('vedal');
      expect(getSpeakerColor(updated.speakerId)).toBe(SPEAKER_COLORS.vedal);
      expect(SPEAKER_COLORS.vedal).toBe('#98FB98');

      unmount();
    });

    it('UT-22.2: Key "2" assigns speaker_id = "neuro" with color #FFB6C1', () => {
      const segments = [createSegment('seg_001', 0, 2, 'unknown')];
      useStudioStore.getState().setSegments(segments);
      useStudioStore.getState().selectSegment('seg_001');

      const { unmount } = renderHook(() => useStudioHotkeys());

      act(() => {
        window.dispatchEvent(
          new KeyboardEvent('keydown', { key: '2', bubbles: true, cancelable: true }),
        );
      });

      const updated = useStudioStore.getState().segments[0]!;
      expect(updated.speakerId).toBe('neuro');
      expect(updated.speaker_id).toBe('neuro');
      expect(getSpeakerColor(updated.speakerId)).toBe(SPEAKER_COLORS.neuro);
      expect(SPEAKER_COLORS.neuro).toBe('#FFB6C1');

      unmount();
    });

    it('UT-22.3: Key "3" assigns speaker_id = "evil" with color #FF0055', () => {
      const segments = [createSegment('seg_001', 0, 2, 'unknown')];
      useStudioStore.getState().setSegments(segments);
      useStudioStore.getState().selectSegment('seg_001');

      const { unmount } = renderHook(() => useStudioHotkeys());

      act(() => {
        window.dispatchEvent(
          new KeyboardEvent('keydown', { key: '3', bubbles: true, cancelable: true }),
        );
      });

      const updated = useStudioStore.getState().segments[0]!;
      expect(updated.speakerId).toBe('evil');
      expect(updated.speaker_id).toBe('evil');
      expect(getSpeakerColor(updated.speakerId)).toBe(SPEAKER_COLORS.evil);
      expect(SPEAKER_COLORS.evil).toBe('#FF0055');

      unmount();
    });

    it('UT-22.4: Key "4" assigns speaker_id = "guest" with color #FFD700', () => {
      const segments = [createSegment('seg_001', 0, 2, 'unknown')];
      useStudioStore.getState().setSegments(segments);
      useStudioStore.getState().selectSegment('seg_001');

      const { unmount } = renderHook(() => useStudioHotkeys());

      act(() => {
        window.dispatchEvent(
          new KeyboardEvent('keydown', { key: '4', bubbles: true, cancelable: true }),
        );
      });

      const updated = useStudioStore.getState().segments[0]!;
      expect(updated.speakerId).toBe('guest');
      expect(updated.speaker_id).toBe('guest');
      expect(getSpeakerColor(updated.speakerId)).toBe(SPEAKER_COLORS.guest);
      expect(SPEAKER_COLORS.guest).toBe('#FFD700');

      unmount();
    });

    it('UT-22.5: Key "5" assigns speaker_id = "chat" with color #87CEEB', () => {
      const segments = [createSegment('seg_001', 0, 2, 'unknown')];
      useStudioStore.getState().setSegments(segments);
      useStudioStore.getState().selectSegment('seg_001');

      const { unmount } = renderHook(() => useStudioHotkeys());

      act(() => {
        window.dispatchEvent(
          new KeyboardEvent('keydown', { key: '5', bubbles: true, cancelable: true }),
        );
      });

      const updated = useStudioStore.getState().segments[0]!;
      expect(updated.speakerId).toBe('chat');
      expect(updated.speaker_id).toBe('chat');
      expect(getSpeakerColor(updated.speakerId)).toBe(SPEAKER_COLORS.chat);
      expect(SPEAKER_COLORS.chat).toBe('#87CEEB');

      unmount();
    });
  });

  // -------------------------------------------------------------------------
  // Integration Test: IT-22.1 (Multi-Selection Batch Update)
  // -------------------------------------------------------------------------
  describe('IT-22.1 & AC-22.2: Multi-Selection Batch Reassignment (>= 20 Rows)', () => {
    it('should batch-reassign 20 selected rows to "neuro" via key "2" in 1 keystroke without state corruption', () => {
      const segments = generateSegments(25);
      useStudioStore.getState().setSegments(segments);

      // Select first 20 rows (index 0 to 19)
      const target20Ids = segments.slice(0, 20).map((s) => s.segmentId);
      useStudioStore.getState().setSelectedSegmentIds(target20Ids);

      expect(useStudioStore.getState().selectedSegmentIds).toHaveLength(20);

      const onSpeakerChange = vi.fn();
      const { unmount } = renderHook(() => useStudioHotkeys({ onSpeakerChange }));

      act(() => {
        window.dispatchEvent(
          new KeyboardEvent('keydown', { key: '2', bubbles: true, cancelable: true }),
        );
      });

      const updatedSegments = useStudioStore.getState().segments;

      // Verify all 20 rows are now Neuro
      for (let i = 0; i < 20; i++) {
        expect(updatedSegments[i]!.speakerId).toBe('neuro');
        expect(updatedSegments[i]!.speaker_id).toBe('neuro');
      }

      // Verify row 20 to 24 remain unchanged ('unknown')
      for (let i = 20; i < 25; i++) {
        expect(updatedSegments[i]!.speakerId).toBe('unknown');
      }

      // Verify callback was called once with the 20 IDs
      expect(onSpeakerChange).toHaveBeenCalledTimes(1);
      expect(onSpeakerChange).toHaveBeenCalledWith('neuro', target20Ids);

      unmount();
    });

    it('should support Shift+Click range selection from anchor to target', () => {
      const segments = generateSegments(10);
      useStudioStore.getState().setSegments(segments);

      // Select segment at index 2
      useStudioStore.getState().selectSegment(segments[2]!.segmentId);
      expect(useStudioStore.getState().selectedSegmentIds).toEqual([segments[2]!.segmentId]);

      // Range select to segment at index 6
      useStudioStore.getState().selectSegmentRange(segments[6]!.segmentId);

      const selectedIds = useStudioStore.getState().selectedSegmentIds;
      expect(selectedIds).toHaveLength(5);
      expect(selectedIds).toEqual([
        segments[2]!.segmentId,
        segments[3]!.segmentId,
        segments[4]!.segmentId,
        segments[5]!.segmentId,
        segments[6]!.segmentId,
      ]);

      // Reverse range select to segment at index 0
      useStudioStore.getState().selectSegmentRange(segments[0]!.segmentId);
      const reverseIds = useStudioStore.getState().selectedSegmentIds;
      expect(reverseIds).toHaveLength(3);
      expect(reverseIds).toEqual([
        segments[0]!.segmentId,
        segments[1]!.segmentId,
        segments[2]!.segmentId,
      ]);
    });

    it('should support toggle selection (Ctrl/Cmd+Click)', () => {
      const segments = generateSegments(5);
      useStudioStore.getState().setSegments(segments);

      // Initial single select
      useStudioStore.getState().toggleSegmentSelection(segments[1]!.segmentId, false);
      expect(useStudioStore.getState().selectedSegmentIds).toEqual([segments[1]!.segmentId]);

      // Toggle add segment 3
      useStudioStore.getState().toggleSegmentSelection(segments[3]!.segmentId, true);
      expect(useStudioStore.getState().selectedSegmentIds).toEqual([
        segments[1]!.segmentId,
        segments[3]!.segmentId,
      ]);

      // Toggle remove segment 1
      useStudioStore.getState().toggleSegmentSelection(segments[1]!.segmentId, true);
      expect(useStudioStore.getState().selectedSegmentIds).toEqual([segments[3]!.segmentId]);
    });

    it('should support selectAllSegments and clearSegmentSelection', () => {
      const segments = generateSegments(10);
      useStudioStore.getState().setSegments(segments);

      useStudioStore.getState().selectAllSegments();
      expect(useStudioStore.getState().selectedSegmentIds).toHaveLength(10);
      expect(useStudioStore.getState().selectedSegmentId).toBe(segments[0]!.segmentId);

      useStudioStore.getState().clearSegmentSelection();
      expect(useStudioStore.getState().selectedSegmentIds).toEqual([]);
      expect(useStudioStore.getState().selectedSegmentId).toBeNull();
    });
  });

  // -------------------------------------------------------------------------
  // Performance SLA: ED-22.1 & AC-22.1
  // -------------------------------------------------------------------------
  describe('ED-22.1 & AC-22.1: Performance SLA (<= 50ms Reassignment Latency)', () => {
    it('should complete batch speaker reassignment for 20 rows within <= 50ms', () => {
      const segments = generateSegments(500);
      useStudioStore.getState().setSegments(segments);

      const target20Ids = segments.slice(50, 70).map((s) => s.segmentId);
      useStudioStore.getState().setSelectedSegmentIds(target20Ids);

      const { unmount } = renderHook(() => useStudioHotkeys());

      const t0 = performance.now();
      act(() => {
        window.dispatchEvent(
          new KeyboardEvent('keydown', { key: '3', bubbles: true, cancelable: true }),
        );
      });
      const elapsed = performance.now() - t0;

      expect(elapsed).toBeLessThanOrEqual(50);

      // Verify the 20 rows are indeed Evil
      const storeSegs = useStudioStore.getState().segments;
      for (let i = 50; i < 70; i++) {
        expect(storeSegs[i]!.speakerId).toBe('evil');
      }

      unmount();
    });
  });

  // -------------------------------------------------------------------------
  // Strict Input Guards: Zero Typing Interference
  // -------------------------------------------------------------------------
  describe('Strict Input Guards: Zero Interference with Active Editable Elements', () => {
    it('should NOT trigger speaker hotkey when typing in a focused <textarea>', () => {
      const segments = [createSegment('seg_001', 0, 2, 'vedal')];
      useStudioStore.getState().setSegments(segments);
      useStudioStore.getState().selectSegment('seg_001');

      const textarea = document.createElement('textarea');
      document.body.appendChild(textarea);
      textarea.focus();

      expect(document.activeElement).toBe(textarea);
      expect(isInputFocused()).toBe(true);

      const { unmount } = renderHook(() => useStudioHotkeys());

      act(() => {
        textarea.dispatchEvent(
          new KeyboardEvent('keydown', { key: '2', bubbles: true, cancelable: true }),
        );
      });

      // Segment speaker must remain 'vedal'
      expect(useStudioStore.getState().segments[0]!.speakerId).toBe('vedal');

      unmount();
      document.body.removeChild(textarea);
    });

    it('should NOT trigger speaker hotkey when typing in a focused <input>', () => {
      const segments = [createSegment('seg_001', 0, 2, 'vedal')];
      useStudioStore.getState().setSegments(segments);
      useStudioStore.getState().selectSegment('seg_001');

      const input = document.createElement('input');
      document.body.appendChild(input);
      input.focus();

      expect(document.activeElement).toBe(input);
      expect(isInputFocused()).toBe(true);

      const { unmount } = renderHook(() => useStudioHotkeys());

      act(() => {
        input.dispatchEvent(
          new KeyboardEvent('keydown', { key: '3', bubbles: true, cancelable: true }),
        );
      });

      expect(useStudioStore.getState().segments[0]!.speakerId).toBe('vedal');

      unmount();
      document.body.removeChild(input);
    });

    it('should NOT trigger speaker hotkey inside contentEditable element', () => {
      const segments = [createSegment('seg_001', 0, 2, 'vedal')];
      useStudioStore.getState().setSegments(segments);
      useStudioStore.getState().selectSegment('seg_001');

      const div = document.createElement('div');
      div.contentEditable = 'true';
      div.tabIndex = 0;
      document.body.appendChild(div);
      div.focus();

      expect(isInputFocused()).toBe(true);

      const { unmount } = renderHook(() => useStudioHotkeys());

      act(() => {
        div.dispatchEvent(
          new KeyboardEvent('keydown', { key: '4', bubbles: true, cancelable: true }),
        );
      });

      expect(useStudioStore.getState().segments[0]!.speakerId).toBe('vedal');

      unmount();
      document.body.removeChild(div);
    });
  });

  // -------------------------------------------------------------------------
  // Keyboard Guards: Modifiers, IME, Repeat, Numpad
  // -------------------------------------------------------------------------
  describe('Keyboard Edge Case Guards', () => {
    it('should ignore Ctrl/Cmd/Alt modifier combinations (e.g. Ctrl+1)', () => {
      const segments = [createSegment('seg_001', 0, 2, 'vedal')];
      useStudioStore.getState().setSegments(segments);
      useStudioStore.getState().selectSegment('seg_001');

      const { unmount } = renderHook(() => useStudioHotkeys());

      // Ctrl + 2
      act(() => {
        window.dispatchEvent(
          new KeyboardEvent('keydown', { key: '2', ctrlKey: true, bubbles: true }),
        );
      });
      expect(useStudioStore.getState().segments[0]!.speakerId).toBe('vedal');

      // Meta + 2
      act(() => {
        window.dispatchEvent(
          new KeyboardEvent('keydown', { key: '2', metaKey: true, bubbles: true }),
        );
      });
      expect(useStudioStore.getState().segments[0]!.speakerId).toBe('vedal');

      // Alt + 2
      act(() => {
        window.dispatchEvent(
          new KeyboardEvent('keydown', { key: '2', altKey: true, bubbles: true }),
        );
      });
      expect(useStudioStore.getState().segments[0]!.speakerId).toBe('vedal');

      unmount();
    });

    it('should ignore IME composition events (isComposing or keyCode 229)', () => {
      const segments = [createSegment('seg_001', 0, 2, 'vedal')];
      useStudioStore.getState().setSegments(segments);
      useStudioStore.getState().selectSegment('seg_001');

      const { unmount } = renderHook(() => useStudioHotkeys());

      act(() => {
        window.dispatchEvent(
          new KeyboardEvent('keydown', { key: '2', isComposing: true, bubbles: true }),
        );
      });
      expect(useStudioStore.getState().segments[0]!.speakerId).toBe('vedal');

      act(() => {
        window.dispatchEvent(
          new KeyboardEvent('keydown', { key: '2', keyCode: 229, bubbles: true }),
        );
      });
      expect(useStudioStore.getState().segments[0]!.speakerId).toBe('vedal');

      unmount();
    });

    it('should ignore key repeat events', () => {
      const segments = [createSegment('seg_001', 0, 2, 'vedal')];
      useStudioStore.getState().setSegments(segments);
      useStudioStore.getState().selectSegment('seg_001');

      const onSpeakerChange = vi.fn();
      const { unmount } = renderHook(() => useStudioHotkeys({ onSpeakerChange }));

      act(() => {
        window.dispatchEvent(
          new KeyboardEvent('keydown', { key: '2', repeat: true, bubbles: true }),
        );
      });

      expect(useStudioStore.getState().segments[0]!.speakerId).toBe('vedal');
      expect(onSpeakerChange).not.toHaveBeenCalled();

      unmount();
    });

    it('should support Numpad digits via event.code (Numpad1..Numpad5)', () => {
      const segments = [createSegment('seg_001', 0, 2, 'unknown')];
      useStudioStore.getState().setSegments(segments);
      useStudioStore.getState().selectSegment('seg_001');

      const { unmount } = renderHook(() => useStudioHotkeys());

      // Numpad1 -> vedal
      act(() => {
        window.dispatchEvent(
          new KeyboardEvent('keydown', { key: '1', code: 'Numpad1', bubbles: true }),
        );
      });
      expect(useStudioStore.getState().segments[0]!.speakerId).toBe('vedal');

      // Numpad2 -> neuro
      act(() => {
        window.dispatchEvent(
          new KeyboardEvent('keydown', { key: '2', code: 'Numpad2', bubbles: true }),
        );
      });
      expect(useStudioStore.getState().segments[0]!.speakerId).toBe('neuro');

      // Numpad3 -> evil
      act(() => {
        window.dispatchEvent(
          new KeyboardEvent('keydown', { key: '3', code: 'Numpad3', bubbles: true }),
        );
      });
      expect(useStudioStore.getState().segments[0]!.speakerId).toBe('evil');

      // Numpad4 -> guest
      act(() => {
        window.dispatchEvent(
          new KeyboardEvent('keydown', { key: '4', code: 'Numpad4', bubbles: true }),
        );
      });
      expect(useStudioStore.getState().segments[0]!.speakerId).toBe('guest');

      // Numpad5 -> chat
      act(() => {
        window.dispatchEvent(
          new KeyboardEvent('keydown', { key: '5', code: 'Numpad5', bubbles: true }),
        );
      });
      expect(useStudioStore.getState().segments[0]!.speakerId).toBe('chat');

      unmount();
    });

    it('should do nothing when no segment is selected', () => {
      const segments = [createSegment('seg_001', 0, 2, 'vedal')];
      useStudioStore.getState().setSegments(segments);
      useStudioStore.getState().clearSegmentSelection();

      const { unmount } = renderHook(() => useStudioHotkeys());

      act(() => {
        window.dispatchEvent(
          new KeyboardEvent('keydown', { key: '3', bubbles: true, cancelable: true }),
        );
      });

      // Remains vedal because nothing was selected
      expect(useStudioStore.getState().segments[0]!.speakerId).toBe('vedal');

      unmount();
    });

    it('should cleanly remove event listener on unmount', () => {
      const segments = [createSegment('seg_001', 0, 2, 'vedal')];
      useStudioStore.getState().setSegments(segments);
      useStudioStore.getState().selectSegment('seg_001');

      const { unmount } = renderHook(() => useStudioHotkeys());
      unmount();

      act(() => {
        window.dispatchEvent(
          new KeyboardEvent('keydown', { key: '2', bubbles: true, cancelable: true }),
        );
      });

      // Still vedal because hook was unmounted
      expect(useStudioStore.getState().segments[0]!.speakerId).toBe('vedal');
    });
  });

  // -------------------------------------------------------------------------
  // ADR-006 Public Contract Verification
  // -------------------------------------------------------------------------
  describe('ADR-006: Hotkey Speaker Mapping Dictionaries Contract', () => {
    it('should export HOTKEY_SPEAKER_MAP matching ADR-006 1-to-5 specification and be immutable', () => {
      expect(HOTKEY_SPEAKER_MAP).toEqual({
        '1': 'vedal',
        '2': 'neuro',
        '3': 'evil',
        '4': 'guest',
        '5': 'chat',
      });
      expect(Object.isFrozen(HOTKEY_SPEAKER_MAP)).toBe(true);
    });

    it('should export NUMPAD_CODE_MAP matching ADR-006 Numpad specification and be immutable', () => {
      expect(NUMPAD_CODE_MAP).toEqual({
        Numpad1: 'vedal',
        Numpad2: 'neuro',
        Numpad3: 'evil',
        Numpad4: 'guest',
        Numpad5: 'chat',
      });
      expect(Object.isFrozen(NUMPAD_CODE_MAP)).toBe(true);
    });
  });
});
