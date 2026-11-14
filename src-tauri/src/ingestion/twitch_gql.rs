//! Twitch GraphQL API client for cursor-paginated chat replay extraction.
//!
//! Complies with docs/CONVENTIONS.md (zero-panic invariant, Result<T, AppError>)
//! and docs/ARCHITECTURE.md §4.2, §5.3, §6 (ERR-05).

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::process::Stdio;
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, IngestError, StorageError};
use crate::ingestion::streamer::assign_child_to_job_guard;

pub const TWITCH_GQL_ENDPOINT: &str = "https://gql.twitch.tv/gql";
pub const TWITCH_CLIENT_ID: &str = "kimne78kx3ncx6brgo4mv6wki5h1ko";
pub const TWITCH_COMMENTS_SHA256: &str =
    "b70a3591ff0f4e0313d126c6a1502d79a1c02baebb28822d56ad53c4451407af";
pub const TWITCH_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

/// Progress payload emitted during chat extraction stages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatProgressPayload {
    pub task_id: String,
    pub stage: String,
    pub percent: f64,
    pub messages_extracted: u64,
    pub current_offset_sec: f64,
}

#[derive(Serialize)]
struct GqlVariables<'a> {
    #[serde(rename = "videoID")]
    video_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none", rename = "contentOffsetSeconds")]
    content_offset_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<&'a str>,
}

#[derive(Serialize)]
struct GqlPersistedQuery<'a> {
    version: u32,
    #[serde(rename = "sha256Hash")]
    sha256_hash: &'a str,
}

#[derive(Serialize)]
struct GqlExtensions<'a> {
    #[serde(rename = "persistedQuery")]
    persisted_query: GqlPersistedQuery<'a>,
}

#[derive(Serialize)]
struct GqlRequestPayload<'a> {
    #[serde(rename = "operationName")]
    operation_name: &'a str,
    variables: GqlVariables<'a>,
    extensions: GqlExtensions<'a>,
}

