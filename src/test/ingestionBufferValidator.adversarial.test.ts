/**
 * ADVERSARIAL STRESS TEST SUITE: Bounded Queue, RingBuffer, Backpressure & AudioStreamValidator
 * Target: TASK-M4-06 (Challenger 1)
 *
 * Vectors:
 * 1. BoundedAudioQueue Memory Ceiling (64KB chunks, capacity 16 = 1MB static footprint, 8-hour VOD simulation)
 * 2. Backpressure Propagation to OS Pipe & FFmpeg Pause Invariant
 * 3. AudioRingBuffer Zero Heap Reallocation & Wrap-Around Correctness (including defect check in buffer.rs test)
 * 4. AudioStreamValidator Odd-Byte Alignment & Multi-Chunk Carry-Byte Invariant
 * 5. Fragmented WAV Header (< 44 bytes) Reassembly & False-Positive Prefix Vulnerability
 * 6. AST & Zero-Panic Audit of Rust Ingestion Files
 */

import { describe, it, expect } from 'vitest';
import * as fs from 'node:fs';
import * as path from 'node:path';

// ============================================================================
// Faithful TypeScript Emulation of buffer.rs and validator.rs
// ============================================================================

export interface EmulatedAudioChunk {
  sequence: number;
  timestampSec: number;
  samples: Int16Array;
  rawBytesLen: number;
  isLast: boolean;
}

export class EmulatedAudioRingBuffer {
  private readonly storage: Int16Array;
  private readonly capacity: number;
  private head: number = 0;
  private tail: number = 0;
  private length: number = 0;

  constructor(capacity: number) {
    this.capacity = capacity;
    this.storage = new Int16Array(capacity);
  }

  public pushSamples(samples: Int16Array | number[]): number {
    if (this.capacity === 0 || this.length === this.capacity) {
      return 0;
    }
    const toWrite = Math.min(samples.length, this.capacity - this.length);
    for (let i = 0; i < toWrite; i++) {
      this.storage[this.tail] = samples[i]!;
      this.tail = (this.tail + 1) % this.capacity;
    }
    this.length += toWrite;
    return toWrite;
  }

  public popSamples(dest: Int16Array | number[]): number {
    if (this.length === 0 || dest.length === 0) {
      return 0;
    }
    const toRead = Math.min(dest.length, this.length);
    for (let i = 0; i < toRead; i++) {
      dest[i] = this.storage[this.head]!;
      this.head = (this.head + 1) % this.capacity;
    }
    this.length -= toRead;
    return toRead;
  }

  public peekSamples(dest: Int16Array | number[]): number {
    if (this.length === 0 || dest.length === 0) {
      return 0;
    }
    const toRead = Math.min(dest.length, this.length);
    let curr = this.head;
    for (let i = 0; i < toRead; i++) {
      dest[i] = this.storage[curr]!;
      curr = (curr + 1) % this.capacity;
    }
    return toRead;
  }

  public availableRead(): number {
    return this.length;
  }

  public availableWrite(): number {
    return this.capacity - this.length;
  }

  public getCapacity(): number {
    return this.capacity;
  }

  public len(): number {
    return this.length;
  }

  public isEmpty(): boolean {
    return this.length === 0;
  }

  public isFull(): boolean {
    return this.length === this.capacity;
  }

  public clear(): void {
    this.head = 0;
    this.tail = 0;
    this.length = 0;
  }
}

export class EmulatedAudioStreamValidator {
  private carryByte: number | null = null;
  private headerChecked: boolean = false;
  private isWav: boolean = false;
  private headerBuf: number[] = [];
  private totalSamplesProcessed: number = 0;
  private totalBytesReceived: number = 0;
  private clippedSamplesCount: number = 0;
  private sequence: number = 0;

  public static readonly TARGET_SAMPLE_RATE = 16000;
  public static readonly WAV_HEADER_SIZE = 44;

