//! Atomic file persistence primitives for NeuroClip.
//!
//! Provides atomic, fail-safe file read and write operations preventing 0-byte
//! truncated files, torn writes, or data corruption in the event of unexpected
//! application termination, OS crash, or power loss.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use serde::de::DeserializeOwned;
use serde::Serialize;
use crate::error::ConfigError;

/// Monotonically increasing atomic sequence counter ensuring unique temporary
/// filenames across all threads in the current process.
static ATOMIC_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Atomically persist a serializable data structure to a JSON file.
///
/// Steps:
/// 1. Create parent directory if missing.
/// 2. Serialize payload to pretty-printed JSON bytes.
/// 3. Create a unique temporary file (`.<name>.tmp.<pid>.<timestamp_nanos>.<seq>`) in the same folder.
/// 4. Write data and call `file.sync_all()` to ensure physical persistence on disk.
/// 5. Atomically rename the temporary file to target destination (atomic swap).
/// 6. Automatically cleanup temporary file on any failure.
pub fn atomic_save_json<T: Serialize>(path: &Path, data: &T) -> Result<(), ConfigError> {
    let parent = path.parent().ok_or_else(|| ConfigError::SaveFailed {
        path: path.to_string_lossy().to_string(),
        details: "Đường dẫn không có thư mục cha hợp lệ".to_string(),
    })?;

    // 1. Ensure target directory exists
    if !parent.as_os_str().is_empty() {
        fs::create_dir_all(parent).map_err(|e| ConfigError::SaveFailed {
            path: path.to_string_lossy().to_string(),
            details: format!("Không thể tạo thư mục lưu trữ: {e}"),
        })?;
    }

    // 2. Serialize JSON
    let json_bytes = serde_json::to_vec_pretty(data).map_err(|e| ConfigError::SaveFailed {
        path: path.to_string_lossy().to_string(),
        details: format!("Lỗi tuần tự hóa JSON: {e}"),
    })?;

    // 3. Generate unique temporary filename in the exact same directory (avoids EXDEV)
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    let seq = ATOMIC_COUNTER.fetch_add(1, Ordering::Relaxed);
    let base_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");
    let temp_name = format!(".{base_name}.tmp.{pid}.{nonce}.{seq}");
    let temp_path = parent.join(temp_name);

    // 4. Write to temp file and sync_all
    let write_result = (|| -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp_path)?;
        file.write_all(&json_bytes)?;
        file.sync_all()?;
        Ok(())
    })();

    if let Err(e) = write_result {
        let _ = fs::remove_file(&temp_path);
        return Err(ConfigError::SaveFailed {
            path: path.to_string_lossy().to_string(),
            details: format!("Không thể ghi tệp tạm thời: {e}"),
        });
    }

    // 5. Atomic rename to target destination
    if let Err(e) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(ConfigError::SaveFailed {
            path: path.to_string_lossy().to_string(),
            details: format!("Đổi tên tệp nguyên tử thất bại: {e}"),
        });
    }

    Ok(())
}

/// Atomically read and deserialize a JSON file from disk.
pub fn atomic_load_json<T: DeserializeOwned>(path: &Path) -> Result<T, ConfigError> {
    if !path.exists() {
        return Err(ConfigError::NotFound(path.to_string_lossy().to_string()));
    }

    let content = fs::read_to_string(path).map_err(|e| {
        ConfigError::InvalidFormat(format!("Không thể đọc tệp {}: {e}", path.display()))
    })?;

    let data: T = serde_json::from_str(&content).map_err(|e| {
        ConfigError::InvalidFormat(format!(
            "Cấu trúc JSON không hợp lệ tại {}: {e}",
            path.display()
        ))
    })?;

    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct DummyPayload {
        pub title: String,
        pub count: u32,
    }

    #[test]
    fn test_atomic_save_and_load_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("neuroclip_test_{}", std::process::id()));
        let file_path = temp_dir.join("test_config.json");

        let payload = DummyPayload {
            title: "NeuroClip Test".to_string(),
            count: 42,
        };

        atomic_save_json(&file_path, &payload)?;
        assert!(file_path.exists());

        let loaded: DummyPayload = atomic_load_json(&file_path)?;
        assert_eq!(loaded, payload);

        // Clean up
        let _ = fs::remove_file(&file_path);
        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_atomic_load_non_existent_file() {
        let non_existent = Path::new("non_existent_never_created_99999.json");
        let result: Result<DummyPayload, ConfigError> = atomic_load_json(non_existent);
        assert!(matches!(result, Err(ConfigError::NotFound(_))));
    }
}