/// Download Twitch VOD chat replay using Twitch GQL cursor pagination.
///
/// If chat is missing, deleted, or expired, returns `ERR_INGEST_CHAT_UNAVAILABLE`
/// within <= 200ms to allow soft fallback (ERR-05).
pub async fn fetch_twitch_vod_chat_gql<F>(
    vod_id: &str,
    output_ndjson_path: &Path,
    cancel_token: CancellationToken,
    progress_callback: F,
) -> Result<u64, AppError>
where
    F: Fn(ChatProgressPayload) + Send + Sync + 'static,
{
    // Fast mock check for unit / integration tests
    if std::env::var("NEUROCLIP_MOCK_CHAT_UNAVAILABLE").is_ok() {
        return Err(AppError::Ingestion(IngestError::ChatReplayUnavailable));
    }

    if let Ok(mock_file_path) = std::env::var("NEUROCLIP_MOCK_TWITCH_CHAT_FILE") {
        if let Ok(content) = std::fs::read_to_string(&mock_file_path) {
            std::fs::write(output_ndjson_path, content.as_bytes()).map_err(|e| {
                AppError::Storage(StorageError::DirectoryCreationFailed {
                    path: output_ndjson_path.display().to_string(),
                    details: e.to_string(),
                })
            })?;
            let line_count = content.lines().filter(|l| !l.trim().is_empty()).count() as u64;
            progress_callback(ChatProgressPayload {
                task_id: vod_id.to_string(),
                stage: "completed".to_string(),
                percent: 100.0,
                messages_extracted: line_count,
                current_offset_sec: 0.0,
            });
            return Ok(line_count);
        }
    }

    // Clean vod_id (strip prefix if user entered full URL or v12345)
    let clean_vod_id = vod_id
        .trim_start_matches("https://www.twitch.tv/videos/")
        .trim_start_matches("https://twitch.tv/videos/")
        .trim_start_matches('v')
        .split('?')
        .next()
        .unwrap_or(vod_id);

    let out_file = File::create(output_ndjson_path).map_err(|e| {
        AppError::Storage(StorageError::DirectoryCreationFailed {
            path: output_ndjson_path.display().to_string(),
            details: e.to_string(),
        })
    })?;
    let mut writer = BufWriter::with_capacity(64 * 1024, out_file);

    let mut cursor: Option<String> = None;
    let mut messages_count: u64 = 0;
    let mut current_offset: f64 = 0.0;
    let mut has_next_page = true;
    let mut page_index: u64 = 0;

    while has_next_page {
        if cancel_token.is_cancelled() {
            return Err(AppError::TaskCancelled);
        }

        let payload = vec![GqlRequestPayload {
            operation_name: "VideoCommentsByOffsetOrCursor",
            variables: GqlVariables {
                video_id: clean_vod_id,
                content_offset_seconds: if cursor.is_none() { Some(0) } else { None },
                cursor: cursor.as_deref(),
            },
            extensions: GqlExtensions {
                persisted_query: GqlPersistedQuery {
                    version: 1,
                    sha256_hash: TWITCH_COMMENTS_SHA256,
                },
            },
        }];

        let json_body = serde_json::to_string(&payload)
            .map_err(|e| AppError::Internal(format!("Failed to serialize GQL payload: {e}")))?;

        // Execute request via Windows-native curl.exe subprocess
        let mut cmd = Command::new("curl.exe");
        cmd.args([
            "-s",
            "--max-time",
            "15",
            "-X",
            "POST",
            "-H",
            &format!("Client-ID: {TWITCH_CLIENT_ID}"),
            "-H",
            "Content-Type: application/json",
            "-H",
            &format!("User-Agent: {TWITCH_USER_AGENT}"),
            "-d",
            &json_body,
            TWITCH_GQL_ENDPOINT,
        ]);
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let child = cmd.spawn().map_err(|e| {
            AppError::Internal(format!("Failed to spawn curl for Twitch GQL: {e}"))
        })?;
        assign_child_to_job_guard(&child);

        let output = child
            .wait_with_output()
            .await
            .map_err(|e| AppError::Internal(format!("Curl process execution failed: {e}")))?;

        if !output.status.success() {
            if page_index == 0 {
                return Err(AppError::Ingestion(IngestError::ChatReplayUnavailable));
            }
            break;
        }

        let response_str = String::from_utf8_lossy(&output.stdout);
        if response_str.trim().is_empty() {
            if page_index == 0 {
                return Err(AppError::Ingestion(IngestError::ChatReplayUnavailable));
            }
            break;
        }

        let resp_json: serde_json::Value = match serde_json::from_str(&response_str) {
            Ok(j) => j,
            Err(_) => {
                if page_index == 0 {
                    return Err(AppError::Ingestion(IngestError::ChatReplayUnavailable));
                }
                break;
            }
        };

        // Check if root array element has video comments
        let first_elem = resp_json
            .as_array()
            .and_then(|arr| arr.first())
            .unwrap_or(&resp_json);

        let video = first_elem.pointer("/data/video");
        if video.is_none() || video == Some(&serde_json::Value::Null) {
            return Err(AppError::Ingestion(IngestError::ChatReplayUnavailable));
        }

        let comments = first_elem.pointer("/data/video/comments");
        if comments.is_none() || comments == Some(&serde_json::Value::Null) {
            return Err(AppError::Ingestion(IngestError::ChatReplayUnavailable));
        }

        let edges = comments
            .and_then(|c| c.get("edges"))
            .and_then(|e| e.as_array());

        let Some(edge_array) = edges else {
            if page_index == 0 {
                return Err(AppError::Ingestion(IngestError::ChatReplayUnavailable));
            }
            break;
        };

        if edge_array.is_empty() {
            break;
        }

        for edge in edge_array {
            if let Some(node) = edge.get("node") {
                if let Ok(line) = serde_json::to_string(node) {
                    let _ = writer.write_all(line.as_bytes());
                    let _ = writer.write_all(b"\n");
                    messages_count = messages_count.saturating_add(1);

                    if let Some(offset) = node.get("contentOffsetSeconds").and_then(|o| o.as_f64()) {
                        current_offset = offset;
                    }
                }
            }
        }

        // Check pagination cursor
        let page_info = comments.and_then(|c| c.get("pageInfo"));
        has_next_page = page_info
            .and_then(|p| p.get("hasNextPage"))
            .and_then(|h| h.as_bool())
            .unwrap_or(false);

        cursor = edge_array
            .last()
            .and_then(|last_edge| last_edge.get("cursor"))
            .and_then(|c| c.as_str())
            .map(|s| s.to_string());

        page_index = page_index.saturating_add(1);

        progress_callback(ChatProgressPayload {
            task_id: clean_vod_id.to_string(),
            stage: "extracting_twitch_chat".to_string(),
            percent: 0.0, // unknown total length during stream
            messages_extracted: messages_count,
            current_offset_sec: current_offset,
        });

        if cursor.is_none() {
            break;
        }
    }

    writer
        .flush()
        .map_err(|e| AppError::Internal(format!("Failed to flush output NDJSON: {e}")))?;

    if messages_count == 0 {
        return Err(AppError::Ingestion(IngestError::ChatReplayUnavailable));
    }

    progress_callback(ChatProgressPayload {
        task_id: clean_vod_id.to_string(),
        stage: "completed".to_string(),
        percent: 100.0,
        messages_extracted: messages_count,
        current_offset_sec: current_offset,
    });

    Ok(messages_count)
}
