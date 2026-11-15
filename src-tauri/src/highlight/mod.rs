//! NeuroClip Multimodal Highlight Detection Subsystem
//!
//! Complies with `docs/CONVENTIONS.md` (zero-panic invariant, Result<T, AppError>),
//! `docs/ARCHITECTURE.md` §4.3 & §5.3, and `docs/TASK_BOARD.md` TASK-M4-09, TASK-M4-10 & TASK-M4-11.

pub mod audio_rms;
pub mod bucket;
pub mod emote_filter;
pub mod energy_analyzer;
pub mod swarm_dict;
pub mod zscore;

pub use audio_rms::{
    apply_silence_clamp, calculate_normalized_pcm_rms, calculate_pcm_rms, calculate_raw_rms,
    calculate_sine_normalized_rms, chunk_samples_to_blocks, chunk_samples_to_blocks_default,
    clamp_silence_floor, dbfs_to_normalized_scale, dbfs_to_rms, rms_to_dbfs, AudioBlockMetrics,
    DEFAULT_BLOCK_DURATION_SEC, DEFAULT_BLOCK_SAMPLES, DEFAULT_SAMPLE_RATE, I16_MAX_F64,
    MIN_RMS_THRESHOLD, SILENCE_FLOOR_AMPLITUDE, SILENCE_FLOOR_DBFS, SINE_1KHZ_EXPECTED_RMS,
    SINE_1KHZ_RMS_TOLERANCE, SINE_1KHZ_THEORETICAL_RMS,
};

pub use bucket::{
    bin_chat_events, bin_chat_events_default, bin_chat_events_from_file, bucket_chat_events,
    ChatBucket, DEFAULT_BUCKET_WIDTH_SEC, MAX_ALLOWED_BUCKETS, MAX_CHAT_TIMESTAMP_SEC,
    SPAM_BOT_MAX_MESSAGES_PER_AUTHOR,
};

pub use emote_filter::{
    calculate_emote_ratio, clean_word_token, compute_emote_density_series,
    compute_emote_density_series_default, compute_emote_density_series_from_file,
    extract_message_tokens, extract_swarm_emotes, is_swarm_emote, EmoteDensityPoint,
    MAX_TOP_EMOTES_PER_BUCKET,
};

pub use energy_analyzer::{
    analyze_audio_samples, analyze_audio_samples_default, analyze_wav_file, detect_energy_peaks,
    detect_energy_peaks_default, process_pcm_reader, AudioRmsPoint, AudioRmsSeriesResponse,
    EnergyAnalyzerConfig, EnergyBaselineStats, DEFAULT_BASELINE_WINDOW_SEC as DEFAULT_AUDIO_BASELINE_WINDOW_SEC,
    DEFAULT_PEAK_DB_SPIKE_THRESHOLD, DEFAULT_PEAK_SIGMA_THRESHOLD, EPSILON_RMS, PEAK_DB_THRESHOLD,
    PEAK_SIGMA_THRESHOLD,
};

pub use swarm_dict::{
    default_swarm_dictionary, SwarmDictionary, SwarmEmoteCategory, SwarmEmoteDefinition,
};

pub use zscore::{
    calculate_adjusted_zscore, compute_chat_velocity_series,
    compute_chat_velocity_series_default, compute_chat_velocity_series_from_events,
    compute_chat_velocity_series_from_file, compute_zscore, ChatVelocityPoint,
    DEFAULT_BASELINE_WINDOW_SEC, MESSAGE_FLOOR_THRESHOLD, SMOOTHING_EPSILON,
};
