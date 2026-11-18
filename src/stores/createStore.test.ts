import { describe, it, expect, vi } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { create, createStore } from './createStore.ts';

interface CounterState {
  count: number;
  label: string;
  extra?: number;
  inc: () => void;
}

describe('createStore (Zustand-compatible native store factory)', () => {
  it('supports uncurried and curried createStore and create alias', () => {
    const useUncurried = createStore<CounterState>((set) => ({
      count: 0,
      label: 'init',
      inc: () => set((s) => ({ count: s.count + 1 })),
    }));

    const useCurried = create<CounterState>()((set) => ({
      count: 10,
      label: 'curried',
      inc: () => set((s) => ({ count: s.count + 1 })),
    }));

    expect(useUncurried.getState().count).toBe(0);
    expect(useCurried.getState().count).toBe(10);

    useUncurried.getState().inc();
    useCurried.getState().inc();

    expect(useUncurried.getState().count).toBe(1);
    expect(useCurried.getState().count).toBe(11);
  });

  it('returns the immutable initial snapshot from getInitialState() after mutations', () => {
    const useCounter = createStore<CounterState>((set) => ({
      count: 5,
      label: 'initial',
      inc: () => set((s) => ({ count: s.count + 1 })),
    }));

    const initial = useCounter.getInitialState();
    expect(initial.count).toBe(5);
    expect(initial.label).toBe('initial');

    useCounter.setState({ count: 99, label: 'mutated' });
    expect(useCounter.getState().count).toBe(99);
    expect(useCounter.getInitialState().count).toBe(5);
    expect(useCounter.getInitialState().label).toBe('initial');
  });

  it('replaces state completely when setState(nextState, true) is called', () => {
    const useCounter = createStore<CounterState>((set) => ({
      count: 1,
      label: 'first',
      extra: 42,
      inc: () => set((s) => ({ count: s.count + 1 })),
    }));

    const replacement: CounterState = {
      count: 2,
      label: 'replaced',
      inc: useCounter.getState().inc,
    };

    useCounter.setState(replacement, true);
    const current = useCounter.getState();
    expect(current.count).toBe(2);
    expect(current.label).toBe('replaced');
    expect('extra' in current).toBe(false);
  });

  it('memoizes derived selector snapshots and respects custom equalityFn', () => {
    const useCounter = createStore<CounterState>((set) => ({
      count: 1,
      label: 'alpha',
      inc: () => set((s) => ({ count: s.count + 1 })),
    }));

    let renderCount = 0;
    const parityEq = (a: { parity: number }, b: { parity: number }): boolean =>
      a.parity === b.parity;

    const { result } = renderHook(() => {
      renderCount += 1;
      return useCounter((s) => ({ parity: s.count % 2 }), parityEq);
    });

    expect(result.current).toEqual({ parity: 1 });
    const initialRenders = renderCount;
    const firstSnapshot = result.current;

    // Update label only: parity remains 1, so equalityFn prevents re-render
    act(() => {
      useCounter.setState({ label: 'beta' });
    });
    expect(renderCount).toBe(initialRenders);
    expect(Object.is(result.current, firstSnapshot)).toBe(true);

    // Update count from 1 -> 3: parity is still 1
    act(() => {
      useCounter.setState({ count: 3 });
    });
    expect(renderCount).toBe(initialRenders);
    expect(Object.is(result.current, firstSnapshot)).toBe(true);

    // Update count from 3 -> 4: parity changes to 0 -> triggers re-render
    act(() => {
      useCounter.setState({ count: 4 });
    });
    expect(renderCount).toBe(initialRenders + 1);
    expect(result.current).toEqual({ parity: 0 });
  });

  it('notifies subscribers with (state, prevState) and supports unsubscribe', () => {
    const useCounter = createStore<CounterState>((set) => ({
      count: 0,
      label: 'a',
      inc: () => set((s) => ({ count: s.count + 1 })),
    }));

    const listener = vi.fn();
    const unsub = useCounter.subscribe(listener);

    useCounter.getState().inc();
    expect(listener).toHaveBeenCalledTimes(1);
    expect(listener.mock.calls[0]?.[0].count).toBe(1);
    expect(listener.mock.calls[0]?.[1].count).toBe(0);

    unsub();
    useCounter.getState().inc();
    expect(listener).toHaveBeenCalledTimes(1);
  });
});
