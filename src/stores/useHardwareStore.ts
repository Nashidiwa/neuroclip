/**
 * NeuroClip Hardware Profile & Mode Recommendation Store
 * Manages machine hardware probe results, recommended workflow modes,
 * and user override selections during Onboarding and in Settings.
 *
 * Conforms 100% to docs/ARCHITECTURE.md §5.1 (Schema hardware_profile.json)
 * and docs/CONVENTIONS.md §3.3 (Zustand-compatible via createStore, zero-any).
 */

import { createStore } from './createStore.ts';
import {
  type HardwareProfile,
  type RecommendedMode,
  fetchHardwareProfile,
  detectHardwareProfile,
} from '../services/config/hardware.ts';
import { parseIpcError, getFriendlyErrorMessage } from '../services/ipc/error.ts';
import { useSettingsStore } from './settingsStore.ts';

export interface HardwareState {
  // Reactive State
  readonly profile: HardwareProfile | null;
  readonly selectedMode: RecommendedMode | null;
  readonly isOverridden: boolean;
  readonly isLoading: boolean;
  readonly error: string | null;
  readonly lastProbedAt: string | null;

  // Actions
  fetchProfile: () => Promise<HardwareProfile | null>;
  detectHardware: () => Promise<HardwareProfile | null>;
  selectModeOverride: (mode: RecommendedMode) => void;
  resetOverride: () => void;
  applyToSettings: (mode?: RecommendedMode) => void;
  clearError: () => void;
  reset: () => void;
}

const INITIAL_STATE = {
  profile: null,
  selectedMode: null,
  isOverridden: false,
  isLoading: false,
  error: null,
  lastProbedAt: null,
} as const;

export function mapModeToSttProvider(
  mode: RecommendedMode,
): 'local_whisper' | 'groq' | 'cloud_byok' {
  switch (mode) {
    case 'offline_local':
      return 'local_whisper';
    case 'hybrid':
      return 'groq';
    case 'cloud_byok':
      return 'cloud_byok';
  }
}

export const useHardwareStore = createStore<HardwareState>((set, get) => {
  let currentRequestId = 0;

  return {
    ...INITIAL_STATE,

    fetchProfile: async (): Promise<HardwareProfile | null> => {
      const requestId = ++currentRequestId;
      set({ isLoading: true, error: null });
      try {
        const profile = await fetchHardwareProfile();
        if (requestId !== currentRequestId) {
          return profile;
        }
        if (profile !== null) {
          const currentSelected = get().selectedMode;
          const currentOverridden = get().isOverridden;
          const selectedMode =
            currentOverridden && currentSelected ? currentSelected : profile.recommendedMode;
          const isOverridden =
            currentOverridden && currentSelected
              ? currentSelected !== profile.recommendedMode
              : false;
          set({
            profile,
            selectedMode,
            isOverridden,
            lastProbedAt: profile.timestamp,
            isLoading: false,
          });
        } else {
          set({ isLoading: false });
        }
        return profile;
      } catch (err: unknown) {
        if (requestId !== currentRequestId) {
          return null;
        }
        const parsed = parseIpcError(err, 'Không thể tải thông số phần cứng');
        const friendly = getFriendlyErrorMessage(parsed);
        const error =
          parsed.code === 'ERR_INTERNAL_ERROR'
            ? `Không thể tải thông số phần cứng: ${parsed.message}`
            : friendly;
        set({
          error,
          isLoading: false,
        });
        return null;
      }
    },

    detectHardware: async (): Promise<HardwareProfile | null> => {
      const requestId = ++currentRequestId;
      set({ isLoading: true, error: null });
      try {
        const profile = await detectHardwareProfile();
        if (requestId !== currentRequestId) {
          return profile;
        }
        const currentSelected = get().selectedMode;
        const currentOverridden = get().isOverridden;
        const selectedMode =
          currentOverridden && currentSelected ? currentSelected : profile.recommendedMode;
        const isOverridden =
          currentOverridden && currentSelected
            ? currentSelected !== profile.recommendedMode
            : false;
        set({
          profile,
          selectedMode,
          isOverridden,
          lastProbedAt: profile.timestamp,
          isLoading: false,
        });
        return profile;
      } catch (err: unknown) {
        if (requestId !== currentRequestId) {
          return null;
        }
        const parsed = parseIpcError(err, 'Không thể quét thông số phần cứng');
        const friendly = getFriendlyErrorMessage(parsed);
        const error =
          parsed.code === 'ERR_INTERNAL_ERROR'
            ? `Không thể quét thông số phần cứng: ${parsed.message}`
            : friendly;
        set({
          error,
          isLoading: false,
        });
        return null;
      }
    },

    selectModeOverride: (mode: RecommendedMode): void => {
      const profile = get().profile;
      const isOverridden = profile !== null ? mode !== profile.recommendedMode : true;
      set({
        selectedMode: mode,
        isOverridden,
        error: null,
      });
    },

    resetOverride: (): void => {
      const profile = get().profile;
      if (profile !== null) {
        set({
          selectedMode: profile.recommendedMode,
          isOverridden: false,
        });
      } else {
        set({
          selectedMode: null,
          isOverridden: false,
        });
      }
    },

    applyToSettings: (modeOverride?: RecommendedMode): void => {
      const mode = modeOverride ?? get().selectedMode;
      if (mode !== null) {
        const activeSttProvider = mapModeToSttProvider(mode);
        useSettingsStore.getState().updateConfig({ activeSttProvider });
      }
    },

    clearError: (): void => {
      set({ error: null });
    },

    reset: (): void => {
      currentRequestId++;
      set({ ...INITIAL_STATE });
    },
  };
});
