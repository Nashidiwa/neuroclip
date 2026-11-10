import { describe, it, expect, beforeEach } from 'vitest';
import {
  useLogStore,
  selectFilteredLogs,
  DEFAULT_MAX_LOG_ENTRIES,
} from '../stores/useLogStore.ts';
import { type LogEntry, type LogEntryJson } from '../services/logging/types.ts';

describe('Log Viewer Reactive Store & Ring Buffer (useLogStore)', () => {
  beforeEach(() => {
    useLogStore.getState().reset();
  });

  describe('Initial State & Defaults', () => {
    it('should initialize with empty entries and default filter', () => {
      const state = useLogStore.getState();
      expect(state.entries).toEqual([]);
      expect(state.maxEntries).toBe(DEFAULT_MAX_LOG_ENTRIES);
      expect(state.isLoading).toBe(false);
      expect(state.isClearing).toBe(false);
      expect(state.error).toBeNull();
      expect(state.filter.level).toBe('all');
      expect(state.filter.search).toBe('');
      expect(state.autoScroll).toBe(true);
      expect(state.autoRefresh).toBe(true);
    });
  });

  describe('Ring Buffer Capacity & Eviction', () => {
    it('should append log entries up to capacity', () => {
      const entry: LogEntry = {
        id: 'log-1',
        timestamp: '2026-11-10T04:15:00.000Z',
        level: 'info',
        target: 'neuroclip::test',
        message: 'First test log',
      };

      useLogStore.getState().addLogEntry(entry);
      expect(useLogStore.getState().entries.length).toBe(1);
      expect(useLogStore.getState().entries[0]).toEqual(entry);
    });

    it('should convert raw LogEntryJson objects into domain LogEntry instances', () => {
      const rawJson: LogEntryJson = {
        timestamp: '2026-11-10T04:15:01.000Z',
        level: 'WARN',
        target: 'neuroclip::audio',
        message: 'Audio buffer low',
        fields: { bufferMs: 250 },
        line: 120,
        file: 'audio.rs',
      };

      useLogStore.getState().addLogEntry(rawJson);
      const entries = useLogStore.getState().entries;
      expect(entries.length).toBe(1);
      const saved = entries[0];
      expect(saved).toBeDefined();
      if (!saved) return;

      expect(saved.level).toBe('warn');
      expect(saved.target).toBe('neuroclip::audio');
      expect(saved.message).toBe('Audio buffer low');
      expect(saved.line).toBe(120);
      expect(saved.file).toBe('audio.rs');
      expect(saved.id.startsWith('log-')).toBe(true);
    });

    it('should evict oldest entry when capacity is reached (FIFO Ring Buffer)', () => {
      // Temporarily constrain maxEntries for deterministic unit test
      useLogStore.setState({ maxEntries: 5 });

      for (let i = 1; i <= 7; i++) {
        useLogStore.getState().addLogEntry({
          id: `log-${i}`,
          timestamp: `2026-11-10T04:15:0${i}.000Z`,
          level: 'info',
          target: 'test',
          message: `Message ${i}`,
        });
      }

      const entries = useLogStore.getState().entries;
      expect(entries.length).toBe(5);

      // Oldest logs (1 and 2) must be evicted; remaining must be 3..7
      expect(entries[0]?.id).toBe('log-3');
      expect(entries[4]?.id).toBe('log-7');
    });
  });

  describe('Filtering & Search Predicates (selectFilteredLogs)', () => {
    beforeEach(() => {
      const testEntries: LogEntry[] = [
        {
          id: 'log-1',
          timestamp: '2026-11-10T10:00:00.000Z',
          level: 'info',
          target: 'neuroclip::ingestion',
          message: 'Downloading Twitch VOD',
        },
        {
          id: 'log-2',
          timestamp: '2026-11-10T10:05:00.000Z',
          level: 'warn',
          target: 'neuroclip::stt',
          message: 'VAD silence detected in segment',
        },
        {
          id: 'log-3',
          timestamp: '2026-11-10T10:10:00.000Z',
          level: 'error',
          target: 'neuroclip::storage',
          message: 'Disk space warning on drive C:',
        },
        {
          id: 'log-4',
          timestamp: '2026-11-10T10:15:00.000Z',
          level: 'debug',
          target: 'neuroclip::ingestion',
          message: 'Chat replay frames decoded: 450',
        },
      ];

      useLogStore.setState({ entries: testEntries });
    });

    it('should return all logs when filter level is "all" and search is empty', () => {
      const state = useLogStore.getState();
      const filtered = selectFilteredLogs(state);
      expect(filtered.length).toBe(4);
    });

    it('should filter logs by exact level', () => {
      useLogStore.getState().setFilter({ level: 'error' });
      const filtered = selectFilteredLogs(useLogStore.getState());
      expect(filtered.length).toBe(1);
      expect(filtered[0]?.level).toBe('error');
      expect(filtered[0]?.id).toBe('log-3');
    });

    it('should filter logs by search text in message (case-insensitive)', () => {
      useLogStore.getState().setFilter({ search: 'twitch' });
      const filtered = selectFilteredLogs(useLogStore.getState());
      expect(filtered.length).toBe(1);
      expect(filtered[0]?.message).toContain('Twitch');
    });

    it('should filter logs by target module', () => {
      useLogStore.getState().setFilter({ target: 'ingestion' });
      const filtered = selectFilteredLogs(useLogStore.getState());
      expect(filtered.length).toBe(2);
      expect(filtered.every((e) => e.target.includes('ingestion'))).toBe(true);
    });

    it('should filter logs by time boundaries', () => {
      useLogStore.getState().setFilter({
        startTime: '2026-11-10T10:04:00.000Z',
        endTime: '2026-11-10T10:12:00.000Z',
      });
      const filtered = selectFilteredLogs(useLogStore.getState());
      expect(filtered.length).toBe(2);
      expect(filtered[0]?.id).toBe('log-2');
      expect(filtered[1]?.id).toBe('log-3');
    });

    it('should reset filter cleanly', () => {
      useLogStore.getState().setFilter({ level: 'error', search: 'drive' });
      expect(selectFilteredLogs(useLogStore.getState()).length).toBe(1);

      useLogStore.getState().resetFilter();
      expect(selectFilteredLogs(useLogStore.getState()).length).toBe(4);
    });
  });

  describe('Actions & State Modifiers', () => {
    it('should toggle autoScroll and autoRefresh flags', () => {
      useLogStore.getState().setAutoScroll(false);
      expect(useLogStore.getState().autoScroll).toBe(false);

      useLogStore.getState().setAutoRefresh(false);
      expect(useLogStore.getState().autoRefresh).toBe(false);
    });

    it('should clear logs and reset error on clearLogs call', async () => {
      useLogStore.setState({
        entries: [
          {
            id: 'log-1',
            timestamp: '2026-11-10T04:15:00.000Z',
            level: 'info',
            target: 'test',
            message: 'To be cleared',
          },
        ],
        error: 'Prior error',
      });

      const success = await useLogStore.getState().clearLogs();
      expect(success).toBe(true);
      expect(useLogStore.getState().entries).toEqual([]);
      expect(useLogStore.getState().error).toBeNull();
    });

    it('should clear error cleanly via clearError()', () => {
      useLogStore.setState({ error: 'Some error' });
      expect(useLogStore.getState().error).toBe('Some error');

      useLogStore.getState().clearError();
      expect(useLogStore.getState().error).toBeNull();
    });
  });
});
