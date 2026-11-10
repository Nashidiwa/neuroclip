//! Audio Stream Validator: Sample Alignment, Header Stripping & Sanity Metrics.
//!
//! Complies with docs/CONVENTIONS.md (zero-panic invariant) and TASK_BOARD.md (UT-06.1, UT-06.2).

use crate::error::IngestError;
use crate::ingestion::buffer::AudioChunk;

/// Target sampling rate for all downstream AI engines (Whisper STT & Silero VAD).
pub const TARGET_SAMPLE_RATE: u32 = 16_000;

/// Bytes per sample for 16-bit PCM (2 bytes).
pub const BYTES_PER_SAMPLE: usize = 2;

/// Standard PCM WAV container header size (44 bytes).
pub const WAV_HEADER_SIZE: usize = 44;

/// Stream validator ensuring 2-byte sample alignment and transparent container header stripping.
#[derive(Debug)]
pub struct AudioStreamValidator {
    carry_byte: Option<u8>,
    header_checked: bool,
    is_wav: bool,
    header_buf: Vec<u8>,
    total_samples_processed: u64,
    total_bytes_received: u64,
    clipped_samples_count: u64,
    sequence: u64,
}

impl Default for AudioStreamValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioStreamValidator {
    /// Construct a fresh validator ready to process an audio byte stream.
    #[must_use]
    pub fn new() -> Self {
        Self {
            carry_byte: None,
            header_checked: false,
            is_wav: false,
            header_buf: Vec::new(),
            total_samples_processed: 0,
            total_bytes_received: 0,
            clipped_samples_count: 0,
            sequence: 0,
        }
    }

