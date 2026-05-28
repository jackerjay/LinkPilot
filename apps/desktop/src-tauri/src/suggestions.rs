//! Tauri commands backing the Rules-page "Suggestions" surface and the
//! Settings page "Behavior recording" controls. The on-disk store
//! itself lives in `linkpilot-core::observations` — this layer just
//! marshals errors to strings for the JSON IPC and threads the
//! `AppState` reference.

use std::path::PathBuf;

use linkpilot_core::observations::Suggestion;
use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub fn suggestions_list(state: State<'_, AppState>) -> Result<Vec<Suggestion>, String> {
    state
        .observations
        .list_suggestions()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn suggestions_dismiss(
    state: State<'_, AppState>,
    host: String,
    browser_id: String,
    profile_id: Option<String>,
) -> Result<(), String> {
    state
        .observations
        .dismiss(&host, &browser_id, profile_id.as_deref())
        .map_err(|e| e.to_string())
}

/// Wipe every recorded observation. Existing dismissals stay (they age
/// out after 30 days regardless).
#[tauri::command]
pub fn observations_clear(state: State<'_, AppState>) -> Result<(), String> {
    state
        .observations
        .clear_observations()
        .map_err(|e| e.to_string())
}

/// Copy the raw NDJSON log to a user-chosen path. Used by the
/// Settings "Export" button and the future LLM ingestion pipeline.
#[tauri::command]
pub fn observations_export(state: State<'_, AppState>, dest: String) -> Result<(), String> {
    let path = PathBuf::from(dest);
    state.observations.export(&path).map_err(|e| e.to_string())
}
