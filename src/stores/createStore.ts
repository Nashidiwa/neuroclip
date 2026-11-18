/**
 * NeuroClip Shared Reactive Store Factory
 *
 * Built on native React `useSyncExternalStore` with Zustand-compatible API:
 * `createStore<T>(initializer)` (and curried `createStore<T>()(initializer)`),
 * `create` alias, `useStore(selector?, equalityFn?)`, `getState()`,
 * `getInitialState()`, `setState(partial, replace?)`, and `subscribe(listener)`.
 *
 * Designed for drop-in migration to `zustand` in TASK-M4-20+ without breaking callers.
 * Strict adherence to docs/CONVENTIONS.md (zero-any, exactOptionalPropertyTypes).
 */

import { useCallback, useRef, useSyncExternalStore } from 'react';

export type SetStateFn<T> = {
  (partial: T | Partial<T> | ((state: T) => T | Partial<T>), replace?: false | undefined): void;
  (state: T | ((state: T) => T), replace: true): void;
};

export type GetStateFn<T> = () => T;
export type Listener<T> = (state: T, prevState: T) => void;
export type EqualityFn<U> = (a: U, b: U) => boolean;

export interface StoreApi<T> {
  getState: GetStateFn<T>;
  getInitialState: GetStateFn<T>;
  setState: SetStateFn<T>;
  subscribe: (listener: Listener<T>) => () => void;
}

export type StateCreator<T> = (set: SetStateFn<T>, get: GetStateFn<T>, api: StoreApi<T>) => T;

export type StoreHook<T> = {
  (): T;
  <U>(selector: (state: T) => U, equalityFn?: EqualityFn<U> | undefined): U;
} & StoreApi<T>;

interface SnapshotCache<T, U> {
  hasValue: boolean;
  state: T;
  selector: ((state: T) => U) | undefined;
  equalityFn: EqualityFn<U>;
  value: T | U;
}

function createStoreImpl<T extends object>(initializer: StateCreator<T>): StoreHook<T> {
  let state: T;
  const listeners = new Set<Listener<T>>();

  const getState: GetStateFn<T> = () => state;

  const setState: SetStateFn<T> = (
    partial: T | Partial<T> | ((current: T) => T | Partial<T>),
    replace?: boolean,
  ): void => {
    const prevState = state;
    const nextPartial = typeof partial === 'function' ? partial(prevState) : partial;

    if (!Object.is(nextPartial, prevState)) {
      const nextState =
        replace === true ? (nextPartial as T) : Object.assign({}, prevState, nextPartial);

      if (!Object.is(prevState, nextState)) {
        state = nextState;
        listeners.forEach((listener) => {
          listener(state, prevState);
        });
      }
    }
  };

  const subscribe = (listener: Listener<T>): (() => void) => {
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  };

  const api: StoreApi<T> = {
    getState,
    getInitialState: () => initialState,
    setState,
    subscribe,
  };

  const initialState = initializer(setState, getState, api);
  state = initialState;

  const useStore = (<U>(
    selector?: (current: T) => U,
    equalityFn: EqualityFn<U> | undefined = Object.is,
  ): T | U => {
    const cacheRef = useRef<SnapshotCache<T, U> | null>(null);
    const resolvedEqualityFn = equalityFn ?? Object.is;

    const getSnapshot = useCallback((): T | U => {
      const currentState = getState();
      if (selector === undefined) {
        return currentState;
      }

      const cached = cacheRef.current;
      if (
        cached !== null &&
        cached.hasValue &&
        Object.is(cached.state, currentState) &&
        Object.is(cached.selector, selector) &&
        Object.is(cached.equalityFn, resolvedEqualityFn)
      ) {
        return cached.value;
      }

      const nextSlice = selector(currentState);
      if (cached !== null && cached.hasValue && resolvedEqualityFn(cached.value as U, nextSlice)) {
        cacheRef.current = {
          hasValue: true,
          state: currentState,
          selector,
          equalityFn: resolvedEqualityFn,
          value: cached.value,
        };
        return cached.value;
      }

      cacheRef.current = {
        hasValue: true,
        state: currentState,
        selector,
        equalityFn: resolvedEqualityFn,
        value: nextSlice,
      };
      return nextSlice;
    }, [selector, resolvedEqualityFn]);

    return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
  }) as StoreHook<T>;

  useStore.getState = getState;
  useStore.getInitialState = api.getInitialState;
  useStore.setState = setState;
  useStore.subscribe = subscribe;

  return useStore;
}

export function createStore<T extends object>(initializer: StateCreator<T>): StoreHook<T>;
export function createStore<T extends object>(): (initializer: StateCreator<T>) => StoreHook<T>;
export function createStore<T extends object>(
  initializer?: StateCreator<T>,
): StoreHook<T> | ((fn: StateCreator<T>) => StoreHook<T>) {
  if (initializer === undefined) {
    return (fn: StateCreator<T>) => createStoreImpl(fn);
  }
  return createStoreImpl(initializer);
}

/** Zustand `create` alias for seamless migration in TASK-M4-20+ */
export const create = createStore;