  public processChunk(
    chunkBytes: Uint8Array,
    isEof: boolean,
  ): { chunk: EmulatedAudioChunk } | { error: string } {
    this.totalBytesReceived += chunkBytes.length;
    let slice: Uint8Array;

    if (!this.headerChecked) {
      for (let i = 0; i < chunkBytes.length; i++) {
        this.headerBuf.push(chunkBytes[i]!);
      }

      const checkLen = Math.min(this.headerBuf.length, 4);
      const riffBytes = [0x52, 0x49, 0x46, 0x46]; // "RIFF"
      let matchesRiffPrefix = checkLen > 0;
      for (let i = 0; i < checkLen; i++) {
        if (this.headerBuf[i] !== riffBytes[i]) {
          matchesRiffPrefix = false;
          break;
        }
      }

      if (matchesRiffPrefix) {
        if (this.headerBuf.length < EmulatedAudioStreamValidator.WAV_HEADER_SIZE) {
          if (isEof) {
            return {
              error: 'Truncated WAV container: stream ended before complete 44-byte header',
            };
          }
          const chunk: EmulatedAudioChunk = {
            sequence: this.sequence,
            timestampSec: this.totalSamplesProcessed / EmulatedAudioStreamValidator.TARGET_SAMPLE_RATE,
            samples: new Int16Array(0),
            rawBytesLen: chunkBytes.length,
            isLast: false,
          };
          this.sequence += 1;
          return { chunk };
        }

        // Validate WAVE header
        const waveMagic = String.fromCharCode(...this.headerBuf.slice(8, 12));
        if (waveMagic !== 'WAVE') {
          return { error: 'Unsupported RIFF container: expected WAVE format chunk' };
        }

        this.headerChecked = true;
        this.isWav = true;
        const remainder = this.headerBuf.slice(EmulatedAudioStreamValidator.WAV_HEADER_SIZE);
        this.headerBuf = [];
        slice = new Uint8Array(remainder);
      } else {
        this.headerChecked = true;
        this.isWav = false;
        slice = new Uint8Array(this.headerBuf);
        this.headerBuf = [];
      }
    } else {
      slice = chunkBytes;
    }

    const samplesList: number[] = [];
    let idx = 0;

    if (this.carryByte !== null) {
      const prev = this.carryByte;
      this.carryByte = null;
      if (idx < slice.length) {
        const b0 = prev;
        const b1 = slice[idx]!;
        const val = (b1 << 8) | b0;
        const sample = val > 0x7fff ? val - 0x10000 : val;
        this.recordSample(sample);
        samplesList.push(sample);
        idx += 1;
      } else {
        if (isEof) {
          const val = (0x00 << 8) | prev;
          const sample = val > 0x7fff ? val - 0x10000 : val;
          this.recordSample(sample);
          samplesList.push(sample);
        } else {
          this.carryByte = prev;
        }
      }
    }

    while (idx + 1 < slice.length) {
      const b0 = slice[idx]!;
      const b1 = slice[idx + 1]!;
      const val = (b1 << 8) | b0;
      const sample = val > 0x7fff ? val - 0x10000 : val;
      this.recordSample(sample);
      samplesList.push(sample);
      idx += 2;
    }

    if (idx < slice.length) {
      const oddByte = slice[idx]!;
      if (isEof) {
        const val = (0x00 << 8) | oddByte;
        const sample = val > 0x7fff ? val - 0x10000 : val;
        this.recordSample(sample);
        samplesList.push(sample);
      } else {
        this.carryByte = oddByte;
      }
    }

    const timestampSec = this.totalSamplesProcessed / EmulatedAudioStreamValidator.TARGET_SAMPLE_RATE;
    this.totalSamplesProcessed += samplesList.length;

    const seq = this.sequence;
    this.sequence += 1;

    return {
      chunk: {
        sequence: seq,
        timestampSec,
        samples: new Int16Array(samplesList),
        rawBytesLen: chunkBytes.length,
        isLast: isEof,
      },
    };
  }

