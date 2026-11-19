/**
 * NeuroClip Subtitle Studio - Global Keyboard Hotkeys Hook
 *
 * Implements 1-Touch Character Hotkeys (1-5) per ADR-006 & TASK-M4-22:
 * - '1' / 'Numpad1': Vedal (#98FB98)
 * - '2' / 'Numpad2': Neuro-sama (#FFB6C1)
 * - '3' / 'Numpad3': Evil Neuro (#FF0055)
 * - '4' / 'Numpad4': Guest / Collab (#FFD700)
 * - '5' / 'Numpad5': Twitch Chat / Swarm (#87CEEB)
 *
 * Strict Guardrails:
 * - Excludes active input/textarea/contenteditable elements (zero text typing interference)
 * - Excludes IME composition sessions (Vietnamese Telex/VNI IME)
 * - Excludes modifier combinations (Ctrl/Cmd/Alt)
 * - Reassigns all selected segments in <= 50ms without state errors
 */

import { useEffect, useCallback } from 'react';
import type { SpeakerRole } from '../types/transcript.ts';
import { useStudioStore } from '../stores/useStudioStore.ts';

export const HOTKEY_SPEAKER_MAP: Readonly<Record<string, SpeakerRole>> = Object.freeze({
  '1': 'vedal',
  '2': 'neuro',
  '3': 'evil',
  '4': 'guest',
  '5': 'chat',
});

export const NUMPAD_CODE_MAP: Readonly<Record<string, SpeakerRole>> = Object.freeze({
  Numpad1: 'vedal',
  Numpad2: 'neuro',
  Numpad3: 'evil',
  Numpad4: 'guest',
  Numpad5: 'chat',
});

export interface UseStudioHotkeysOptions {
  readonly enabled?: boolean;
  readonly targetElement?: HTMLElement | Window | null;
  readonly onSpeakerChange?: (
    speakerId: SpeakerRole,
    updatedSegmentIds: readonly string[],
  ) => void;
}

/**
 * Checks whether user focus or the event target is currently inside an editable text element.
 */
export function isInputFocused(event?: KeyboardEvent): boolean {
  const isEditable = (el: HTMLElement | null): boolean => {
    if (!el) return false;
    const tagName = el.tagName?.toUpperCase();
    if (tagName === 'INPUT' || tagName === 'TEXTAREA' || tagName === 'SELECT') {
      return true;
    }
    if (el.isContentEditable) {
      return true;
    }
    const attr = el.getAttribute?.('contenteditable');
    if (attr === 'true' || attr === '') {
      return true;
    }
    if (
      (el as unknown as { contentEditable?: string | boolean }).contentEditable === 'true' ||
      (el as unknown as { contentEditable?: string | boolean }).contentEditable === true
    ) {
      return true;
    }
    return false;
  };

  const target = event?.target as HTMLElement | null;
  if (isEditable(target)) {
    return true;
  }

  if (typeof document !== 'undefined' && document.activeElement) {
    if (isEditable(document.activeElement as HTMLElement)) {
      return true;
    }
  }

  return false;
}

export function useStudioHotkeys(options: UseStudioHotkeysOptions = {}): {
  readonly enabled: boolean;
  readonly reassignSelectedSpeaker: (speakerRole: SpeakerRole) => void;
} {
  const { enabled = true, targetElement, onSpeakerChange } = options;

  const reassignSelectedSpeaker = useCallback(
    (speakerRole: SpeakerRole) => {
      const state = useStudioStore.getState();
      const targetIds =
        state.selectedSegmentIds.length > 0
          ? state.selectedSegmentIds
          : state.selectedSegmentId
          ? [state.selectedSegmentId]
          : [];

      if (targetIds.length === 0) return;

      state.updateSegmentsSpeaker(targetIds, speakerRole);

      if (onSpeakerChange) {
        onSpeakerChange(speakerRole, targetIds);
      }
    },
    [onSpeakerChange],
  );

  useEffect(() => {
    if (!enabled) return;

    const target = targetElement ?? (typeof window !== 'undefined' ? window : null);
    if (!target) return;

    const handleKeyDown = (e: Event) => {
      const keyEvent = e as KeyboardEvent;

      // 1. Modifier guard (Ctrl/Cmd/Alt must not trigger hotkeys)
      if (keyEvent.ctrlKey || keyEvent.metaKey || keyEvent.altKey) {
        return;
      }

      // 2. IME composition guard (Vietnamese Telex/VNI IME)
      if (keyEvent.isComposing || keyEvent.keyCode === 229) {
        return;
      }

      // 3. Key repeat guard
      if (keyEvent.repeat) {
        return;
      }

      // 4. Active text input guard
      if (isInputFocused(keyEvent)) {
        return;
      }

      // 5. Hotkey lookup (matches top-row digits or numpad digits)
      const targetRole = HOTKEY_SPEAKER_MAP[keyEvent.key] ?? NUMPAD_CODE_MAP[keyEvent.code];
      if (!targetRole) {
        return;
      }

      const state = useStudioStore.getState();
      const targetIds =
        state.selectedSegmentIds.length > 0
          ? state.selectedSegmentIds
          : state.selectedSegmentId
          ? [state.selectedSegmentId]
          : [];

      if (targetIds.length === 0) {
        return;
      }

      // Prevent default browser action
      keyEvent.preventDefault();

      // Dispatch speaker update
      state.updateSegmentsSpeaker(targetIds, targetRole);

      if (onSpeakerChange) {
        onSpeakerChange(targetRole, targetIds);
      }
    };

    target.addEventListener('keydown', handleKeyDown as EventListener);
    return () => {
      target.removeEventListener('keydown', handleKeyDown as EventListener);
    };
  }, [enabled, targetElement, onSpeakerChange]);

  return {
    enabled,
    reassignSelectedSpeaker,
  };
}