    /// Process a raw byte slice from the audio decoder pipe.
    ///
    /// Corrects for odd-byte chunk boundaries, strips any container WAV header, and
    /// generates an aligned `AudioChunk`.
    ///
    /// # Errors
    /// Returns `IngestError::ExtractionFailed` if an invalid or corrupted WAV header is encountered.
    pub fn process_chunk(
        &mut self,
        chunk_bytes: &[u8],
        is_eof: bool,
    ) -> Result<AudioChunk, IngestError> {
        self.total_bytes_received += chunk_bytes.len() as u64;

        let working_bytes_owner: Vec<u8>;
        let slice: &[u8];

        // Check for 44-byte WAV header, buffering fragmented initial chunks if needed
        if !self.header_checked {
            self.header_buf.extend_from_slice(chunk_bytes);

            let check_len = self.header_buf.len().min(4);
            let matches_riff_prefix = check_len > 0 && self.header_buf[..check_len] == b"RIFF"[..check_len];

            if matches_riff_prefix && self.header_buf.len() < 4 && !is_eof {
                // Potential RIFF header fragmented under 4 bytes: wait for more bytes
                let chunk = AudioChunk::new(
                    self.sequence,
                    self.current_timestamp_sec(),
                    Vec::new(),
                    chunk_bytes.len(),
                    false,
                );
                self.sequence += 1;
                return Ok(chunk);
            }

            let is_riff = self.header_buf.len() >= 4 && &self.header_buf[..4] == b"RIFF";

            if is_riff {
                if self.header_buf.len() < WAV_HEADER_SIZE {
                    if is_eof {
                        return Err(IngestError::ExtractionFailed(
                            "Truncated WAV container: stream ended before complete 44-byte header".to_string(),
                        ));
                    }
                    // Wait for subsequent chunk(s) to reach full 44-byte header
                    let chunk = AudioChunk::new(
                        self.sequence,
                        self.current_timestamp_sec(),
                        Vec::new(),
                        chunk_bytes.len(),
                        false,
                    );
                    self.sequence += 1;
                    return Ok(chunk);
                }

                if &self.header_buf[8..12] != b"WAVE" {
                    return Err(IngestError::ExtractionFailed(
                        "Unsupported RIFF container: expected WAVE format chunk".to_string(),
                    ));
                }

                // Validate audio format: PCM = 1
                let audio_format = u16::from_le_bytes([self.header_buf[20], self.header_buf[21]]);
                let num_channels = u16::from_le_bytes([self.header_buf[22], self.header_buf[23]]);
                let sample_rate = u32::from_le_bytes([
                    self.header_buf[24],
                    self.header_buf[25],
                    self.header_buf[26],
                    self.header_buf[27],
                ]);
                let bits_per_sample = u16::from_le_bytes([self.header_buf[34], self.header_buf[35]]);

                if audio_format != 1 {
                    return Err(IngestError::ExtractionFailed(format!(
                        "Unsupported WAV compression format: {audio_format} (expected 1 for uncompressed PCM)"
                    )));
                }

                if num_channels != 1 {
                    return Err(IngestError::ExtractionFailed(format!(
                        "Unsupported WAV channel count: {num_channels} (expected 1 for mono)"
                    )));
                }

                if sample_rate != TARGET_SAMPLE_RATE {
                    return Err(IngestError::ExtractionFailed(format!(
                        "Unsupported WAV sample rate: {sample_rate}Hz (expected {TARGET_SAMPLE_RATE}Hz)"
                    )));
                }

                if bits_per_sample != 16 {
                    return Err(IngestError::ExtractionFailed(format!(
                        "Unsupported WAV bits per sample: {bits_per_sample} (expected 16-bit)"
                    )));
                }

                self.header_checked = true;
                self.is_wav = true;

                // Strip the 44-byte header and expose remaining bytes
                working_bytes_owner = self.header_buf[WAV_HEADER_SIZE..].to_vec();
                self.header_buf.clear();
                slice = &working_bytes_owner;
            } else {
                // Not a WAV container (does not start with RIFF); treat as raw PCM
                self.header_checked = true;
                self.is_wav = false;
                working_bytes_owner = std::mem::take(&mut self.header_buf);
                slice = &working_bytes_owner;
            }
        } else {
            slice = chunk_bytes;
        }

        // Assemble working bytes taking carry_byte into account
        let mut samples = Vec::with_capacity((slice.len() + usize::from(self.carry_byte.is_some())) / 2);
        let mut idx = 0;

        // If there was an orphan byte from the previous chunk, combine with first byte
        if let Some(prev) = self.carry_byte.take() {
            if idx < slice.len() {
                let sample = i16::from_le_bytes([prev, slice[idx]]);
                self.record_sample(sample);
                samples.push(sample);
                idx += 1;
            } else {
                // Slice is empty but we have carry_byte
                if is_eof {
                    let sample = i16::from_le_bytes([prev, 0x00]);
                    self.record_sample(sample);
                    samples.push(sample);
                } else {
                    // Still waiting for next data
                    self.carry_byte = Some(prev);
                }
            }
        }

        // Parse remaining complete 2-byte samples
        while idx + 1 < slice.len() {
            let b0 = slice[idx];
            let b1 = slice[idx + 1];
            let sample = i16::from_le_bytes([b0, b1]);
            self.record_sample(sample);
            samples.push(sample);
            idx += 2;
        }

        // If an odd byte remains at the end of slice
        if idx < slice.len() {
            let odd_byte = slice[idx];
            if is_eof {
                // EOF: pad with 0x00 so no byte is discarded
                let sample = i16::from_le_bytes([odd_byte, 0x00]);
                self.record_sample(sample);
                samples.push(sample);
            } else {
                // Save for next chunk
                self.carry_byte = Some(odd_byte);
            }
        }

        let chunk_timestamp = self.total_samples_processed as f64 / TARGET_SAMPLE_RATE as f64;
        self.total_samples_processed += samples.len() as u64;

        let seq = self.sequence;
        self.sequence += 1;

        Ok(AudioChunk::new(
            seq,
            chunk_timestamp,
            samples,
            chunk_bytes.len(),
            is_eof,
        ))
    }

    #[inline]
    fn record_sample(&mut self, sample: i16) {
        if sample == i16::MAX || sample == i16::MIN {
            self.clipped_samples_count += 1;
        }
    }

    /// Total samples decoded and emitted through this validator.
    #[inline]
    #[must_use]
    pub const fn total_samples(&self) -> u64 {
        self.total_samples_processed
    }

    /// Total bytes ingested.
    #[inline]
    #[must_use]
    pub const fn total_bytes(&self) -> u64 {
        self.total_bytes_received
    }

    /// Current timestamp in seconds based on decoded sample count.
    #[inline]
    #[must_use]
    pub fn current_timestamp_sec(&self) -> f64 {
        self.total_samples_processed as f64 / TARGET_SAMPLE_RATE as f64
    }

