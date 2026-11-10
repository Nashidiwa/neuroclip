//! Bounded Queue and Circular RingBuffer primitives for streaming audio processing.
//!
//! Complies with ADR-002 (low RAM footprint <= 5MB), CONVENTIONS.md §2.3, and
//! zero-panic invariant.

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// Default raw chunk size in bytes (64KB).
///
/// At 16kHz 16-bit mono PCM (32,000 bytes/sec), a 64KB chunk represents exactly ~2.048s of audio.
pub const DEFAULT_CHUNK_SIZE_BYTES: usize = 64 * 1024;

/// Default capacity for bounded mpsc channel (16 chunks).
///
/// 16 chunks * 64KB = 1,048,576 bytes (~1MB static RAM footprint).
pub const DEFAULT_QUEUE_CAPACITY: usize = 16;

/// Hard ceiling for pipeline queue memory allocation (5MB).
pub const MAX_PIPELINE_RAM_CAP_BYTES: usize = 5 * 1024 * 1024;

/// Audio chunk emitted through the bounded streaming pipeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioChunk {
    /// Monotonically increasing sequence number starting from 0.
    pub sequence: u64,
    /// Absolute timestamp of the start of this chunk in seconds.
    pub timestamp_sec: f64,
    /// Decoded 16-bit signed PCM samples.
    pub samples: Vec<i16>,
    /// Raw byte count representing this chunk in the stream.
    pub raw_bytes_len: usize,
    /// Indicates whether this chunk is the final chunk in the stream (EOF).
    pub is_last: bool,
}

impl AudioChunk {
    /// Construct a new AudioChunk with calculated metrics.
    #[must_use]
    pub fn new(
        sequence: u64,
        timestamp_sec: f64,
        samples: Vec<i16>,
        raw_bytes_len: usize,
        is_last: bool,
    ) -> Self {
        Self {
            sequence,
            timestamp_sec,
            samples,
            raw_bytes_len,
            is_last,
        }
    }

    /// Calculate RMS (Root Mean Square) energy of the samples in this chunk.
    #[must_use]
    pub fn calculate_rms(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let sum_sq: f64 = self
            .samples
            .iter()
            .map(|&s| {
                let v = f64::from(s);
                v * v
            })
            .sum();
        (sum_sq / self.samples.len() as f64).sqrt()
    }
}

/// Fixed-capacity circular ring buffer specialized for `i16` PCM samples.
///
/// Guarantees zero dynamic heap reallocation after initialization, perfectly suited
/// for sliding-window algorithms such as RMS energy scoring and VAD.
#[derive(Debug)]
pub struct AudioRingBuffer {
    storage: Vec<i16>,
    capacity: usize,
    head: usize,
    tail: usize,
    len: usize,
}

impl AudioRingBuffer {
    /// Allocate an empty circular ring buffer with a fixed maximum sample capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            storage: vec![0i16; capacity],
            capacity,
            head: 0,
            tail: 0,
            len: 0,
        }
    }

    /// Push new samples into the ring buffer up to the remaining capacity.
    ///
    /// Returns the count of samples actually written.
    pub fn push_samples(&mut self, samples: &[i16]) -> usize {
        if self.capacity == 0 || self.len == self.capacity {
            return 0;
        }

        let to_write = samples.len().min(self.capacity - self.len);
        for &sample in &samples[..to_write] {
            self.storage[self.tail] = sample;
            self.tail = (self.tail + 1) % self.capacity;
        }
        self.len += to_write;
        to_write
    }

    /// Pop samples from the buffer into the destination slice and advance the read pointer.
    ///
    /// Returns the count of samples copied.
    pub fn pop_samples(&mut self, dest: &mut [i16]) -> usize {
        if self.len == 0 || dest.is_empty() {
            return 0;
        }

        let to_read = dest.len().min(self.len);
        for item in dest.iter_mut().take(to_read) {
            *item = self.storage[self.head];
            self.head = (self.head + 1) % self.capacity;
        }
        self.len -= to_read;
        to_read
    }

    /// Read samples without advancing the read pointer (sliding window inspection).
    ///
    /// Returns the count of samples previewed.
    pub fn peek_samples(&self, dest: &mut [i16]) -> usize {
        if self.len == 0 || dest.is_empty() {
            return 0;
        }

        let to_read = dest.len().min(self.len);
        let mut curr = self.head;
        for item in dest.iter_mut().take(to_read) {
            *item = self.storage[curr];
            curr = (curr + 1) % self.capacity;
        }
        to_read
    }

    /// Number of available samples ready to be read.
    #[inline]
    #[must_use]
    pub const fn available_read(&self) -> usize {
        self.len
    }

    /// Remaining write capacity before the buffer is full.
    #[inline]
    #[must_use]
    pub const fn available_write(&self) -> usize {
        self.capacity - self.len
    }

    /// Total allocated capacity of this ring buffer.
    #[inline]
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Current number of stored samples.
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether the ring buffer contains 0 samples.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Whether the ring buffer is at full capacity.
    #[inline]
    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.len == self.capacity
    }

    /// Reset read and write pointers, clearing all stored samples.
    pub fn clear(&mut self) {
        self.head = 0;
        self.tail = 0;
        self.len = 0;
    }
}

