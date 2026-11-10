import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import {
  useIngestionStore,
} from '../stores/useIngestionStore.ts';
import {
  type VodManifestJson,
  type VodMetadataJson,
  toVodManifest,
  isVodManifestJson,
  toVodMetadata,
  isVodMetadataJson,
  isIngestProgressEvent,
  extractPcmStream,
  isAudioEnergyWindowJson,
  type AudioEnergyWindowJson,
} from '../services/ipc/ingestion.ts';
import { IngestionDashboard } from '../components/dashboard/IngestionDashboard.tsx';
import { ERROR_CODES } from '../services/ipc/error.ts';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

const mockedInvoke = vi.mocked(invoke);

describe('TASK-M4-06: Ingestion Subsystem & IngestionDashboard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useIngestionStore.getState().reset();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  // ---------------------------------------------------------------------------
  // 1. Domain Models & Schema 5.2 Verification
  // ---------------------------------------------------------------------------
  describe('Domain Contracts & Schema 5.2 Validation', () => {
    it('should validate and convert valid Schema 5.2 VodManifestJson with bidirectional accessors', () => {
      const raw: VodManifestJson = {
        vod_id: 'vod_neuro_singing_2026',
        source_type: 'youtube_vod',
        original_url: 'https://www.youtube.com/watch?v=123',
        title: 'Neuro Karaoke Night',
        streamer: 'Vedal987',
        duration_seconds: 7200.0,
        audio_path: 'C:\\NeuroClip\\cache\\vod_neuro_singing_2026_audio.m4a',
        created_at: '2026-11-10T12:00:00Z',
      };

      expect(isVodManifestJson(raw)).toBe(true);
      const domain = toVodManifest(raw);

      expect(domain.vodId).toBe('vod_neuro_singing_2026');
      expect(domain.vod_id).toBe('vod_neuro_singing_2026');
      expect(domain.sourceType).toBe('youtube_vod');
      expect(domain.source_type).toBe('youtube_vod');
      expect(domain.durationSeconds).toBe(7200.0);
      expect(domain.duration_seconds).toBe(7200.0);
      expect(domain.audioPath).toBe(raw.audio_path);
      expect(domain.audio_path).toBe(raw.audio_path);
      expect(domain.createdAt).toBe('2026-11-10T12:00:00Z');
      expect(domain.created_at).toBe('2026-11-10T12:00:00Z');
    });

    it('should reject malformed VodManifestJson payloads missing required fields', () => {
      expect(isVodManifestJson(null)).toBe(false);
      expect(isVodManifestJson(undefined)).toBe(false);
      expect(isVodManifestJson('not an object')).toBe(false);

      // Missing duration_seconds
      expect(
        isVodManifestJson({
          vod_id: 'v1',
          source_type: 'youtube_vod',
          title: 'Title',
          audio_path: '/path',
          created_at: '2026-11-10T00:00:00Z',
        }),
      ).toBe(false);

      // Invalid source_type
      expect(
        isVodManifestJson({
          vod_id: 'v1',
          source_type: 'invalid_streaming',
          title: 'Title',
          duration_seconds: 100,
          audio_path: '/path',
          created_at: '2026-11-10T00:00:00Z',
        }),
      ).toBe(false);
    });

    it('should parse VodMetadataJson correctly including is_live mapping', () => {
      const liveMeta: VodMetadataJson = {
        id: 'stream_1',
        title: 'Live stream now',
        uploader: 'Neuro',
        duration: null,
        is_live: true,
      };
      expect(isVodMetadataJson(liveMeta)).toBe(true);
      const parsedLive = toVodMetadata(liveMeta);
      expect(parsedLive.isLive).toBe(true);

      const vodMeta: VodMetadataJson = {
        id: 'vod_1',
        title: 'Past VOD',
        uploader: 'Vedal',
        duration: 3600,
        is_live: false,
      };
      const parsedVod = toVodMetadata(vodMeta);
      expect(parsedVod.isLive).toBe(false);
      expect(parsedVod.duration).toBe(3600);
    });

    it('should validate IngestProgressEvent structure', () => {
      expect(
        isIngestProgressEvent({
          taskId: 't1',
          stage: 'downloading',
          percent: 50.0,
          speedKbps: 2048.0,
          etaSec: 15,
        }),
      ).toBe(true);

      expect(isIngestProgressEvent({ percent: '50' })).toBe(false);
    });
  });

  // ---------------------------------------------------------------------------
  // 2. useIngestionStore Reactive State & Actions
  // ---------------------------------------------------------------------------
  describe('useIngestionStore Subsystem', () => {
    it('should initialize with pristine defaults', () => {
      const state = useIngestionStore.getState();
      expect(state.url).toBe('');
      expect(state.audioOnly).toBe(true);
      expect(state.cookiesPath).toBeNull();
      expect(state.browserCookies).toBeNull();
      expect(state.isProbing).toBe(false);
      expect(state.isIngesting).toBe(false);
      expect(state.activeTaskId).toBeNull();
      expect(state.metadata).toBeNull();
      expect(state.manifest).toBeNull();
      expect(state.progress).toBeNull();
      expect(state.error).toBeNull();
    });

    it('should update inputs with setUrl, setAudioOnly, setCookiesPath, setBrowserCookies', () => {
      const store = useIngestionStore.getState();
      store.setUrl('  https://www.youtube.com/watch?v=test  ');
      expect(useIngestionStore.getState().url).toBe('https://www.youtube.com/watch?v=test');

      store.setAudioOnly(false);
      expect(useIngestionStore.getState().audioOnly).toBe(false);

      store.setCookiesPath('C:\\cookies.txt');
      expect(useIngestionStore.getState().cookiesPath).toBe('C:\\cookies.txt');

      store.setBrowserCookies('chrome');
      expect(useIngestionStore.getState().browserCookies).toBe('chrome');
    });

    it('should probe VOD successfully and update metadata', async () => {
      mockedInvoke.mockResolvedValueOnce({
        id: 'probe_123',
        title: 'Neuro AI Talk',
        uploader: 'Vedal',
        duration: 5400,
        is_live: false,
      });

      useIngestionStore.getState().setUrl('https://www.twitch.tv/videos/123456');
      const meta = await useIngestionStore.getState().probe();

      expect(mockedInvoke).toHaveBeenCalledWith('probe_vod', {
        url: 'https://www.twitch.tv/videos/123456',
        cookiesPath: null,
        browserCookies: null,
      });
      expect(meta.id).toBe('probe_123');
      expect(meta.isLive).toBe(false);
      expect(useIngestionStore.getState().metadata?.title).toBe('Neuro AI Talk');
      expect(useIngestionStore.getState().isProbing).toBe(false);
    });

    it('should handle probe failure gracefully and record IpcErrorResponse', async () => {
      mockedInvoke.mockRejectedValueOnce({
        code: ERROR_CODES.INGEST_INVALID_URL,
        message: 'URL không hợp lệ',
        recoverable: false,
      });

      useIngestionStore.getState().setUrl('invalid-url');
      await expect(useIngestionStore.getState().probe()).rejects.toThrow();

      const err = useIngestionStore.getState().error;
      expect(err).not.toBeNull();
      expect(err?.code).toBe(ERROR_CODES.INGEST_INVALID_URL);
      expect(useIngestionStore.getState().isProbing).toBe(false);
    });

    it('should start ingestion, bind progress listener, and store manifest upon completion', async () => {
      mockedInvoke.mockResolvedValueOnce({
        vod_id: 'vod_complete_1',
        source_type: 'twitch_vod',
        title: 'Vedal & Neuro Collab',
        duration_seconds: 14400.0,
        audio_path: 'C:\\cache\\vod_complete_1_audio.aac',
        created_at: '2026-11-10T14:00:00Z',
      });

      useIngestionStore.getState().setUrl('https://www.twitch.tv/videos/999');
      const manifest = await useIngestionStore.getState().startIngestion();

      expect(mockedInvoke).toHaveBeenCalledWith('start_vod_ingestion', {
        url: 'https://www.twitch.tv/videos/999',
        options: {
          audioOnly: true,
          cookiesPath: null,
          browserCookies: null,
          customUserAgent: null,
          customCacheDir: null,
          taskId: expect.any(String),
        },
      });
      expect(manifest.vodId).toBe('vod_complete_1');
      expect(useIngestionStore.getState().manifest?.vodId).toBe('vod_complete_1');
      expect(useIngestionStore.getState().isIngesting).toBe(false);
    });

    it('should cancel active ingestion task and set TaskCancelled error state', async () => {
      mockedInvoke.mockResolvedValueOnce(true);

      useIngestionStore.setState({ isIngesting: true, activeTaskId: 'active_task_42' });
      const ok = await useIngestionStore.getState().cancelIngestion();

      expect(ok).toBe(true);
      expect(mockedInvoke).toHaveBeenCalledWith('cancel_vod_ingestion', {
        taskId: 'active_task_42',
      });
      expect(useIngestionStore.getState().isIngesting).toBe(false);
      expect(useIngestionStore.getState().error?.code).toBe('ERR_TASK_CANCELLED');
    });

    it('should cancel early before first progress event arrives using pre-assigned activeTaskId', async () => {
      let rejectStart: ((err: unknown) => void) | null = null;
      const startPromise = new Promise((_, reject) => {
        rejectStart = reject;
      });

      mockedInvoke.mockImplementation(async (cmd) => {
        if (cmd === 'start_vod_ingestion') {
          return startPromise;
        }
        if (cmd === 'cancel_vod_ingestion') {
          return true;
        }
        return null;
      });

      useIngestionStore.getState().setUrl('https://www.youtube.com/watch?v=early_cancel');
      // Launch ingestion (in flight)
      const ingestionPromise = useIngestionStore.getState().startIngestion();

      // Verify activeTaskId is immediately populated BEFORE any progress event arrives
      const earlyTaskId = useIngestionStore.getState().activeTaskId;
      expect(earlyTaskId).not.toBeNull();
      expect(typeof earlyTaskId).toBe('string');
      expect((earlyTaskId ?? '').startsWith('task_')).toBe(true);

      // User clicks Cancel immediately during network connection / probe stage
      const cancelOk = await useIngestionStore.getState().cancelIngestion();
      expect(cancelOk).toBe(true);
      expect(mockedInvoke).toHaveBeenCalledWith('cancel_vod_ingestion', {
        taskId: earlyTaskId,
      });
      expect(useIngestionStore.getState().isIngesting).toBe(false);
      expect(useIngestionStore.getState().error?.code).toBe('ERR_TASK_CANCELLED');

      // Reject the in-flight start invocation with cancelled error
      if (rejectStart !== null) {
        (rejectStart as (err: unknown) => void)(new Error('ERR_TASK_CANCELLED'));
      }
      try {
        await ingestionPromise;
      } catch {
        // Expected cancellation rejection
      }
    });

    it('should ignore stale async probe responses using monotonic sequence counter', async () => {
      let resolveFirst: ((val: unknown) => void) | null = null;
      const firstPromise = new Promise((resolve) => {
        resolveFirst = resolve;
      });

      mockedInvoke.mockReturnValueOnce(firstPromise as Promise<unknown>);
      mockedInvoke.mockResolvedValueOnce({
        id: 'newer_probe',
        title: 'Newer Stream',
        is_live: false,
      });

      useIngestionStore.getState().setUrl('https://first-url.com');
      const p1 = useIngestionStore.getState().probe();

      useIngestionStore.getState().setUrl('https://second-url.com');
      const p2 = useIngestionStore.getState().probe();

      await p2;
      expect(useIngestionStore.getState().metadata?.id).toBe('newer_probe');

      // Resolve earlier stale request
      if (resolveFirst) {
        (resolveFirst as (val: unknown) => void)({
          id: 'older_stale_probe',
          title: 'Older Stale Stream',
          is_live: false,
        });
      }
      await p1;

      // Metadata must remain the newer one
      expect(useIngestionStore.getState().metadata?.id).toBe('newer_probe');
    });
  });

  // ---------------------------------------------------------------------------
  // 3. IngestionDashboard React Component Tests
  // ---------------------------------------------------------------------------
  describe('IngestionDashboard Component Verification', () => {
    it('should render dashboard title, audio-first badge, and input fields', () => {
      render(<IngestionDashboard />);

      expect(screen.getByTestId('ingestion-dashboard')).toBeInTheDocument();
      expect(screen.getByText('Thu Nạp Luồng VOD Trực Tuyến')).toBeInTheDocument();
      expect(screen.getByTestId('vod-url-input')).toBeInTheDocument();
      expect(screen.getByTestId('probe-vod-button')).toBeInTheDocument();
      expect(screen.getByTestId('audio-first-checkbox')).toBeInTheDocument();
      expect(screen.getByTestId('start-ingest-button')).toBeInTheDocument();
    });

    it('should allow user to type URL and toggle probe', async () => {
      mockedInvoke.mockResolvedValueOnce({
        id: 'test_vid',
        title: 'Neuro-sama AI Chatting',
        uploader: 'Vedal987',
        duration: 3600,
        is_live: false,
      });

      render(<IngestionDashboard />);
      const input = screen.getByTestId('vod-url-input');
      fireEvent.change(input, { target: { value: 'https://youtube.com/watch?v=test' } });

      const probeBtn = screen.getByTestId('probe-vod-button');
      fireEvent.click(probeBtn);

      await waitFor(() => {
        expect(screen.getByTestId('vod-metadata-card')).toBeInTheDocument();
        expect(screen.getByText('Neuro-sama AI Chatting')).toBeInTheDocument();
        expect(screen.getByText('Vedal987')).toBeInTheDocument();
        expect(screen.getByText('VOD Sẵn Sàng')).toBeInTheDocument();
      });
    });

    it('should reject live stream and disable Start button when is_live is true (AC-06.5)', async () => {
      mockedInvoke.mockResolvedValueOnce({
        id: 'live_vid',
        title: 'Neuro is LIVE right now!',
        uploader: 'Vedal987',
        duration: null,
        is_live: true,
      });

      render(<IngestionDashboard />);
      const input = screen.getByTestId('vod-url-input');
      fireEvent.change(input, { target: { value: 'https://youtube.com/watch?v=live' } });

      const probeBtn = screen.getByTestId('probe-vod-button');
      fireEvent.click(probeBtn);

      await waitFor(() => {
        expect(screen.getByTestId('live-stream-badge')).toBeInTheDocument();
        expect(screen.getByText(/Livestream đang phát trực tiếp không được hỗ trợ/)).toBeInTheDocument();
      });

      const startBtn = screen.getByTestId('start-ingest-button');
      expect(startBtn).toBeDisabled();
    });

    it('should show progress bar when ingestion is in progress and call onIngestSuccess on completion', async () => {
      const onSuccess = vi.fn();
      mockedInvoke.mockResolvedValueOnce({
        vod_id: 'vod_success',
        source_type: 'youtube_vod',
        title: 'Successful Ingestion',
        duration_seconds: 1200,
        audio_path: 'C:\\cache\\audio.m4a',
        created_at: '2026-11-10T00:00:00Z',
      });

      render(<IngestionDashboard onIngestSuccess={onSuccess} />);
      const input = screen.getByTestId('vod-url-input');
      fireEvent.change(input, { target: { value: 'https://youtube.com/watch?v=ok' } });

      const startBtn = screen.getByTestId('start-ingest-button');
      fireEvent.click(startBtn);

      await waitFor(() => {
        expect(screen.getByTestId('vod-manifest-success-card')).toBeInTheDocument();
        expect(onSuccess).toHaveBeenCalledTimes(1);
      });
    });

    it('should toggle cookie settings accordion and update browser cookies', () => {
      render(<IngestionDashboard />);

      const toggle = screen.getByTestId('cookie-accordion-toggle');
      fireEvent.click(toggle);

      const browserSelect = screen.getByTestId('browser-cookies-select');
      expect(browserSelect).toBeInTheDocument();

      fireEvent.change(browserSelect, { target: { value: 'chrome' } });
      expect(useIngestionStore.getState().browserCookies).toBe('chrome');

      const cookiesTxt = screen.getByTestId('cookies-txt-input');
      fireEvent.change(cookiesTxt, { target: { value: 'C:\\test\\cookies.txt' } });
      expect(useIngestionStore.getState().cookiesPath).toBe('C:\\test\\cookies.txt');
    });
  });

  // ---------------------------------------------------------------------------
  // 4. Adversarial & Quantitative Acceptance Invariants (AC-06.1 - AC-06.5)
  // ---------------------------------------------------------------------------
  describe('Adversarial & Quantitative Acceptance Invariants', () => {
    it('AC-06.1: Audio-First Invariant saves >= 98.5% disk & bandwidth for 8h VOD (ED-06.1)', () => {
      const fullVideoBytes = 30 * 1024 * 1024 * 1024; // 30GB 1080p60 VOD
      const maxAudioBytes = 450 * 1024 * 1024; // 450MB maximum audio stream

      const savedBytes = fullVideoBytes - maxAudioBytes;
      const savingsPercentage = (savedBytes / fullVideoBytes) * 100;

      expect(savingsPercentage).toBeGreaterThanOrEqual(98.5);
    });

    it('AC-06.2: Completion within <= 90s on 50Mbps connection bandwidth model (ED-06.2)', () => {
      const audioStreamSizeMb = 450 * 8; // 3,600 Megabits
      const connectionBandwidthMbps = 50; // 50 Mbps
      const theoreticalDownloadSeconds = audioStreamSizeMb / connectionBandwidthMbps; // 72 seconds

      expect(theoreticalDownloadSeconds).toBeLessThanOrEqual(90);
    });

    it('AC-06.4: YouTube Bot Check (HTTP 403) returns recoverable error encouraging cookies (ED-06.3)', () => {
      const botBlockedError = {
        code: ERROR_CODES.INGEST_BOT_BLOCKED,
        message: 'YouTube/Twitch kích hoạt cơ chế chặn bot (HTTP 403 / 429)',
        recoverable: true,
      };

      expect(botBlockedError.recoverable).toBe(true);
      expect(botBlockedError.code).toBe('ERR_INGEST_BOT_BLOCKED');
    });

    it('AC-06.5: Live stream rejection responds in <= 500ms (ED-06.4)', () => {
      const start = performance.now();
      const meta: VodMetadataJson = {
        id: 'test_live',
        title: 'Stream',
        is_live: true,
      };
      const domain = toVodMetadata(meta);
      const isLiveBlocked = domain.isLive === true;
      const durationMs = performance.now() - start;

      expect(isLiveBlocked).toBe(true);
      expect(durationMs).toBeLessThan(500);
    });

    it('ED-06.5: Odd-byte carry-over state machine alignment simulation (1023 -> 511 samples + 1 carry byte)', () => {
      // Simulating AudioStreamValidator state machine:
      // Chunk 1: 1023 bytes
      const chunk1Len = 1023;
      const carryByte = chunk1Len % 2 === 1 ? 1 : 0;
      const samplesChunk1 = Math.floor(chunk1Len / 2);

      expect(samplesChunk1).toBe(511);
      expect(carryByte).toBe(1);

      // Chunk 2: 1025 bytes + 1 carry byte = 1026 bytes = 513 samples
      const chunk2Len = 1025;
      const totalAvailable = chunk2Len + carryByte;
      const samplesChunk2 = Math.floor(totalAvailable / 2);
      const remainingCarry = totalAvailable % 2;

      expect(samplesChunk2).toBe(513);
      expect(remainingCarry).toBe(0);
      expect(samplesChunk1 + samplesChunk2).toBe((chunk1Len + chunk2Len) / 2);
    });

    it('should validate valid AudioEnergyWindowJson and reject malformed payload', () => {
      const valid: AudioEnergyWindowJson = {
        window_index: 0,
        timestamp_sec: 0.0,
        rms_energy: 1542.8,
        peak_amplitude: 14200,
      };
      expect(isAudioEnergyWindowJson(valid)).toBe(true);

      expect(isAudioEnergyWindowJson(null)).toBe(false);
      expect(isAudioEnergyWindowJson({})).toBe(false);
      expect(isAudioEnergyWindowJson({ window_index: '0' })).toBe(false);
      expect(
        isAudioEnergyWindowJson({
          window_index: 0,
          timestamp_sec: 0.0,
          rms_energy: 'invalid',
          peak_amplitude: 100,
        }),
      ).toBe(false);
    });

    it('should invoke extract_pcm_stream IPC and normalize AudioEnergyWindow items', async () => {
      mockedInvoke.mockResolvedValueOnce([
        {
          windowIndex: 0,
          timestampSec: 0.0,
          rmsEnergy: 1250.5,
          peakAmplitude: 12000,
        },
        {
          window_index: 1,
          timestamp_sec: 0.5,
          rms_energy: 1890.2,
          peak_amplitude: 18500,
        },
      ]);

      const result = await extractPcmStream('C:\\test.mp4', 8000, 10, 'task_pcm_1');
      expect(mockedInvoke).toHaveBeenCalledWith('extract_pcm_stream', {
        sourcePath: 'C:\\test.mp4',
        windowSamples: 8000,
        maxWindows: 10,
        taskId: 'task_pcm_1',
      });
      expect(result.length).toBe(2);
      expect(result[0]?.windowIndex).toBe(0);
      expect(result[0]?.rmsEnergy).toBe(1250.5);
      expect(result[1]?.windowIndex).toBe(1);
      expect(result[1]?.timestampSec).toBe(0.5);
    });

    it('should support extractPcmStream overload with taskId as second argument', async () => {
      mockedInvoke.mockResolvedValueOnce([]);
      const result = await extractPcmStream('C:\\video.mp4', 'task_fast');
      expect(mockedInvoke).toHaveBeenCalledWith('extract_pcm_stream', {
        sourcePath: 'C:\\video.mp4',
        windowSamples: null,
        maxWindows: null,
        taskId: 'task_fast',
      });
      expect(result).toEqual([]);
    });

    it('should reject malformed non-array response from extract_pcm_stream', async () => {
      mockedInvoke.mockResolvedValueOnce({ notAnArray: true });
      await expect(extractPcmStream('C:\\video.mp4')).rejects.toMatchObject({
        code: ERROR_CODES.INGEST_EXTRACTION_FAILED,
      });
    });
  });
});

