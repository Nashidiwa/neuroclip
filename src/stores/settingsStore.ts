/**
 * NeuroClip Reactive Settings & Hardware Profile Store
 * Compatible with Zustand API: useSettingsStore(selector), getState(), setState(), subscribe()
 * Strict adherence to docs/CONVENTIONS.md §3.3 & §5.3 (zero-any, exactOptionalPropertyTypes).
 */

import { useSyncExternalStore } from 'react';
import {
  type AppConfig,
  DEFAULT_APP_CONFIG,
  fetchAppConfig,
  saveAppConfig,
} from '../services/config/settings.ts';
import {
  type HardwareProfile,
  fetchHardwareProfile,
  detectHardwareProfile,
} from '../services/config/hardware.ts';
import { parseIpcError, getFriendlyErrorMessage } from '../services/ipc/error.ts';

// ---------------------------------------------------------------------------
// Store Interface Contract
// ---------------------------------------------------------------------------

export interface SettingsState {
  // Reactive State
  readonly config: AppConfig;
  readonly hardwareProfile: HardwareProfile | null;
  readonly isLoading: boolean;
  readonly isSaving: boolean;
  readonly error: string | null;
  readonly lastSavedAt: string | null;

  // Actions
  loadConfig: () => Promise<void>;
  updateConfig: (partial: Partial<AppConfig>) => void;
  saveConfig: () => Promise<boolean>;
  loadHardwareProfile: () => Promise<void>;
  detectHardware: () => Promise<HardwareProfile | null>;
  resetDefaults: () => void;
  clearError: () => void;
}

// ---------------------------------------------------------------------------
// Re-export Defaults for Convenience
// ---------------------------------------------------------------------------

export { DEFAULT_APP_CONFIG };

// ---------------------------------------------------------------------------
// Type-Safe Reactive Store Core (Zustand-Compatible Architecture)
// ---------------------------------------------------------------------------

type SetStateFn<T> = (
  partial: Partial<T> | ((state: T) => Partial<T>),
) => void;
type GetStateFn<T> = () => T;
type Listener<T> = (state: T, prevState: T) => void;

interface StoreApi<T> {
  getState: GetStateFn<T>;
  setState: SetStateFn<T>;
  subscribe: (listener: Listener<T>) => () => void;
}

type StoreHook<T> = {
  (): T;
  <U>(selector: (state: T) => U): U;
} & StoreApi<T>;

function createStore<T extends object>(
  initializer: (set: SetStateFn<T>, get: GetStateFn<T>) => T,
): StoreHook<T> {
  let state: T;
  const listeners = new Set<Listener<T>>();

  const getState: GetStateFn<T> = () => state;

  const setState: SetStateFn<T> = (partial) => {
    const prevState = state;
    const nextState =
      typeof partial === 'function'
        ? { ...prevState, ...partial(prevState) }
        : { ...prevState, ...partial };

    if (!Object.is(prevState, nextState)) {
      state = nextState;
      listeners.forEach((listener) => {
        listener(state, prevState);
      });
    }
  };

  const subscribe = (listener: Listener<T>): (() => void) => {
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  };

  state = initializer(setState, getState);

  const useStore = (<U>(selector?: (state: T) => U): T | U => {
    return useSyncExternalStore(
      (notify) => subscribe(notify),
      () => {
        const cur = getState();
        return selector ? selector(cur) : cur;
      },
      () => {
        const cur = getState();
        return selector ? selector(cur) : cur;
      },
    );
  }) as StoreHook<T>;

  useStore.getState = getState;
  useStore.setState = setState;
  useStore.subscribe = subscribe;

  return useStore;
}

// ---------------------------------------------------------------------------
// Settings Store Instance
// ---------------------------------------------------------------------------

export const useSettingsStore = createStore<SettingsState>((set, get) => ({
  config: { ...DEFAULT_APP_CONFIG },
  hardwareProfile: null,
  isLoading: false,
  isSaving: false,
  error: null,
  lastSavedAt: null,

  loadConfig: async () => {
    set({ isLoading: true, error: null });
    try {
      const config = await fetchAppConfig();
      set({ config, isLoading: false });
    } catch (err: unknown) {
      const parsed = parseIpcError(err, 'Không thể tải cấu hình cài đặt');
      set({ error: getFriendlyErrorMessage(parsed), isLoading: false });
    }
  },

  updateConfig: (partial: Partial<AppConfig>) => {
    const current = get().config;
    const updated: AppConfig = {
      ...current,
      ...partial,
      sidecarPaths: partial.sidecarPaths
        ? { ...current.sidecarPaths, ...partial.sidecarPaths }
        : current.sidecarPaths,
    };
    set({ config: updated, error: null });
  },

  saveConfig: async (): Promise<boolean> => {
    if (get().isSaving) {
      return false;
    }

    const current = get().config;
    set({ isSaving: true, error: null });
    try {
      await saveAppConfig(current);
      set({
        isSaving: false,
        lastSavedAt: new Date().toISOString(),
      });
      return true;
    } catch (err: unknown) {
      const parsed = parseIpcError(err, 'Không thể lưu cấu hình cài đặt');
      set({
        error: getFriendlyErrorMessage(parsed),
        isSaving: false,
      });
      return false;
    }
  },

  loadHardwareProfile: async () => {
    set({ isLoading: true, error: null });
    try {
      const profile = await fetchHardwareProfile();
      set({ hardwareProfile: profile, isLoading: false });
    } catch (err: unknown) {
      const parsed = parseIpcError(err, 'Không thể tải thông số phần cứng');
      set({ error: getFriendlyErrorMessage(parsed), isLoading: false });
    }
  },

  detectHardware: async (): Promise<HardwareProfile | null> => {
    set({ isLoading: true, error: null });
    try {
      const profile = await detectHardwareProfile();
      set({ hardwareProfile: profile, isLoading: false });
      return profile;
    } catch (err: unknown) {
      const parsed = parseIpcError(err, 'Không thể đánh giá thông số phần cứng');
      set({
        hardwareProfile: null,
        error: getFriendlyErrorMessage(parsed),
        isLoading: false,
      });
      return null;
    }
  },

  resetDefaults: () => {
    set({
      config: { ...DEFAULT_APP_CONFIG },
      error: null,
    });
  },

  clearError: () => {
    set({ error: null });
  },
}));
