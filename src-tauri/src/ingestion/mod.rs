//! Ingestion subsystem: Audio-First Online VOD Download & FFmpeg PCM Extraction.
//!
//! Complies with docs/CONVENTIONS.md, docs/ARCHITECTURE.md §4.2, §5.2, and TASK-M4-06.

pub mod buffer;
pub mod chat_parser;
pub mod cookies;
pub mod streamer;
pub mod twitch_gql;
pub mod validator;
pub mod youtube_chat;

pub use buffer::{
    create_bounded_audio_channel, AudioChunk, AudioRingBuffer, DEFAULT_CHUNK_SIZE_BYTES,
    DEFAULT_QUEUE_CAPACITY, MAX_PIPELINE_RAM_CAP_BYTES,
};
pub use chat_parser::{
    extract_message_emotes, is_neuro_swarm_emote, parse_raw_chat_line, ChatDensityPoint,
    ChatEventsContainer, ChatEvent, ChatHeatmapContainer, ChatHeatmapResponse, HeatmapWindow,
    StreamingChatNormalizer, NEURO_SWARM_EMOTES,
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
pub use twitch_gql::{
    fetch_twitch_vod_chat_gql, ChatProgressPayload, TWITCH_CLIENT_ID, TWITCH_COMMENTS_SHA256,
    TWITCH_GQL_ENDPOINT, TWITCH_USER_AGENT,
};
pub use validator::{
    AudioStreamValidator, BYTES_PER_SAMPLE, TARGET_SAMPLE_RATE, WAV_HEADER_SIZE,
};
pub use youtube_chat::fetch_youtube_live_chat_replay;
