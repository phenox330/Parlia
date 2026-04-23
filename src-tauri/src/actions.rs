use crate::audio_feedback::{play_feedback_sound, play_feedback_sound_blocking, SoundType};
use crate::managers::audio::AudioRecordingManager;
use crate::managers::history::HistoryManager;
use crate::managers::transcription::TranscriptionManager;
use crate::settings::{get_settings, AppSettings, CommandsLlmProvider};
use crate::shortcut;
use crate::tray::{change_tray_icon, TrayIconState};
use crate::utils::{
    self, show_processing_overlay, show_recording_overlay, show_transcribing_overlay,
};
use crate::TranscriptionCoordinator;
use ferrous_opencc::{config::BuiltinConfig, OpenCC};
use log::{debug, error};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tauri::AppHandle;
use tauri::Emitter;
use tauri::Manager;

/// Drop guard that resets user-visible state (overlay + tray) and notifies
/// the [`TranscriptionCoordinator`] when the transcription pipeline finishes
/// — whether it completes normally, returns early, or panics. Centralising
/// cleanup here means every branch of `stop` automatically gets consistent
/// teardown and we don't have to remember to hide the overlay in each one.
struct FinishGuard(AppHandle);
impl Drop for FinishGuard {
    fn drop(&mut self) {
        utils::hide_recording_overlay(&self.0);
        change_tray_icon(&self.0, TrayIconState::Idle);
        if let Some(c) = self.0.try_state::<TranscriptionCoordinator>() {
            c.notify_processing_finished();
        }
    }
}

/// If the transcription starts with an enabled voice-command keyword, run
/// the configured LLM on the remainder and return the transformed text.
/// Returns `None` when no command matches or when no LLM model is active.
/// LLM errors are logged and surfaced as `None` so the caller falls back to
/// the raw transcription.
async fn apply_voice_command(ah: &AppHandle, settings: &AppSettings, text: &str) -> Option<String> {
    if settings.commands.is_empty() {
        return None;
    }

    // Match and slice in the SAME lowercased domain so the byte offset
    // always lands on a valid UTF-8 boundary. Mixing domains (lowercased
    // `starts_with` + raw-string byte slice) panics for any char whose
    // lowercase form differs in byte length (e.g. Turkish `İ` → `i̇`).
    let normalized = text.trim().to_lowercase();

    // Collect every keyword that prefixes the transcription AND is followed
    // by a word boundary (whitespace / punctuation / end-of-string). Without
    // the boundary check, `"to"` would match `"tomato please"` and hijack
    // unrelated transcriptions. Pick the *longest* keyword so a more specific
    // match (`"open browser"`) wins over a more generic one (`"open"`).
    let (matched_cmd, rest) = settings
        .commands
        .iter()
        .filter(|c| c.enabled && !c.keyword.is_empty())
        .filter_map(|c| {
            let kw = c.keyword.trim().to_lowercase();
            let tail = normalized.strip_prefix(&kw)?;
            let boundary_ok = tail.is_empty()
                || tail
                    .chars()
                    .next()
                    .map(|ch| ch.is_whitespace() || ch.is_ascii_punctuation())
                    .unwrap_or(true);
            if !boundary_ok {
                return None;
            }
            Some((c, kw.chars().count(), tail.trim().to_string()))
        })
        .max_by_key(|(_, kw_len, _)| *kw_len)
        .map(|(c, _, rest)| (c, rest))?;

    if rest.is_empty() {
        return None;
    }

    // A keyword matched. If the master toggle is off, tell the user so they
    // understand why their transcription wasn't transformed — the silent
    // fallback to raw text was impossible to distinguish from "keyword didn't
    // match" before.
    if !settings.commands_enabled {
        let _ = ah.emit(
            "llm-error",
            format!(
                "Voice commands are disabled. Enable them in Settings > Commands to run '{}'.",
                matched_cmd.keyword
            ),
        );
        return None;
    }

    debug!(
        "Voice command matched: '{}', provider={:?}, processing",
        matched_cmd.keyword, settings.commands_llm_provider
    );
    show_processing_overlay(ah);

    let prompt = matched_cmd.prompt.clone();

    let llm_result: anyhow::Result<String> = match settings.commands_llm_provider {
        CommandsLlmProvider::Anthropic => {
            let key = match settings.anthropic_api_key.as_deref() {
                Some(k) if !k.trim().is_empty() => k.to_string(),
                _ => {
                    let _ = ah.emit(
                        "llm-error",
                        "Anthropic API key is missing. Add it in Settings > Commands."
                            .to_string(),
                    );
                    return None;
                }
            };
            let model = settings.anthropic_model.clone();
            crate::cloud_llm::generate_anthropic(&key, &model, &prompt, &rest).await
        }
        CommandsLlmProvider::Local => {
            let llm_state = ah.try_state::<Arc<crate::managers::llm::LlmModelManager>>()?;
            let llm_mgr: Arc<crate::managers::llm::LlmModelManager> = (*llm_state).clone();
            let model_id_opt = settings.commands_llm_model_id.clone();
            if model_id_opt.is_none() {
                let _ = ah.emit(
                    "llm-error",
                    "No local LLM model is active. Activate one in Settings > Commands."
                        .to_string(),
                );
                return None;
            }
            // Local LLM is CPU-heavy; keep it off the async executor.
            let join = tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<String> {
                if let Some(model_id) = &model_id_opt {
                    if llm_mgr.get_loaded_model_id().as_deref() != Some(model_id) {
                        llm_mgr.load_model(model_id)?;
                    }
                }
                llm_mgr.generate(&prompt, &rest)
            })
            .await;
            match join {
                Ok(r) => r,
                Err(e) => Err(anyhow::anyhow!("Voice command worker crashed: {}", e)),
            }
        }
    };

    match llm_result {
        Ok(result) => {
            debug!("Voice command LLM output: {} chars", result.len());
            Some(result)
        }
        Err(e) => {
            error!("Voice command LLM failed: {:#}", e);
            let _ = ah.emit("llm-error", format!("Voice command failed: {:#}", e));
            None
        }
    }
}

