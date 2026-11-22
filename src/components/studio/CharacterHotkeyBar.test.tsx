/**
 * Character Hotkey Bar Test Suite
 *
 * Verifies toolbar rendering, badges, selection counter, click-to-reassign,
 * and bulk selection actions according to TASK-M4-22 specifications and ADR-006.
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { CharacterHotkeyBar, CHARACTER_HOTKEYS } from './CharacterHotkeyBar.tsx';
import { useStudioStore, SPEAKER_COLORS } from '../../stores/useStudioStore.ts';
import type { TranscriptSegment, SpeakerRole } from '../../types/transcript.ts';
import { normalizeTranscriptSegment } from '../../types/transcript.ts';

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
    text_en: `English ${id}`,
    text_vi: `Vietsub ${id}`,
    confidence: 0.95,
    cps_score: 15.0,
  });
}

describe('CharacterHotkeyBar Component', () => {
  beforeEach(() => {
    useStudioStore.getState().reset();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('should render 5 character buttons with correct names and hotkey badges [1]-[5]', () => {
    render(<CharacterHotkeyBar />);

    expect(screen.getByTestId('character-hotkey-bar')).toBeInTheDocument();

    for (const item of CHARACTER_HOTKEYS) {
      const btn = screen.getByTestId(`hotkey-btn-${item.key}`);
      expect(btn).toBeInTheDocument();
      expect(btn).toHaveTextContent(item.name);

      const badge = screen.getByTestId(`hotkey-badge-${item.key}`);
      expect(badge).toBeInTheDocument();
      expect(badge).toHaveTextContent(item.key);
    }
  });

  it('should disable character buttons when no segments are selected', () => {
    const segments = [createSegment('seg_1', 0, 2)];
    useStudioStore.getState().setSegments(segments);
    useStudioStore.getState().clearSegmentSelection();

    render(<CharacterHotkeyBar />);

    const btn1 = screen.getByTestId('hotkey-btn-1');
    expect(btn1).toBeDisabled();

    const counter = screen.getByTestId('selection-counter');
    expect(counter).toHaveTextContent('Chưa chọn dòng nào');

    const clearBtn = screen.getByTestId('btn-clear-selection');
    expect(clearBtn).toBeDisabled();
  });

  it('should display correct selection count when single segment is selected', () => {
    const segments = [createSegment('seg_1', 0, 2), createSegment('seg_2', 2, 4)];
    useStudioStore.getState().setSegments(segments);
    useStudioStore.getState().selectSegment('seg_1');

    render(<CharacterHotkeyBar />);

    const counter = screen.getByTestId('selection-counter');
    expect(counter).toHaveTextContent('Đã chọn 1 dòng');

    const btn1 = screen.getByTestId('hotkey-btn-1');
    expect(btn1).not.toBeDisabled();
  });

  it('should display correct selection count when 20 segments are selected', () => {
    const segments = Array.from({ length: 25 }, (_, i) =>
      createSegment(`seg_${i}`, i * 2, i * 2 + 1.8),
    );
    useStudioStore.getState().setSegments(segments);

    const first20Ids = segments.slice(0, 20).map((s) => s.segmentId);
    useStudioStore.getState().setSelectedSegmentIds(first20Ids);

    render(<CharacterHotkeyBar />);

    const counter = screen.getByTestId('selection-counter');
    expect(counter).toHaveTextContent('Đã chọn 20 dòng');

    const btn2 = screen.getByTestId('hotkey-btn-2');
    expect(btn2).not.toBeDisabled();
  });

  it('should reassign speaker to selected segments when clicking a character button', () => {
    const segments = [
      createSegment('seg_1', 0, 2, 'unknown'),
      createSegment('seg_2', 2, 4, 'unknown'),
    ];
    useStudioStore.getState().setSegments(segments);
    useStudioStore.getState().setSelectedSegmentIds(['seg_1', 'seg_2']);

    const onSpeakerAssigned = vi.fn();
    render(<CharacterHotkeyBar onSpeakerAssigned={onSpeakerAssigned} />);

    // Click Neuro button (key 2)
    const neuroBtn = screen.getByTestId('hotkey-btn-2');
    fireEvent.click(neuroBtn);

    const updated = useStudioStore.getState().segments;
    expect(updated[0]!.speakerId).toBe('neuro');
    expect(updated[1]!.speakerId).toBe('neuro');

    expect(onSpeakerAssigned).toHaveBeenCalledWith('neuro', ['seg_1', 'seg_2']);
  });

  it('should reassign speaker to single segment when selected via fallback selectedSegmentId', () => {
    const segments = [createSegment('seg_1', 0, 2, 'unknown')];
    useStudioStore.getState().setSegments(segments);
    useStudioStore.getState().selectSegment('seg_1');

    render(<CharacterHotkeyBar />);

    // Click Vedal button (key 1)
    const vedalBtn = screen.getByTestId('hotkey-btn-1');
    fireEvent.click(vedalBtn);

    const updated = useStudioStore.getState().segments[0]!;
    expect(updated.speakerId).toBe('vedal');
  });

  it('should select all segments when clicking "Tất cả"', () => {
    const segments = [
      createSegment('seg_1', 0, 2),
      createSegment('seg_2', 2, 4),
      createSegment('seg_3', 4, 6),
    ];
    useStudioStore.getState().setSegments(segments);

    render(<CharacterHotkeyBar />);

    const selectAllBtn = screen.getByTestId('btn-select-all');
    fireEvent.click(selectAllBtn);

    expect(useStudioStore.getState().selectedSegmentIds).toEqual(['seg_1', 'seg_2', 'seg_3']);
  });

  it('should clear selection when clicking "Bỏ chọn"', () => {
    const segments = [createSegment('seg_1', 0, 2), createSegment('seg_2', 2, 4)];
    useStudioStore.getState().setSegments(segments);
    useStudioStore.getState().setSelectedSegmentIds(['seg_1', 'seg_2']);

    render(<CharacterHotkeyBar />);

    const clearBtn = screen.getByTestId('btn-clear-selection');
    fireEvent.click(clearBtn);

    expect(useStudioStore.getState().selectedSegmentIds).toEqual([]);
    expect(useStudioStore.getState().selectedSegmentId).toBeNull();
  });

  it('should support clicking all 5 characters and verify official color palette', () => {
    const segments = [createSegment('seg_1', 0, 2, 'unknown')];
    useStudioStore.getState().setSegments(segments);
    useStudioStore.getState().selectSegment('seg_1');

    render(<CharacterHotkeyBar />);

    // 1 -> Vedal (#98FB98)
    fireEvent.click(screen.getByTestId('hotkey-btn-1'));
    expect(useStudioStore.getState().segments[0]!.speakerId).toBe('vedal');
    expect(SPEAKER_COLORS.vedal).toBe('#98FB98');

    // 2 -> Neuro (#FFB6C1)
    fireEvent.click(screen.getByTestId('hotkey-btn-2'));
    expect(useStudioStore.getState().segments[0]!.speakerId).toBe('neuro');
    expect(SPEAKER_COLORS.neuro).toBe('#FFB6C1');

    // 3 -> Evil (#FF0055)
    fireEvent.click(screen.getByTestId('hotkey-btn-3'));
    expect(useStudioStore.getState().segments[0]!.speakerId).toBe('evil');
    expect(SPEAKER_COLORS.evil).toBe('#FF0055');

    // 4 -> Guest (#FFD700)
    fireEvent.click(screen.getByTestId('hotkey-btn-4'));
    expect(useStudioStore.getState().segments[0]!.speakerId).toBe('guest');
    expect(SPEAKER_COLORS.guest).toBe('#FFD700');

    // 5 -> Chat (#87CEEB)
    fireEvent.click(screen.getByTestId('hotkey-btn-5'));
    expect(useStudioStore.getState().segments[0]!.speakerId).toBe('chat');
    expect(SPEAKER_COLORS.chat).toBe('#87CEEB');
  });
});