/// Create a bounded Tokio channel for streaming AudioChunks.
///
/// Capacity is bounded to enforce OS-level backpressure when downstream consumers
/// lag behind the audio decoder.
#[must_use]
pub fn create_bounded_audio_channel(
    capacity: usize,
) -> (mpsc::Sender<AudioChunk>, mpsc::Receiver<AudioChunk>) {
    let cap = if capacity == 0 {
        DEFAULT_QUEUE_CAPACITY
    } else {
        capacity
    };
    mpsc::channel::<AudioChunk>(cap)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_push_and_pop() {
        let mut buf = AudioRingBuffer::with_capacity(8);
        assert!(buf.is_empty());
        assert_eq!(buf.capacity(), 8);

        let input = [10, 20, 30, 40, 50];
        let written = buf.push_samples(&input);
        assert_eq!(written, 5);
        assert_eq!(buf.available_read(), 5);
        assert_eq!(buf.available_write(), 3);

        let mut out = [0i16; 3];
        let read = buf.pop_samples(&mut out);
        assert_eq!(read, 3);
        assert_eq!(out, [10, 20, 30]);
        assert_eq!(buf.available_read(), 2);

        // Wrap around test
        let input2 = [60, 70, 80, 90, 100, 110];
        let written2 = buf.push_samples(&input2);
        assert_eq!(written2, 6); // 8 - 2 = 6 available
        assert!(buf.is_full());

        let mut out2 = [0i16; 8];
        let read2 = buf.pop_samples(&mut out2);
        assert_eq!(read2, 8);
        assert_eq!(out2, [40, 50, 60, 70, 80, 90, 100, 110]);
        assert!(buf.is_empty());
    }

    #[test]
    fn test_ring_buffer_peek() {
        let mut buf = AudioRingBuffer::with_capacity(4);
        buf.push_samples(&[1, 2, 3]);

        let mut peek_dest = [0i16; 3];
        let peeked = buf.peek_samples(&mut peek_dest);
        assert_eq!(peeked, 3);
        assert_eq!(peek_dest, [1, 2, 3]);
        assert_eq!(buf.available_read(), 3); // Read pointer not moved
    }

    #[test]
    fn test_audio_chunk_rms() {
        let chunk = AudioChunk::new(0, 0.0, vec![1000, -1000, 1000, -1000], 8, false);
        let rms = chunk.calculate_rms();
        assert!((rms - 1000.0).abs() < 1e-4);
    }

    #[tokio::test]
    async fn test_bounded_channel_backpressure() {
        let (tx, mut rx) = create_bounded_audio_channel(2);

        let chunk1 = AudioChunk::new(1, 0.0, vec![1, 2], 4, false);
        let chunk2 = AudioChunk::new(2, 0.1, vec![3, 4], 4, false);

        assert!(tx.send(chunk1.clone()).await.is_ok());
        assert!(tx.send(chunk2.clone()).await.is_ok());

        let recv1 = rx.recv().await;
        assert_eq!(recv1, Some(chunk1));
        let recv2 = rx.recv().await;
        assert_eq!(recv2, Some(chunk2));
    }
}