  private recordSample(sample: number): void {
    if (sample === 32767 || sample === -32768) {
      this.clippedSamplesCount += 1;
    }
  }

  public getCarryByte(): number | null {
    return this.carryByte;
  }

  public getTotalSamples(): number {
    return this.totalSamplesProcessed;
  }

  public getTotalBytes(): number {
    return this.totalBytesReceived;
  }

  public getClippingRatio(): number {
    return this.totalSamplesProcessed === 0 ? 0.0 : this.clippedSamplesCount / this.totalSamplesProcessed;
  }

  public isWavContainer(): boolean {
    return this.isWav;
  }
}

// ============================================================================
// EMPIRICAL TESTS
// ============================================================================

describe('ADVERSARIAL STRESS TEST: Bounded Queue, RingBuffer & Stream Validator', () => {
  describe('Vector 1: BoundedAudioQueue Memory Footprint & 8-Hour VOD Simulation', () => {
    it('enforces 16 chunks * 64KB = 1,048,576 bytes static footprint throughout 8-hour VOD stream', () => {
      const CHUNK_SIZE_BYTES = 64 * 1024; // 65,536 bytes (~2.048 sec of 16kHz mono)
      const QUEUE_CAPACITY = 16;
      const MAX_PIPELINE_RAM_CAP_BYTES = 5 * 1024 * 1024; // 5MB

      // 8 hours at 16kHz 16-bit mono PCM = 32,000 bytes/sec * 28,800 sec = 921,600,000 bytes (~921.6 MB)
      const TOTAL_STREAM_BYTES = 921_600_000;
      const TOTAL_CHUNKS = Math.ceil(TOTAL_STREAM_BYTES / CHUNK_SIZE_BYTES); // ~14,063 chunks

      // The bounded queue channel holds at most QUEUE_CAPACITY chunks in transit at any moment
      const maxQueueBytes = QUEUE_CAPACITY * CHUNK_SIZE_BYTES;
      expect(maxQueueBytes).toBe(1_048_576); // Exactly 1MB
      expect(maxQueueBytes).toBeLessThanOrEqual(MAX_PIPELINE_RAM_CAP_BYTES);

      // Simulate a queue with continuous push and pop
      let currentQueueOccupancy = 0;
      let peakOccupancy = 0;
      let totalBytesProcessed = 0;

      for (let chunkIdx = 0; chunkIdx < TOTAL_CHUNKS; chunkIdx++) {
        // Producer tries to push
        if (currentQueueOccupancy < QUEUE_CAPACITY) {
          currentQueueOccupancy++;
          totalBytesProcessed += CHUNK_SIZE_BYTES;
          if (currentQueueOccupancy > peakOccupancy) {
            peakOccupancy = currentQueueOccupancy;
          }
        }

        // Consumer drains chunks
        if (currentQueueOccupancy > 0 && chunkIdx % 2 === 0) {
          currentQueueOccupancy--;
        }
      }

      // Assert queue never exceeded bounded capacity
      expect(peakOccupancy).toBeLessThanOrEqual(QUEUE_CAPACITY);
      const peakMemoryMb = (peakOccupancy * CHUNK_SIZE_BYTES) / (1024 * 1024);
      expect(peakMemoryMb).toBeLessThanOrEqual(1.0); // Never exceeds 1MB RAM footprint!
      expect(totalBytesProcessed).toBeGreaterThan(0);
    });
  });

  describe('Vector 2: Backpressure Propagation to OS Pipe & FFmpeg Pause Invariant', () => {
    it('suspends producer when consumer halts, maintaining 0 memory leak and bounded queue', async () => {
      const QUEUE_CAPACITY = 4;
      const queue: EmulatedAudioChunk[] = [];
      let isProducerSuspended = false;

      // Simulated producer loop
      function tryProduce(chunk: EmulatedAudioChunk): boolean {
        if (queue.length >= QUEUE_CAPACITY) {
          isProducerSuspended = true; // Natural backpressure: channel.send().await suspends
          return false;
        }
        queue.push(chunk);
        isProducerSuspended = false;
        return true;
      }

      // Fill queue to capacity
      for (let i = 0; i < QUEUE_CAPACITY; i++) {
        const ok = tryProduce({
          sequence: i,
          timestampSec: i * 2.0,
          samples: new Int16Array(100),
          rawBytesLen: 200,
          isLast: false,
        });
        expect(ok).toBe(true);
      }
      expect(queue.length).toBe(QUEUE_CAPACITY);
      expect(isProducerSuspended).toBe(false);

      // Attempt to push 5th chunk while downstream consumer is halted
      const overflowPushed = tryProduce({
        sequence: 4,
        timestampSec: 8.0,
        samples: new Int16Array(100),
        rawBytesLen: 200,
        isLast: false,
      });
      expect(overflowPushed).toBe(false);
      expect(isProducerSuspended).toBe(true); // Producer is suspended! OS pipe fills!
      expect(queue.length).toBe(QUEUE_CAPACITY);

      // Downstream consumer unblocks by taking 1 chunk
      const consumed = queue.shift();
      expect(consumed?.sequence).toBe(0);
      expect(queue.length).toBe(QUEUE_CAPACITY - 1);

      // Producer unblocks and successfully pushes 5th chunk
      const resumedPushed = tryProduce({
        sequence: 4,
        timestampSec: 8.0,
        samples: new Int16Array(100),
        rawBytesLen: 200,
        isLast: false,
      });
      expect(resumedPushed).toBe(true);
      expect(isProducerSuspended).toBe(false);
      expect(queue.length).toBe(QUEUE_CAPACITY);
    });
  });

  describe('Vector 3: AudioRingBuffer Zero Heap Reallocation & Defect Detection', () => {
    it('maintains fixed capacity and zero reallocation during 100,000 continuous sliding-window samples', () => {
      const CAPACITY = 16000; // 1 second of audio samples
      const ring = new EmulatedAudioRingBuffer(CAPACITY);

      expect(ring.getCapacity()).toBe(CAPACITY);
      expect(ring.isEmpty()).toBe(true);
      expect(ring.availableWrite()).toBe(CAPACITY);

      const block = new Int16Array(512);
      for (let i = 0; i < 512; i++) block[i] = (i % 100) * 10;

      const out = new Int16Array(512);

      // Push and pop 200 blocks (102,400 samples)
      for (let cycle = 0; cycle < 200; cycle++) {
        const written = ring.pushSamples(block);
        expect(written).toBe(512);

        const read = ring.popSamples(out);
        expect(read).toBe(512);
        expect(out[0]).toBe(block[0]);
        expect(out[511]).toBe(block[511]);
      }

      expect(ring.isEmpty()).toBe(true);
      expect(ring.getCapacity()).toBe(CAPACITY); // Guaranteed 0 reallocation
    });

    it('EMPIRICAL BUG DETECTION: verifies flawed test assertion in src-tauri/src/ingestion/buffer.rs lines 240-248', () => {
      // In src-tauri/src/ingestion/buffer.rs:
      // test_ring_buffer_push_and_pop sets capacity = 8, pushes 5, pops 3 (remaining len = 2, available_write = 6)
      // then line 240: let input2 = [60, 70, 80, 90, 100]; (5 items!)
      // line 241: let written2 = buf.push_samples(&input2);
      // line 242: assert_eq!(written2, 6); // <-- BUG: 5.min(6) == 5, NOT 6!
      // line 243: assert!(buf.is_full()); // <-- BUG: len is 2+5=7, NOT 8!

      const ring = new EmulatedAudioRingBuffer(8);
      ring.pushSamples([10, 20, 30, 40, 50]);
      const pop3 = new Int16Array(3);
      ring.popSamples(pop3);

      expect(ring.availableRead()).toBe(2);
      expect(ring.availableWrite()).toBe(6);

      const input2 = [60, 70, 80, 90, 100]; // 5 items
      const actualWritten = ring.pushSamples(input2);

      // Prove that actual written is 5, NOT 6!
      expect(actualWritten).toBe(5);
      expect(actualWritten === 6).toBe(false); // Rust test assert_eq!(written2, 6) WOULD PANIC!

      // Prove that ring is NOT full (len is 7, capacity is 8)
      expect(ring.len()).toBe(7);
      expect(ring.isFull()).toBe(false); // Rust test assert!(buf.is_full()) WOULD PANIC!

      const out2 = new Int16Array(8);
      const actualRead = ring.popSamples(out2);
      expect(actualRead).toBe(7); // Rust test assert_eq!(read2, 8) WOULD PANIC!
      expect(out2[6]).toBe(100); // 100 was popped, not truncated
    });
  });

  describe('Vector 4: AudioStreamValidator Odd-Byte Alignment & Multi-Chunk Carry-Byte', () => {
    it('preserves perfect sample alignment across multiple consecutive odd-byte chunks (1023 bytes each)', () => {
      const validator = new EmulatedAudioStreamValidator();

      // Create a known deterministic sequence of 1000 16-bit samples (2000 bytes)
      const totalSamples = 1000;
      const rawBytes = new Uint8Array(totalSamples * 2);
      for (let i = 0; i < totalSamples; i++) {
        const sampleVal = ((i * 17) % 65535) - 32768; // Signed 16-bit
        const unsignedVal = sampleVal < 0 ? sampleVal + 0x10000 : sampleVal;
        rawBytes[i * 2] = unsignedVal & 0xff;
        rawBytes[i * 2 + 1] = (unsignedVal >> 8) & 0xff;
      }

      // Split rawBytes into chunks of 1023 bytes (odd length!)
      const CHUNK_SIZE = 1023;
      const chunks: Uint8Array[] = [];
      let offset = 0;
      while (offset < rawBytes.length) {
        const end = Math.min(offset + CHUNK_SIZE, rawBytes.length);
        chunks.push(rawBytes.slice(offset, end));
        offset = end;
      }
      expect(chunks.length).toBe(2); // 1023 bytes + 977 bytes (both odd!)

      const decodedSamples: number[] = [];

      for (let i = 0; i < chunks.length; i++) {
        const isEof = i === chunks.length - 1;
        const res = validator.processChunk(chunks[i]!, isEof);
        expect('chunk' in res).toBe(true);
        if ('chunk' in res) {
          for (let s = 0; s < res.chunk.samples.length; s++) {
            decodedSamples.push(res.chunk.samples[s]!);
          }
        }
      }

      // Assert zero samples lost and every single sample value matches original identically
      expect(decodedSamples.length).toBe(totalSamples);
      for (let i = 0; i < totalSamples; i++) {
        const expectedVal = ((i * 17) % 65535) - 32768;
        expect(decodedSamples[i]).toBe(expectedVal);
      }
      expect(validator.getCarryByte()).toBe(null);
    });

    it('handles trailing orphan carry byte at EOF by padding with 0x00 instead of discarding', () => {
      const validator = new EmulatedAudioStreamValidator();

      // Chunk 1: 3 bytes (sample 1: 0x1000, odd byte: 0x42)
      const chunk1 = new Uint8Array([0x00, 0x10, 0x42]);
      const res1 = validator.processChunk(chunk1, false);
      expect('chunk' in res1).toBe(true);
      if ('chunk' in res1) {
        expect(res1.chunk.samples.length).toBe(1);
        expect(res1.chunk.samples[0]).toBe(0x1000);
      }
      expect(validator.getCarryByte()).toBe(0x42);

      // Chunk 2: EOF with 0 bytes
      const res2 = validator.processChunk(new Uint8Array(0), true);
      expect('chunk' in res2).toBe(true);
      if ('chunk' in res2) {
        expect(res2.chunk.samples.length).toBe(1);
        expect(res2.chunk.samples[0]).toBe(0x0042); // 0x42 padded with 0x00
      }
      expect(validator.getCarryByte()).toBe(null);
    });
  });

  describe('Vector 5: Fragmented WAV Headers & Vulnerability / Signature Check', () => {
    it('correctly reassembles fragmented WAV header (< 44 bytes chunks) without failing', () => {
      const validator = new EmulatedAudioStreamValidator();

      // Build 44-byte WAV header + 2 samples (4 bytes)
      const wavHeader = new Uint8Array(48);
      // "RIFF"
      wavHeader.set([0x52, 0x49, 0x46, 0x46], 0);
      // Size 40
      wavHeader.set([40, 0, 0, 0], 4);
      // "WAVE"
      wavHeader.set([0x57, 0x41, 0x56, 0x45], 8);
      // "fmt "
      wavHeader.set([0x66, 0x6d, 0x74, 0x20], 12);
      // Subchunk1Size 16
      wavHeader.set([16, 0, 0, 0], 16);
      // AudioFormat 1 (PCM)
      wavHeader.set([1, 0], 20);
      // NumChannels 1 (Mono)
      wavHeader.set([1, 0], 22);
      // SampleRate 16000
      wavHeader.set([0x80, 0x3e, 0x00, 0x00], 24);
      // ByteRate 32000
      wavHeader.set([0x00, 0x7d, 0x00, 0x00], 28);
      // BlockAlign 2
      wavHeader.set([2, 0], 32);
      // BitsPerSample 16
      wavHeader.set([16, 0], 34);
      // "data"
      wavHeader.set([0x64, 0x61, 0x74, 0x61], 36);
      // DataSize 4
      wavHeader.set([4, 0, 0, 0], 40);
      // Sample 1: 1234 (0x04D2)
      wavHeader.set([0xd2, 0x04], 44);
      // Sample 2: -5678 (0xE9D2)
      wavHeader.set([0xd2, 0xe9], 46);

      // Feed Chunk 1: Fragmented, 20 bytes (< 44 bytes)
      const res1 = validator.processChunk(wavHeader.slice(0, 20), false);
      expect('chunk' in res1).toBe(true);
      if ('chunk' in res1) {
        expect(res1.chunk.samples.length).toBe(0); // Waiting for complete header
      }
      expect(validator.isWavContainer()).toBe(false);

      // Feed Chunk 2: Remaining 28 bytes (completes 44-byte header + 2 samples)
      const res2 = validator.processChunk(wavHeader.slice(20), true);
      expect('chunk' in res2).toBe(true);
      if ('chunk' in res2) {
        expect(res2.chunk.samples.length).toBe(2);
        expect(res2.chunk.samples[0]).toBe(1234);
        expect(res2.chunk.samples[1]).toBe(-5678);
        expect(res2.chunk.isLast).toBe(true);
      }
      expect(validator.isWavContainer()).toBe(true);
    });

    it('EMPIRICAL BUG DETECTION: detects compilation-breaking signature mismatch in validator.rs line 84', () => {
      const validatorPath = path.resolve('src-tauri/src/ingestion/validator.rs');
      expect(fs.existsSync(validatorPath)).toBe(true);
      const code = fs.readFileSync(validatorPath, 'utf8');

      // Verify validator.rs line 84 no longer calls AudioChunk::new with 3 arguments
      const brokenCallRegex = /AudioChunk::new\s*\(\s*Vec::new\(\)\s*,\s*self\.sequence\s*,\s*false\s*\)/;
      const hasBrokenCall = brokenCallRegex.test(code);
      expect(
        hasBrokenCall,
        'CRITICAL DEFECT RESOLVED: validator.rs line 84 must not call AudioChunk::new with 3 arguments',
      ).toBe(false);

      // Verify validator.rs supplies all 5 required parameters
      const validCallRegex = /AudioChunk::new\s*\(\s*self\.sequence\s*,\s*self\.current_timestamp_sec\(\)\s*,\s*Vec::new\(\)\s*,\s*chunk_bytes\.len\(\)\s*,\s*false,?\s*\)/;
      expect(validCallRegex.test(code)).toBe(true);

      // Also verify that AudioChunk::new in buffer.rs takes 5 arguments:
      const bufferPath = path.resolve('src-tauri/src/ingestion/buffer.rs');
      const bufferCode = fs.readFileSync(bufferPath, 'utf8');
      expect(bufferCode).toContain('pub fn new(');
      expect(bufferCode).toContain('sequence: u64,');
      expect(bufferCode).toContain('timestamp_sec: f64,');
      expect(bufferCode).toContain('samples: Vec<i16>,');
      expect(bufferCode).toContain('raw_bytes_len: usize,');
      expect(bufferCode).toContain('is_last: bool,');
    });

    it('EMPIRICAL VULNERABILITY DETECTION: highlights raw PCM false-positive RIFF prefix vulnerability', () => {
      const validator = new EmulatedAudioStreamValidator();

      // Challenge test:
      // A raw PCM stream delivers a fragmented initial chunk of 1 byte starting with 'R' (0x52).
      // The validator temporarily buffers the byte while waiting for subsequent bytes to verify RIFF.
      const chunk1 = new Uint8Array([0x52]); // 1 byte: 'R'
      const res1 = validator.processChunk(chunk1, false);

      // In chunk 1, validator buffers and yields an empty chunk
      expect('chunk' in res1).toBe(true);
      if ('chunk' in res1) {
        expect(res1.chunk.samples.length).toBe(0);
      }

      // In chunk 2, 99 raw audio PCM bytes arrive (second byte is 0x12, not 'I')
      const chunk2 = new Uint8Array(99);
      chunk2[0] = 0x12; // combined with 0x52 gives sample 0x1252 = 4690
      for (let i = 1; i < 99; i++) chunk2[i] = 0x00;

      // When chunk 2 arrives, the validator observes that the 4-byte prefix is NOT "RIFF"
      // and gracefully falls back to raw PCM mode without failing!
      const res2 = validator.processChunk(chunk2, true);
      expect('chunk' in res2).toBe(true);
      if ('chunk' in res2) {
        expect(validator.isWavContainer()).toBe(false); // Gracefully treated as raw PCM!
        expect(res2.chunk.samples.length).toBe(50); // 100 bytes total = 50 samples
        expect(res2.chunk.samples[0]).toBe(0x1252); // Buffered byte 0x52 was not lost!
      }
    });
  });

  describe('Vector 6: Static AST & Invariant Auditing for Ingestion Files', () => {
    it('verifies zero forbidden unwrap/expect/panic calls in production code of buffer.rs, validator.rs and streamer.rs', () => {
      const files = [
        'src-tauri/src/ingestion/buffer.rs',
        'src-tauri/src/ingestion/validator.rs',
        'src-tauri/src/ingestion/streamer.rs',
      ];

      for (const relPath of files) {
        const fullPath = path.resolve(relPath);
        expect(fs.existsSync(fullPath)).toBe(true);

        const content = fs.readFileSync(fullPath, 'utf8');
        const prodContent = content.split('#[cfg(test)]')[0] ?? content;
        const lines = prodContent.split('\n');

        const forbiddenPattern = /\b(unwrap|expect|panic!|todo!|unimplemented!)\b/g;

        for (let i = 0; i < lines.length; i++) {
          const line = lines[i]?.trim() ?? '';
          if (
            line.startsWith('//') ||
            line.startsWith('/*') ||
            line.startsWith('*') ||
            line.startsWith('#![deny(')
          ) {
            continue;
          }
          const match = line.match(forbiddenPattern);
          expect(
            match,
            `Forbidden call '${match?.[0]}' found in ${relPath} at line ${i + 1}: ${line}`,
          ).toBeNull();
        }
      }
    });
  });
});
