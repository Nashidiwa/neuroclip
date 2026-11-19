/**
 * NeuroClip Subtitle Studio - Character Hotkey & Multi-Selection Toolbar
 *
 * Visual toolbar displaying the 5 character roles with color swatches,
 * hotkey badges [1]-[5], selection counter, and batch actions.
 * Adheres strictly to ADR-006 and dark VTuber theme tokens.
 */

import React, { useMemo } from 'react';
import { CheckSquare, XSquare, Sparkles } from 'lucide-react';
import { useStudioStore } from '../../stores/useStudioStore.ts';
import type { SpeakerRole } from '../../types/transcript.ts';

export interface CharacterHotkeyItem {
  readonly key: string;
  readonly role: SpeakerRole;
  readonly name: string;
  readonly subtitle: string;
  readonly hexColor: string;
  readonly badgeBg: string;
}

export const CHARACTER_HOTKEYS: readonly CharacterHotkeyItem[] = Object.freeze([
  {
    key: '1',
    role: 'vedal',
    name: 'Vedal',
    subtitle: 'Tutel',
    hexColor: '#98FB98',
    badgeBg: 'bg-[#98FB98]/10 text-[#98FB98] border-[#98FB98]/40 hover:bg-[#98FB98]/20',
  },
  {
    key: '2',
    role: 'neuro',
    name: 'Neuro',
    subtitle: 'Neuro-sama',
    hexColor: '#FFB6C1',
    badgeBg: 'bg-[#FFB6C1]/10 text-[#FFB6C1] border-[#FFB6C1]/40 hover:bg-[#FFB6C1]/20',
  },
  {
    key: '3',
    role: 'evil',
    name: 'Evil',
    subtitle: 'Evil Neuro',
    hexColor: '#FF0055',
    badgeBg: 'bg-[#FF0055]/10 text-[#FF0055] border-[#FF0055]/40 hover:bg-[#FF0055]/20',
  },
  {
    key: '4',
    role: 'guest',
    name: 'Guest',
    subtitle: 'Collab',
    hexColor: '#FFD700',
    badgeBg: 'bg-[#FFD700]/10 text-[#FFD700] border-[#FFD700]/40 hover:bg-[#FFD700]/20',
  },
  {
    key: '5',
    role: 'chat',
    name: 'Chat',
    subtitle: 'Swarm',
    hexColor: '#87CEEB',
    badgeBg: 'bg-[#87CEEB]/10 text-[#87CEEB] border-[#87CEEB]/40 hover:bg-[#87CEEB]/20',
  },
]);

export interface CharacterHotkeyBarProps {
  readonly className?: string | undefined;
  readonly onSpeakerAssigned?: (
    speakerId: SpeakerRole,
    updatedSegmentIds: readonly string[],
  ) => void;
}

