#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]

pub const DEFAULT_WINDOW_WIDTH: f64 = 1280.0;
pub const DEFAULT_WINDOW_HEIGHT: f64 = 800.0;
pub const MIN_WINDOW_WIDTH: f64 = 1024.0;
pub const MIN_WINDOW_HEIGHT: f64 = 640.0;
pub const SOLID_BACKGROUND_FALLBACK: &str = "#12131C";

/// Health check ping/pong command
#[tauri::command]
pub fn ping() -> Result<String, String> {
    Ok("pong".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![ping]);

    if let Err(err) = builder.run(tauri::generate_context!()) {
        eprintln!("error while running tauri application: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ping_pong_response() {
        let result = ping();
        assert!(result.is_ok());
        if let Ok(value) = result {
            assert_eq!(value, "pong");
        }
    }

    #[test]
    fn test_window_geometry_constants() {
        assert!((DEFAULT_WINDOW_WIDTH - 1280.0).abs() < f64::EPSILON);
        assert!((DEFAULT_WINDOW_HEIGHT - 800.0).abs() < f64::EPSILON);
        assert!((MIN_WINDOW_WIDTH - 1024.0).abs() < f64::EPSILON);
        assert!((MIN_WINDOW_HEIGHT - 640.0).abs() < f64::EPSILON);
        assert_eq!(SOLID_BACKGROUND_FALLBACK, "#12131C");
    }
}