pub trait ShortcutAction: Send + Sync {
    fn start(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str);
    fn stop(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str);
}

struct TranscribeAction;

async fn maybe_convert_chinese_variant(
    settings: &AppSettings,
    transcription: &str,
) -> Option<String> {
    let is_simplified = settings.selected_language == "zh-Hans";
    let is_traditional = settings.selected_language == "zh-Hant";

    if !is_simplified && !is_traditional {
        debug!("selected_language is not Simplified or Traditional Chinese; skipping translation");
        return None;
    }

    debug!(
        "Starting Chinese translation using OpenCC for language: {}",
        settings.selected_language
    );

    let config = if is_simplified {
        BuiltinConfig::Tw2sp
    } else {
        BuiltinConfig::S2twp
    };

    match OpenCC::from_config(config) {
        Ok(converter) => {
            let converted = converter.convert(transcription);
            debug!(
                "OpenCC translation completed. Input length: {}, Output length: {}",
                transcription.len(),
                converted.len()
            );
            Some(converted)
        }
        Err(e) => {
            error!("Failed to initialize OpenCC converter: {}. Falling back to original transcription.", e);
            None
        }
    }
}

impl ShortcutAction for TranscribeAction {
    fn start(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        let start_time = Instant::now();
        debug!("TranscribeAction::start called for binding: {}", binding_id);

        let tm = app.state::<Arc<TranscriptionManager>>();
        tm.initiate_model_load();

        let binding_id = binding_id.to_string();
        change_tray_icon(app, TrayIconState::Recording);
        show_recording_overlay(app);

        let rm = app.state::<Arc<AudioRecordingManager>>();

        let settings = get_settings(app);
        let is_always_on = settings.always_on_microphone;
        debug!("Microphone mode - always_on: {}", is_always_on);

        let mut recording_started = false;
        if is_always_on {
            debug!("Always-on mode: Playing audio feedback immediately");
            let rm_clone = Arc::clone(&rm);
            let app_clone = app.clone();
            // play_feedback_sound_blocking returns immediately if audio feedback
            // is disabled, so we can always run apply_mute after it in the same
            // thread to keep mute sequencing consistent.
            std::thread::spawn(move || {
                play_feedback_sound_blocking(&app_clone, SoundType::Start);
                rm_clone.apply_mute();
            });

            recording_started = rm.try_start_recording(&binding_id);
            debug!("Recording started: {}", recording_started);
        } else {
            debug!("On-demand mode: Starting recording first, then audio feedback");
            let recording_start_time = Instant::now();
            if rm.try_start_recording(&binding_id) {
                recording_started = true;
                debug!("Recording started in {:?}", recording_start_time.elapsed());
                let app_clone = app.clone();
                let rm_clone = Arc::clone(&rm);
                std::thread::spawn(move || {
                    // Give the microphone stream a moment to become active before
                    // the audio feedback plays, so the sound isn't captured.
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    debug!("Handling delayed audio feedback/mute sequence");
                    play_feedback_sound_blocking(&app_clone, SoundType::Start);
                    rm_clone.apply_mute();
                });
            } else {
                debug!("Failed to start recording");
            }
        }

        if recording_started {
            // Registered in a separate task to avoid deadlocking the shortcut
            // manager while this handler still holds its lock.
            shortcut::register_cancel_shortcut(app);
        }

        debug!(
            "TranscribeAction::start completed in {:?}",
            start_time.elapsed()
        );
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        shortcut::unregister_cancel_shortcut(app);

        let stop_time = Instant::now();
        debug!("TranscribeAction::stop called for binding: {}", binding_id);

        let ah = app.clone();
        let rm = Arc::clone(&app.state::<Arc<AudioRecordingManager>>());
        let tm = Arc::clone(&app.state::<Arc<TranscriptionManager>>());
        let hm = Arc::clone(&app.state::<Arc<HistoryManager>>());

        change_tray_icon(app, TrayIconState::Transcribing);
        show_transcribing_overlay(app);

        // Unmute first so the stop sound is audible.
        rm.remove_mute();
        play_feedback_sound(app, SoundType::Stop);

        let binding_id = binding_id.to_string();

        // Construct the FinishGuard *before* spawning so cleanup runs even if
        // the future is dropped before its first poll (e.g. runtime shutdown
        // while the stop synchronous prologue has already switched the tray
        // to `Transcribing`). The guard is then moved into the closure so its
        // Drop fires at the end of the async pipeline on the happy path.
        let guard = FinishGuard(ah.clone());

        tauri::async_runtime::spawn(async move {
            let _guard = guard;
            debug!("Starting async transcription task for binding: {}", binding_id);

            let stop_recording_time = Instant::now();
            let Some(samples) = rm.stop_recording(&binding_id) else {
                debug!("No samples retrieved from recording stop");
                return;
            };
            debug!(
                "Recording stopped and samples retrieved in {:?}, sample count: {}",
                stop_recording_time.elapsed(),
                samples.len()
            );

            let transcription_time = Instant::now();
            let samples_for_history = samples.clone();
            // transcribe is CPU-bound (Whisper/Parakeet decode) and synchronous;
            // running it directly on the async executor would stall other
            // tasks scheduled on this worker (progress emits, IPC commands).
            let tm_for_transcribe = Arc::clone(&tm);
            let transcription = match tauri::async_runtime::spawn_blocking(move || {
                tm_for_transcribe.transcribe(samples)
            })
            .await
            {
                Ok(Ok(t)) => t,
                Ok(Err(err)) => {
                    debug!("Global Shortcut Transcription error: {}", err);
                    return;
                }
                Err(join_err) => {
                    error!("Transcription worker panicked: {}", join_err);
                    return;
                }
            };
            debug!(
                "Transcription completed in {:?}: '{}'",
                transcription_time.elapsed(),
                transcription
            );
            if transcription.is_empty() {
                return;
            }

            let settings = get_settings(&ah);
            let mut final_text = transcription.clone();

            if let Some(converted) = maybe_convert_chinese_variant(&settings, &final_text).await {
                final_text = converted;
            }

            if let Some(processed) = apply_voice_command(&ah, &settings, &final_text).await {
                final_text = processed;
            }

            let processed_text = if final_text != transcription {
                Some(final_text.clone())
            } else {
                None
            };

            let hm_clone = Arc::clone(&hm);
            let transcription_for_history = transcription.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = hm_clone
                    .save_transcription(
                        samples_for_history,
                        transcription_for_history,
                        processed_text,
                        None,
                    )
                    .await
                {
                    error!("Failed to save transcription to history: {}", e);
                }
            });

            // Paste must run on the main thread (platform accessibility APIs).
            let ah_clone = ah.clone();
            let paste_time = Instant::now();
            if let Err(e) = ah.run_on_main_thread(move || {
                match utils::paste(final_text, ah_clone.clone()) {
                    Ok(()) => debug!("Text pasted successfully in {:?}", paste_time.elapsed()),
                    Err(e) => error!("Failed to paste transcription: {}", e),
                }
            }) {
                error!("Failed to run paste on main thread: {:?}", e);
            }
        });

        debug!(
            "TranscribeAction::stop completed in {:?}",
            stop_time.elapsed()
        );
    }
}

struct CancelAction;

impl ShortcutAction for CancelAction {
    fn start(&self, app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        utils::cancel_current_operation(app);
    }

    fn stop(&self, _app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        // Nothing to do on stop for cancel
    }
}

pub static ACTION_MAP: Lazy<HashMap<String, Arc<dyn ShortcutAction>>> = Lazy::new(|| {
    let mut map = HashMap::new();
    map.insert(
        "transcribe".to_string(),
        Arc::new(TranscribeAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "cancel".to_string(),
        Arc::new(CancelAction) as Arc<dyn ShortcutAction>,
    );
    map
});