export const CharacterHotkeyBar: React.FC<CharacterHotkeyBarProps> = ({
  className = '',
  onSpeakerAssigned,
}) => {
  const selectedSegmentIds = useStudioStore((s) => s.selectedSegmentIds);
  const selectedSegmentId = useStudioStore((s) => s.selectedSegmentId);
  const segments = useStudioStore((s) => s.segments);
  const updateSegmentsSpeaker = useStudioStore((s) => s.updateSegmentsSpeaker);
  const selectAllSegments = useStudioStore((s) => s.selectAllSegments);
  const clearSegmentSelection = useStudioStore((s) => s.clearSegmentSelection);

  const effectiveSelectedIds = useMemo(() => {
    if (selectedSegmentIds.length > 0) return selectedSegmentIds;
    if (selectedSegmentId) return [selectedSegmentId];
    return [];
  }, [selectedSegmentIds, selectedSegmentId]);

  const selectedCount = effectiveSelectedIds.length;
  const hasSelection = selectedCount > 0;
  const hasSegments = segments.length > 0;

  const handleCharacterClick = (item: CharacterHotkeyItem) => {
    if (!hasSelection) return;
    updateSegmentsSpeaker(effectiveSelectedIds, item.role);
    if (onSpeakerAssigned) {
      onSpeakerAssigned(item.role, effectiveSelectedIds);
    }
  };

  return (
    <div
      role="toolbar"
      aria-label="Thanh công cụ phân vai nhân vật và phím tắt"
      data-testid="character-hotkey-bar"
      className={`flex flex-col space-y-2 border-b border-border-dark bg-bg-secondary/95 px-3 py-2 text-xs select-none ${className}`}
    >
      {/* 5 Character Buttons with Badges */}
      <div className="flex items-center justify-between gap-1 overflow-x-auto pb-0.5">
        {CHARACTER_HOTKEYS.map((item) => (
          <button
            key={item.key}
            type="button"
            data-testid={`hotkey-btn-${item.key}`}
            onClick={() => handleCharacterClick(item)}
            disabled={!hasSelection}
            title={
              hasSelection
                ? `Gán vai ${item.name} cho ${selectedCount} dòng (Phím ${item.key})`
                : `Chọn dòng phụ đề trước khi bấm phím ${item.key}`
            }
            aria-label={`Gán vai ${item.name} (Phím ${item.key})`}
            className={`group flex flex-1 items-center justify-center space-x-1.5 rounded-md border px-2 py-1.5 transition-all text-xs font-medium ${
              hasSelection
                ? `${item.badgeBg} active:scale-95 cursor-pointer shadow-sm`
                : 'border-border-dark/60 bg-bg-tertiary/40 text-slate-500 opacity-60 cursor-not-allowed'
            }`}
          >
            {/* Color Dot Swatch */}
            <span
              className="h-2 w-2 rounded-full shrink-0 shadow-sm"
              style={{ backgroundColor: item.hexColor }}
              aria-hidden="true"
            />

            {/* Character Name */}
            <span className="font-semibold truncate">{item.name}</span>

            {/* Hotkey Indicator Pill */}
            <kbd
              data-testid={`hotkey-badge-${item.key}`}
              className="inline-flex h-4 min-w-[16px] items-center justify-center rounded border border-border-dark/80 bg-bg-primary/80 px-1 font-mono text-[10px] text-slate-300 group-hover:text-white"
            >
              {item.key}
            </kbd>
          </button>
        ))}
      </div>

      {/* Multi-Selection Counter & Convenience Actions */}
      <div className="flex items-center justify-between text-[11px] text-slate-400 pt-0.5 border-t border-border-dark/40">
        <div className="flex items-center space-x-1.5 font-mono">
          <Sparkles className="h-3 w-3 text-accent-neuro shrink-0" />
          <span data-testid="selection-counter" className="truncate">
            {hasSelection ? (
              <span className="text-slate-200">
                Đã chọn <strong className="text-accent-neuro">{selectedCount}</strong> dòng
              </span>
            ) : (
              <span className="text-slate-500 italic">
                Chưa chọn dòng nào (Shift/Ctrl+Click hoặc bấm 1-5)
              </span>
            )}
          </span>
        </div>

        <div className="flex items-center space-x-2">
          {/* Select All Action */}
          <button
            type="button"
            data-testid="btn-select-all"
            onClick={selectAllSegments}
            disabled={!hasSegments}
            title="Chọn tất cả các dòng phụ đề"
            className="inline-flex items-center space-x-1 rounded px-1.5 py-0.5 text-slate-300 hover:text-white hover:bg-bg-tertiary disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
          >
            <CheckSquare className="h-3 w-3" />
            <span>Tất cả</span>
          </button>

          {/* Clear Selection Action */}
          <button
            type="button"
            data-testid="btn-clear-selection"
            onClick={clearSegmentSelection}
            disabled={!hasSelection}
            title="Bỏ chọn các dòng đang chọn"
            className="inline-flex items-center space-x-1 rounded px-1.5 py-0.5 text-slate-300 hover:text-rose-400 hover:bg-bg-tertiary disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
          >
            <XSquare className="h-3 w-3" />
            <span>Bỏ chọn</span>
          </button>
        </div>
      </div>
    </div>
  );
};
