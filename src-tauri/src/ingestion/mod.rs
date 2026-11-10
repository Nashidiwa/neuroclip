//! Ingestion subsystem: Audio-First Online VOD Download & FFmpeg PCM Extraction.
//!
//! Complies with docs/CONVENTIONS.md, docs/ARCHITECTURE.md §4.2, §5.2, and TASK-M4-06.

pub mod buffer;
pub mod cookies;
pub mod streamer;
pub mod validator;

pub use buffer::{
    create_bounded_audio_channel, AudioChunk, AudioRingBuffer, DEFAULT_CHUNK_SIZE_BYTES,
    DEFAULT_QUEUE_CAPACITY, MAX_PIPELINE_RAM_CAP_BYTES,
};
pub use cookies::{
    build_bot_defense_args, build_cookie_args, get_desktop_user_agents,
    get_rotated_desktop_user_agent, parse_browser_type, BrowserType, CookieSource,
};
pub use streamer::{
    build_ffmpeg_file_args, build_ffmpeg_pipe_args, compute_audio_energy_stream, detect_source_type,
    download_audio_stream, generate_vod_manifest, parse_ytdlp_progress_line, probe_vod_metadata,
    start_ffmpeg_pcm_stream, AudioEnergyWindow, ChildProcessGuard, IngestOptions,
    IngestProgressPayload, VodManifest, VodMetadata, WindowsJobObjectGuard,
};
pub use validator::{
    AudioStreamValidator, BYTES_PER_SAMPLE, TARGET_SAMPLE_RATE, WAV_HEADER_SIZE,
};
