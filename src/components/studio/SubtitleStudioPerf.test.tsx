import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, act } from '@testing-library/react';
import { useStudioStore } from '../../stores/useStudioStore.ts';
import { normalizeTranscriptSegment, type TranscriptSegment } from '../../types/transcript.ts';

let characterHotkeyBarRenderCount = 0;

vi.mock('./CharacterHotkeyBar.tsx', () => ({
  CharacterHotkeyBar: () => {
    characterHotkeyBarRenderCount += 1;
    return <div data-testid="character-hotkey-bar-spy" />;
  },
}));

vi.mock('./VideoCanvas.tsx', () => ({
  VideoCanvas: () => <div data-testid="video-canvas-spy" />,
}));

vi.mock('./VirtualizedSegmentList.tsx', () => ({
  VirtualizedSegmentList: () => <div data-testid="virtualized-list-spy" />,
}));

vi.mock('../export/ExportModal.tsx', () => ({
  ExportModal: () => null,
}));

import { SubtitleStudioInner } from './SubtitleStudio.tsx';

function makeSegment(id: string, start: number, end: number): TranscriptSegment {
  return normalizeTranscriptSegment({
    segment_id: id,
    start_sec: start,
    end_sec: end,
    speaker_id: 'neuro',
    text_en: 'Hello',
    text_vi: 'Chào',
    confidence: 0.95,
    cps_score: 5.0,
  });
}

describe('SubtitleStudioInner Render Efficiency (HIGH-01)', () => {
  beforeEach(() => {
    characterHotkeyBarRenderCount = 0;
    useStudioStore.getState().reset();
  });

  it('does not re-render SubtitleStudioInner on playback ticks when activeSegmentIds is unchanged while TimelineScrubber updates directly', () => {
    useStudioStore.getState().setDuration(60.0);
    useStudioStore
      .getState()
      .setSegments([makeSegment('seg_01', 0.0, 10.0), makeSegment('seg_02', 12.0, 18.0)]);

    render(<SubtitleStudioInner videoSrc="https://test.local/clip.mp4" />);

    const rendersAfterMount = characterHotkeyBarRenderCount;
    expect(rendersAfterMount).toBeGreaterThan(0);

    // Advance playback within seg_01 (0..10s): activeSegmentIds stays ['seg_01']
    act(() => {
      useStudioStore.getState().setCurrentTime(1.5);
      useStudioStore.getState().setCurrentTime(4.2);
      useStudioStore.getState().setCurrentTime(8.8);
    });

    // SubtitleStudioInner did NOT re-render, yet TimelineScrubber updated directly
    expect(characterHotkeyBarRenderCount).toBe(rendersAfterMount);
    expect(screen.getByTestId('timeline-timecode')).toHaveTextContent('00:08.800');

    // Crossing into seg_02 updates TimelineScrubber and ActiveSegmentCounter without re-rendering SubtitleStudioInner
    act(() => {
      useStudioStore.getState().setCurrentTime(15.4);
    });
    expect(characterHotkeyBarRenderCount).toBe(rendersAfterMount);
    expect(screen.getByTestId('timeline-timecode')).toHaveTextContent('00:15.400');
    expect(screen.getByText(/1 đang active/i)).toBeInTheDocument();
  });
});
