use crate::managers::llm::{DownloadResult, LlmModelInfo, LlmModelManager};
use crate::settings::{get_settings, write_settings, CommandsLlmProvider, VoiceCommand};
use log::info;
use once_cell::sync::Lazy;
use std::sync::{Arc, Mutex, MutexGuard};
use tauri::{AppHandle, State};
use uuid::Uuid;

/// Serialises read-modify-write of voice commands in settings so concurrent
/// IPC calls can't lose updates (the plugin-store IO is not transactional).
static COMMANDS_MUTATION_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

/// Recover from a poisoned mutex — the guarded data is `()`, so there is
/// nothing to corrupt; continuing is always safe.
fn lock_commands() -> MutexGuard<'static, ()> {
    COMMANDS_MUTATION_LOCK
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

fn fmt_err(e: anyhow::Error) -> String {
    // {:#} prints the full anyhow error chain.
    format!("{:#}", e)
}

// ── LLM Model Management ───────────────────────────────────────────

#[tauri::command]
#[specta::specta]
pub async fn get_available_llm_models(
    llm_manager: State<'_, Arc<LlmModelManager>>,
) -> Result<Vec<LlmModelInfo>, String> {
    // Runs filesystem stat calls during reconciliation — keep it off the
    // async executor so slow filesystems don't stall IPC.
    let mgr = (*llm_manager).clone();
    tokio::task::spawn_blocking(move || mgr.get_available_models())
        .await
        .map_err(|e| format!("spawn_blocking join error: {e}"))
}

#[tauri::command]
#[specta::specta]
pub async fn download_llm_model(
    llm_manager: State<'_, Arc<LlmModelManager>>,
    model_id: String,
) -> Result<DownloadResult, String> {
    llm_manager
        .download_model(&model_id)
        .await
        .map_err(fmt_err)
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_llm_download(
    llm_manager: State<'_, Arc<LlmModelManager>>,
    model_id: String,
) -> Result<(), String> {
    llm_manager.cancel_download(&model_id).map_err(fmt_err)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_llm_model(
    llm_manager: State<'_, Arc<LlmModelManager>>,
    model_id: String,
) -> Result<(), String> {
    // `delete_model` acquires `load_lock` (std Mutex) and does filesystem
    // unlinks; it can stall for seconds behind an in-progress load. Run it
    // on the blocking pool to keep the async executor responsive.
    let mgr = (*llm_manager).clone();
    tokio::task::spawn_blocking(move || mgr.delete_model(&model_id))
        .await
        .map_err(|e| format!("spawn_blocking join error: {e}"))?
        .map_err(fmt_err)
}

#[tauri::command]
#[specta::specta]
pub async fn set_active_llm_model(
    app_handle: AppHandle,
    llm_manager: State<'_, Arc<LlmModelManager>>,
    model_id: String,
) -> Result<(), String> {
    info!("set_active_llm_model invoked for id='{}'", model_id);
    // Loading the model is CPU-heavy; run off the async executor.
    let llm_manager_clone = (*llm_manager).clone();
    let id = model_id.clone();
    tokio::task::spawn_blocking(move || llm_manager_clone.load_model(&id))
        .await
        .map_err(|e| format!("spawn_blocking join error: {e}"))?
        .map_err(fmt_err)?;
    info!("set_active_llm_model: model '{}' loaded", model_id);

    let ah = app_handle.clone();
    tokio::task::spawn_blocking(move || {
        let _guard = lock_commands();
        let mut settings = get_settings(&ah);
        settings.commands_llm_model_id = Some(model_id);
        write_settings(&ah, settings);
    })
    .await
    .map_err(|e| format!("spawn_blocking join error: {e}"))?;

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_llm_model_status(
    llm_manager: State<'_, Arc<LlmModelManager>>,
) -> Result<Option<String>, String> {
    Ok(llm_manager.get_loaded_model_id())
}

// ── Voice Commands CRUD ─────────────────────────────────────────────

#[tauri::command]
#[specta::specta]
pub async fn get_voice_commands(app_handle: AppHandle) -> Result<Vec<VoiceCommand>, String> {
    tokio::task::spawn_blocking(move || {
        let settings = get_settings(&app_handle);
        settings.commands
    })
    .await
    .map_err(|e| format!("spawn_blocking join error: {e}"))
}

#[tauri::command]
#[specta::specta]
pub async fn add_voice_command(
    app_handle: AppHandle,
    keyword: String,
    prompt: String,
) -> Result<VoiceCommand, String> {
    tokio::task::spawn_blocking(move || -> Result<VoiceCommand, String> {
        let command = VoiceCommand {
            id: Uuid::new_v4().to_string(),
            keyword,
            prompt,
            enabled: true,
        };
        let _guard = lock_commands();
        let mut settings = get_settings(&app_handle);
        settings.commands.push(command.clone());
        write_settings(&app_handle, settings);
        Ok(command)
    })
    .await
    .map_err(|e| format!("spawn_blocking join error: {e}"))?
}

#[tauri::command]
#[specta::specta]
pub async fn update_voice_command(
    app_handle: AppHandle,
    id: String,
    keyword: String,
    prompt: String,
    enabled: bool,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let _guard = lock_commands();
        let mut settings = get_settings(&app_handle);
        if let Some(cmd) = settings.commands.iter_mut().find(|c| c.id == id) {
            cmd.keyword = keyword;
            cmd.prompt = prompt;
            cmd.enabled = enabled;
            write_settings(&app_handle, settings);
            Ok(())
        } else {
            Err(format!("Voice command not found: {}", id))
        }
    })
    .await
    .map_err(|e| format!("spawn_blocking join error: {e}"))?
}

#[tauri::command]
#[specta::specta]
pub async fn delete_voice_command(app_handle: AppHandle, id: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let _guard = lock_commands();
        let mut settings = get_settings(&app_handle);
        let before_len = settings.commands.len();
        settings.commands.retain(|c| c.id != id);
        if settings.commands.len() == before_len {
            return Err(format!("Voice command not found: {}", id));
        }
        write_settings(&app_handle, settings);
        Ok(())
    })
    .await
    .map_err(|e| format!("spawn_blocking join error: {e}"))?
}

// ── Commands Settings Setters ───────────────────────────────────────────

#[tauri::command]
#[specta::specta]
pub fn change_commands_enabled_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let _guard = lock_commands();
    let mut settings = get_settings(&app);
    settings.commands_enabled = enabled;
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_commands_llm_provider_setting(
    app: AppHandle,
    provider: CommandsLlmProvider,
) -> Result<(), String> {
    let _guard = lock_commands();
    let mut settings = get_settings(&app);
    settings.commands_llm_provider = provider;
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_anthropic_api_key_setting(app: AppHandle, key: Option<String>) -> Result<(), String> {
    let _guard = lock_commands();
    let mut settings = get_settings(&app);
    // Normalise empty/whitespace-only input to None so the backend never has
    // to defend against blank strings later.
    settings.anthropic_api_key = key.and_then(|k| {
        let trimmed = k.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_anthropic_model_setting(app: AppHandle, model: String) -> Result<(), String> {
    let _guard = lock_commands();
    let mut settings = get_settings(&app);
    settings.anthropic_model = model;
    write_settings(&app, settings);
    Ok(())
}
