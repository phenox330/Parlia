use crate::managers::llm::{LlmModelInfo, LlmModelManager};
use crate::settings::{get_settings, write_settings, VoiceCommand};
use std::sync::Arc;
use tauri::{AppHandle, State};

// ── LLM Model Management ───────────────────────────────────────────

#[tauri::command]
#[specta::specta]
pub async fn get_available_llm_models(
    llm_manager: State<'_, Arc<LlmModelManager>>,
) -> Result<Vec<LlmModelInfo>, String> {
    Ok(llm_manager.get_available_models())
}

#[tauri::command]
#[specta::specta]
pub async fn download_llm_model(
    llm_manager: State<'_, Arc<LlmModelManager>>,
    model_id: String,
) -> Result<(), String> {
    llm_manager
        .download_model(&model_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_llm_download(
    llm_manager: State<'_, Arc<LlmModelManager>>,
    model_id: String,
) -> Result<(), String> {
    llm_manager
        .cancel_download(&model_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_llm_model(
    llm_manager: State<'_, Arc<LlmModelManager>>,
    model_id: String,
) -> Result<(), String> {
    llm_manager
        .delete_model(&model_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn set_active_llm_model(
    app_handle: AppHandle,
    llm_manager: State<'_, Arc<LlmModelManager>>,
    model_id: String,
) -> Result<(), String> {
    llm_manager
        .load_model(&model_id)
        .map_err(|e| e.to_string())?;

    let mut settings = get_settings(&app_handle);
    settings.commands_llm_model_id = Some(model_id);
    write_settings(&app_handle, settings);

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
    let settings = get_settings(&app_handle);
    Ok(settings.commands)
}

#[tauri::command]
#[specta::specta]
pub async fn add_voice_command(
    app_handle: AppHandle,
    keyword: String,
    prompt: String,
) -> Result<VoiceCommand, String> {
    let command = VoiceCommand {
        id: uuid_v4(),
        keyword,
        prompt,
        enabled: true,
    };

    let mut settings = get_settings(&app_handle);
    settings.commands.push(command.clone());
    write_settings(&app_handle, settings);

    Ok(command)
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
}

#[tauri::command]
#[specta::specta]
pub async fn delete_voice_command(app_handle: AppHandle, id: String) -> Result<(), String> {
    let mut settings = get_settings(&app_handle);
    let before_len = settings.commands.len();
    settings.commands.retain(|c| c.id != id);
    if settings.commands.len() == before_len {
        return Err(format!("Voice command not found: {}", id));
    }
    write_settings(&app_handle, settings);
    Ok(())
}

/// Simple UUID v4 generator without external dependency.
fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    // Use timestamp + random-ish bits for a unique-enough ID
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        (now >> 96) as u32,
        (now >> 80) as u16,
        (now >> 64) as u16 & 0x0FFF,
        ((now >> 48) as u16 & 0x3FFF) | 0x8000,
        now as u64 & 0xFFFFFFFFFFFF,
    )
}