    /// Ratio of clipped samples over all processed samples (0.0 to 1.0).
    #[must_use]
    pub fn clipping_ratio(&self) -> f64 {
        if self.total_samples_processed == 0 {
            0.0
        } else {
            self.clipped_samples_count as f64 / self.total_samples_processed as f64
        }
    }

    /// Check whether a container WAV header was detected and stripped.
    #[inline]
    #[must_use]
    pub const fn is_wav_container(&self) -> bool {
        self.is_wav
    }

    /// Reset state for a new audio stream.
    pub fn reset(&mut self) {
        self.carry_byte = None;
        self.header_checked = false;
        self.is_wav = false;
        self.header_buf.clear();
        self.total_samples_processed = 0;
        self.total_bytes_received = 0;
        self.clipped_samples_count = 0;
        self.sequence = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perfect_alignment() {
        let mut validator = AudioStreamValidator::new();
        // 4 bytes = 2 samples: 1000 and -2000
        let s1 = 1000i16.to_le_bytes();
        let s2 = (-2000i16).to_le_bytes();
        let bytes = [s1[0], s1[1], s2[0], s2[1]];

        let chunk = validator.process_chunk(&bytes, false).unwrap();
        assert_eq!(chunk.samples, vec![1000, -2000]);
        assert_eq!(chunk.sequence, 0);
        assert!(!chunk.is_last);
    }

    #[test]
    fn test_odd_byte_carry_over() {
        let mut validator = AudioStreamValidator::new();

        // Chunk 1: 3 bytes (sample 1 complete, 1 carry byte)
        let s1 = 500i16.to_le_bytes();
        let s2 = 1200i16.to_le_bytes();
        let chunk1_bytes = [s1[0], s1[1], s2[0]]; // Odd byte s2[0]

        let chunk1 = validator.process_chunk(&chunk1_bytes, false).unwrap();
        assert_eq!(chunk1.samples, vec![500]);
        assert_eq!(validator.carry_byte, Some(s2[0]));

        // Chunk 2: 3 bytes (s2[1] arrives, plus sample 3 complete)
        let s3 = (-300i16).to_le_bytes();
        let chunk2_bytes = [s2[1], s3[0], s3[1]];

        let chunk2 = validator.process_chunk(&chunk2_bytes, true).unwrap();
        assert_eq!(chunk2.samples, vec![1200, -300]);
        assert_eq!(validator.carry_byte, None);
        assert!(chunk2.is_last);
    }

    #[test]
    fn test_wav_header_stripping() {
        let mut validator = AudioStreamValidator::new();

        // Construct valid 44-byte WAV header
        let mut wav = vec![0u8; 44];
        wav[0..4].copy_from_slice(b"RIFF");
        wav[8..12].copy_from_slice(b"WAVE");
        wav[12..16].copy_from_slice(b"fmt ");
        wav[16..20].copy_from_slice(&16u32.to_le_bytes()); // subchunk size
        wav[20..22].copy_from_slice(&1u16.to_le_bytes());  // PCM
        wav[22..24].copy_from_slice(&1u16.to_le_bytes());  // mono
        wav[24..28].copy_from_slice(&16000u32.to_le_bytes()); // 16kHz
        wav[28..32].copy_from_slice(&32000u32.to_le_bytes()); // byte rate
        wav[32..34].copy_from_slice(&2u16.to_le_bytes());  // block align
        wav[34..36].copy_from_slice(&16u16.to_le_bytes()); // bits per sample
        wav[36..40].copy_from_slice(b"data");

        // Add 2 audio samples (4 bytes)
        let s1 = 42i16.to_le_bytes();
        let s2 = 99i16.to_le_bytes();
        wav.extend_from_slice(&s1);
        wav.extend_from_slice(&s2);

        let chunk = validator.process_chunk(&wav, true).unwrap();
        assert!(validator.is_wav_container());
        assert_eq!(chunk.samples, vec![42, 99]);
    }

    #[test]
    fn test_fragmented_wav_header_under_44_bytes() {
        let mut validator = AudioStreamValidator::new();

        // Construct valid 44-byte WAV header + 2 samples (4 bytes)
        let mut wav = vec![0u8; 44];
        wav[0..4].copy_from_slice(b"RIFF");
        wav[8..12].copy_from_slice(b"WAVE");
        wav[12..16].copy_from_slice(b"fmt ");
        wav[16..20].copy_from_slice(&16u32.to_le_bytes());
        wav[20..22].copy_from_slice(&1u16.to_le_bytes());
        wav[22..24].copy_from_slice(&1u16.to_le_bytes());
        wav[24..28].copy_from_slice(&16000u32.to_le_bytes());
        wav[28..32].copy_from_slice(&32000u32.to_le_bytes());
        wav[32..34].copy_from_slice(&2u16.to_le_bytes());
        wav[34..36].copy_from_slice(&16u16.to_le_bytes());
        wav[36..40].copy_from_slice(b"data");

        let s1 = 1234i16.to_le_bytes();
        let s2 = -5678i16.to_le_bytes();
        wav.extend_from_slice(&s1);
        wav.extend_from_slice(&s2);

        // Chunk 1: Fragmented, only 20 bytes (less than 44 bytes!)
        let chunk1 = validator.process_chunk(&wav[..20], false).unwrap();
        assert_eq!(chunk1.samples.len(), 0);
        assert!(!validator.is_wav_container()); // Not yet finalized until header is complete

        // Chunk 2: Remaining 28 bytes (completes the 44-byte header and carries the 4 sample bytes)
        let chunk2 = validator.process_chunk(&wav[20..], true).unwrap();
        assert!(validator.is_wav_container());
        assert_eq!(chunk2.samples, vec![1234, -5678]);
        assert!(chunk2.is_last);
    }

    #[test]
    fn test_truncated_wav_header_returns_error_on_eof() {
        let mut validator = AudioStreamValidator::new();
        // RIFF header truncated at 16 bytes followed by EOF
        let truncated = b"RIFF\x24\x00\x00\x00WAVEfmt ";
        let res = validator.process_chunk(truncated, true);
        assert!(res.is_err());
    }

    #[test]
    fn test_fragmented_riff_under_4_bytes() {
        let mut validator = AudioStreamValidator::new();

        // 44-byte WAV header + 2 samples
        let mut wav = vec![0u8; 44];
        wav[0..4].copy_from_slice(b"RIFF");
        wav[8..12].copy_from_slice(b"WAVE");
        wav[12..16].copy_from_slice(b"fmt ");
        wav[16..20].copy_from_slice(&16u32.to_le_bytes());
        wav[20..22].copy_from_slice(&1u16.to_le_bytes());
        wav[22..24].copy_from_slice(&1u16.to_le_bytes());
        wav[24..28].copy_from_slice(&16000u32.to_le_bytes());
        wav[28..32].copy_from_slice(&32000u32.to_le_bytes());
        wav[32..34].copy_from_slice(&2u16.to_le_bytes());
        wav[34..36].copy_from_slice(&16u16.to_le_bytes());
        wav[36..40].copy_from_slice(b"data");

        let s1 = 111i16.to_le_bytes();
        let s2 = 222i16.to_le_bytes();
        wav.extend_from_slice(&s1);
        wav.extend_from_slice(&s2);

        // Chunk 1: only 2 bytes ("RI")
        let chunk1 = validator.process_chunk(&wav[..2], false).unwrap();
        assert_eq!(chunk1.samples.len(), 0);
        assert!(!validator.is_wav_container());

        // Chunk 2: remaining bytes
        let chunk2 = validator.process_chunk(&wav[2..], true).unwrap();
        assert!(validator.is_wav_container());
        assert_eq!(chunk2.samples, vec![111, 222]);
        assert!(chunk2.is_last);
    }

    #[test]
    fn test_false_positive_riff_prefix_eof() {
        let mut validator = AudioStreamValidator::new();

        // 1 byte: 0x52 ('R') followed by EOF
        // Must NOT fail as truncated WAV, but fall back to raw PCM gracefully
        let res = validator.process_chunk(&[0x52], true);
        assert!(res.is_ok());
        let chunk = res.unwrap();
        assert!(!validator.is_wav_container());
        assert_eq!(chunk.samples.len(), 1); // 0x52 padded with 0x00 -> sample 0x0052 = 82
        assert_eq!(chunk.samples[0], 0x0052);
    }
}

