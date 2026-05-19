use log::{debug, warn};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use specta::Type;
use std::collections::HashMap;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;


#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

// Custom deserializer to handle both old numeric format (1-5) and new string format ("trace", "debug", etc.)
impl<'de> Deserialize<'de> for LogLevel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct LogLevelVisitor;

        impl<'de> Visitor<'de> for LogLevelVisitor {
            type Value = LogLevel;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or integer representing log level")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<LogLevel, E> {
                match value.to_lowercase().as_str() {
                    "trace" => Ok(LogLevel::Trace),
                    "debug" => Ok(LogLevel::Debug),
                    "info" => Ok(LogLevel::Info),
                    "warn" => Ok(LogLevel::Warn),
                    "error" => Ok(LogLevel::Error),
                    _ => Err(E::unknown_variant(
                        value,
                        &["trace", "debug", "info", "warn", "error"],
                    )),
                }
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<LogLevel, E> {
                match value {
                    1 => Ok(LogLevel::Trace),
                    2 => Ok(LogLevel::Debug),
                    3 => Ok(LogLevel::Info),
                    4 => Ok(LogLevel::Warn),
                    5 => Ok(LogLevel::Error),
                    _ => Err(E::invalid_value(de::Unexpected::Unsigned(value), &"1-5")),
                }
            }
        }

        deserializer.deserialize_any(LogLevelVisitor)
    }
}

impl From<LogLevel> for tauri_plugin_log::LogLevel {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => tauri_plugin_log::LogLevel::Trace,
            LogLevel::Debug => tauri_plugin_log::LogLevel::Debug,
            LogLevel::Info => tauri_plugin_log::LogLevel::Info,
            LogLevel::Warn => tauri_plugin_log::LogLevel::Warn,
            LogLevel::Error => tauri_plugin_log::LogLevel::Error,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct ShortcutBinding {
    pub id: String,
    pub name: String,
    pub description: String,
    pub default_binding: String,
    pub current_binding: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "lowercase")]
pub enum OverlayPosition {
    None,
    Top,
    Bottom,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum ModelUnloadTimeout {
    Never,
    Immediately,
    Min2,
    Min5,
    Min10,
    Min15,
    Hour1,
    Sec5, // Debug mode only
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum PasteMethod {
    CtrlV,
    Direct,
    None,
    ShiftInsert,
    CtrlShiftV,
    ExternalScript,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum ClipboardHandling {
    DontModify,
    CopyToClipboard,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum AutoSubmitKey {
    Enter,
    CtrlEnter,
    CmdEnter,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum RecordingRetentionPeriod {
    Never,
    PreserveLimit,
    Days3,
    Weeks2,
    Months3,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum KeyboardImplementation {
    Tauri,
    HandyKeys,
}

impl Default for KeyboardImplementation {
    fn default() -> Self {
        // Default to HandyKeys only on macOS where it's well-tested.
        // Windows and Linux use Tauri by default (handy-keys not sufficiently tested yet).
        #[cfg(target_os = "macos")]
        return KeyboardImplementation::HandyKeys;
        #[cfg(not(target_os = "macos"))]
        return KeyboardImplementation::Tauri;
    }
}

impl Default for ModelUnloadTimeout {
    fn default() -> Self {
        ModelUnloadTimeout::Never
    }
}

impl Default for PasteMethod {
    fn default() -> Self {
        // Default to CtrlV for macOS and Windows, Direct for Linux
        #[cfg(target_os = "linux")]
        return PasteMethod::Direct;
        #[cfg(not(target_os = "linux"))]
        return PasteMethod::CtrlV;
    }
}

impl Default for ClipboardHandling {
    fn default() -> Self {
        ClipboardHandling::DontModify
    }
}

impl Default for AutoSubmitKey {
    fn default() -> Self {
        AutoSubmitKey::Enter
    }
}

impl ModelUnloadTimeout {
    pub fn to_minutes(self) -> Option<u64> {
        match self {
            ModelUnloadTimeout::Never => None,
            ModelUnloadTimeout::Immediately => Some(0), // Special case for immediate unloading
            ModelUnloadTimeout::Min2 => Some(2),
            ModelUnloadTimeout::Min5 => Some(5),
            ModelUnloadTimeout::Min10 => Some(10),
            ModelUnloadTimeout::Min15 => Some(15),
            ModelUnloadTimeout::Hour1 => Some(60),
            ModelUnloadTimeout::Sec5 => Some(0), // Special case for debug - handled separately
        }
    }

    pub fn to_seconds(self) -> Option<u64> {
        match self {
            ModelUnloadTimeout::Never => None,
            ModelUnloadTimeout::Immediately => Some(0), // Special case for immediate unloading
            ModelUnloadTimeout::Sec5 => Some(5),
            _ => self.to_minutes().map(|m| m * 60),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum SoundTheme {
    Marimba,
    Pop,
    Custom,
}

impl SoundTheme {
    fn as_str(&self) -> &'static str {
        match self {
            SoundTheme::Marimba => "marimba",
            SoundTheme::Pop => "pop",
            SoundTheme::Custom => "custom",
        }
    }

    pub fn to_start_path(&self) -> String {
        format!("resources/{}_start.wav", self.as_str())
    }

    pub fn to_stop_path(&self) -> String {
        format!("resources/{}_stop.wav", self.as_str())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum TypingTool {
    Auto,
    Wtype,
    Kwtype,
    Dotool,
    Ydotool,
    Xdotool,
}

impl Default for TypingTool {
    fn default() -> Self {
        TypingTool::Auto
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VoiceCommand {
    pub id: String,
    pub keyword: String,
    pub prompt: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CommandsLlmProvider {
    /// Parlia Cloud — hosted proxy that relays to Groq (Llama 3.1 8B Instant)
    /// with a hardcoded shared Bearer token. Zero config for the user.
    /// Replace the shared-token auth with magic-link user auth before scaling.
    Parlia,
    /// On-device inference via llama.cpp. Known to crash during model load
    /// on some macOS aarch64 builds; kept as an opt-in for future fixes.
    Local,
    /// Cloud inference via Anthropic's Messages API. Requires an API key.
    Anthropic,
    /// Any OpenAI-compatible chat/completions endpoint — covers Ollama,
    /// LM Studio, Groq, OpenRouter, DeepSeek, vLLM, etc. Needs base URL +
    /// model name; the API key is optional (blank for local Ollama).
    Custom,
}

impl Default for CommandsLlmProvider {
    fn default() -> Self {
        CommandsLlmProvider::Parlia
    }
}

/* still handy for composing the initial JSON in the store ------------- */
/// `Debug` is hand-rolled below so that `anthropic_api_key` and
/// `openai_compat_api_key` are redacted from any `{:?}` formatting — the
/// settings struct is `debug!`-logged on every load, and bug reports
/// routinely include log files. Drop this impl only once the keys are
/// out of the struct entirely (Keychain migration).
#[derive(Serialize, Deserialize, Clone, Type)]
pub struct AppSettings {
    pub bindings: HashMap<String, ShortcutBinding>,
    pub push_to_talk: bool,
    pub audio_feedback: bool,
    #[serde(default = "default_audio_feedback_volume")]
    pub audio_feedback_volume: f32,
    #[serde(default = "default_sound_theme")]
    pub sound_theme: SoundTheme,
    #[serde(default = "default_start_hidden")]
    pub start_hidden: bool,
    #[serde(default = "default_autostart_enabled")]
    pub autostart_enabled: bool,
    #[serde(default = "default_update_checks_enabled")]
    pub update_checks_enabled: bool,
    #[serde(default = "default_model")]
    pub selected_model: String,
    #[serde(default = "default_always_on_microphone")]
    pub always_on_microphone: bool,
    #[serde(default)]
    pub selected_microphone: Option<String>,
    #[serde(default)]
    pub clamshell_microphone: Option<String>,
    #[serde(default)]
    pub selected_output_device: Option<String>,
    #[serde(default = "default_translate_to_english")]
    pub translate_to_english: bool,
    #[serde(default = "default_selected_language")]
    pub selected_language: String,
    #[serde(default = "default_overlay_position")]
    pub overlay_position: OverlayPosition,
    #[serde(default = "default_debug_mode")]
    pub debug_mode: bool,
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,
    #[serde(default)]
    pub custom_words: Vec<String>,
    #[serde(default)]
    pub model_unload_timeout: ModelUnloadTimeout,
    #[serde(default = "default_word_correction_threshold")]
    pub word_correction_threshold: f64,
    #[serde(default = "default_history_limit")]
    pub history_limit: usize,
    #[serde(default = "default_recording_retention_period")]
    pub recording_retention_period: RecordingRetentionPeriod,
    #[serde(default)]
    pub paste_method: PasteMethod,
    #[serde(default)]
    pub clipboard_handling: ClipboardHandling,
    #[serde(default = "default_auto_submit")]
    pub auto_submit: bool,
    #[serde(default)]
    pub auto_submit_key: AutoSubmitKey,
    #[serde(default)]
    pub mute_while_recording: bool,
    #[serde(default)]
    pub append_trailing_space: bool,
    #[serde(default = "default_app_language")]
    pub app_language: String,
    #[serde(default)]
    pub experimental_enabled: bool,
    #[serde(default)]
    pub keyboard_implementation: KeyboardImplementation,
    #[serde(default = "default_show_tray_icon")]
    pub show_tray_icon: bool,
    #[serde(default = "default_paste_delay_ms")]
    pub paste_delay_ms: u64,
    #[serde(default = "default_typing_tool")]
    pub typing_tool: TypingTool,
    pub external_script_path: Option<String>,
    #[serde(default = "default_commands_enabled")]
    pub commands_enabled: bool,
    #[serde(default)]
    pub commands: Vec<VoiceCommand>,
    #[serde(default)]
    pub commands_llm_model_id: Option<String>,
    #[serde(default)]
    pub commands_llm_provider: CommandsLlmProvider,
    /// Migration-only field. From v0.7.14 the Anthropic key lives in the
    /// OS keychain (see `crate::secrets`); this field is read from old
    /// `settings_store.json` files exactly once so `load_or_create_app_settings`
    /// can move the value into the keychain, then it stays `None` forever
    /// (skipped from serialization). Use `secrets::get_secret(Anthropic)`
    /// for runtime reads.
    #[serde(default, skip_serializing)]
    pub anthropic_api_key: Option<String>,
    #[serde(default = "default_anthropic_model")]
    pub anthropic_model: String,
    /// Base URL for the OpenAI-compatible provider, without trailing slash.
    /// Examples: `http://localhost:11434/v1` (Ollama),
    /// `https://api.groq.com/openai/v1`, `https://openrouter.ai/api/v1`.
    #[serde(default)]
    pub openai_compat_base_url: Option<String>,
    /// Migration-only field. See `anthropic_api_key` above — same model: read
    /// once from legacy `settings_store.json`, moved into the OS keychain,
    /// never serialized again. Runtime reads go through
    /// `secrets::get_secret(OpenAiCompat)`.
    #[serde(default, skip_serializing)]
    pub openai_compat_api_key: Option<String>,
    /// Model id as understood by the provider (e.g. `qwen2.5:1.5b`,
    /// `llama-3.1-8b-instant`, `openai/gpt-4o-mini`).
    #[serde(default)]
    pub openai_compat_model: Option<String>,
}

/// Used by the manual `Debug` impl on `AppSettings` to keep API keys out
/// of logs. Returns `"<set>"` / `"<unset>"` rather than the actual value.
fn redact_opt_key(value: &Option<String>) -> &'static str {
    match value.as_deref() {
        Some(s) if !s.is_empty() => "<set>",
        _ => "<unset>",
    }
}

impl std::fmt::Debug for AppSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppSettings")
            .field("bindings", &self.bindings)
            .field("push_to_talk", &self.push_to_talk)
            .field("audio_feedback", &self.audio_feedback)
            .field("audio_feedback_volume", &self.audio_feedback_volume)
            .field("sound_theme", &self.sound_theme)
            .field("start_hidden", &self.start_hidden)
            .field("autostart_enabled", &self.autostart_enabled)
            .field("update_checks_enabled", &self.update_checks_enabled)
            .field("selected_model", &self.selected_model)
            .field("always_on_microphone", &self.always_on_microphone)
            .field("selected_microphone", &self.selected_microphone)
            .field("clamshell_microphone", &self.clamshell_microphone)
            .field("selected_output_device", &self.selected_output_device)
            .field("translate_to_english", &self.translate_to_english)
            .field("selected_language", &self.selected_language)
            .field("overlay_position", &self.overlay_position)
            .field("debug_mode", &self.debug_mode)
            .field("log_level", &self.log_level)
            .field("custom_words", &self.custom_words)
            .field("model_unload_timeout", &self.model_unload_timeout)
            .field("word_correction_threshold", &self.word_correction_threshold)
            .field("history_limit", &self.history_limit)
            .field("recording_retention_period", &self.recording_retention_period)
            .field("paste_method", &self.paste_method)
            .field("clipboard_handling", &self.clipboard_handling)
            .field("auto_submit", &self.auto_submit)
            .field("auto_submit_key", &self.auto_submit_key)
            .field("mute_while_recording", &self.mute_while_recording)
            .field("append_trailing_space", &self.append_trailing_space)
            .field("app_language", &self.app_language)
            .field("experimental_enabled", &self.experimental_enabled)
            .field("keyboard_implementation", &self.keyboard_implementation)
            .field("show_tray_icon", &self.show_tray_icon)
            .field("paste_delay_ms", &self.paste_delay_ms)
            .field("typing_tool", &self.typing_tool)
            .field("external_script_path", &self.external_script_path)
            .field("commands_enabled", &self.commands_enabled)
            .field("commands", &self.commands)
            .field("commands_llm_model_id", &self.commands_llm_model_id)
            .field("commands_llm_provider", &self.commands_llm_provider)
            .field("anthropic_api_key", &redact_opt_key(&self.anthropic_api_key))
            .field("anthropic_model", &self.anthropic_model)
            .field("openai_compat_base_url", &self.openai_compat_base_url)
            .field(
                "openai_compat_api_key",
                &redact_opt_key(&self.openai_compat_api_key),
            )
            .field("openai_compat_model", &self.openai_compat_model)
            .finish()
    }
}

fn default_model() -> String {
    "".to_string()
}

fn default_always_on_microphone() -> bool {
    false
}

fn default_translate_to_english() -> bool {
    false
}

fn default_start_hidden() -> bool {
    false
}

fn default_autostart_enabled() -> bool {
    false
}

fn default_update_checks_enabled() -> bool {
    true
}

fn default_selected_language() -> String {
    "fr".to_string()
}

fn default_overlay_position() -> OverlayPosition {
    #[cfg(target_os = "linux")]
    return OverlayPosition::None;
    #[cfg(not(target_os = "linux"))]
    return OverlayPosition::Bottom;
}

fn default_debug_mode() -> bool {
    false
}

fn default_log_level() -> LogLevel {
    LogLevel::Debug
}

fn default_word_correction_threshold() -> f64 {
    0.18
}

fn default_paste_delay_ms() -> u64 {
    60
}

fn default_auto_submit() -> bool {
    false
}

fn default_history_limit() -> usize {
    5
}

fn default_commands_enabled() -> bool {
    true
}

fn default_recording_retention_period() -> RecordingRetentionPeriod {
    RecordingRetentionPeriod::PreserveLimit
}

fn default_audio_feedback_volume() -> f32 {
    1.0
}

fn default_sound_theme() -> SoundTheme {
    SoundTheme::Marimba
}

fn default_app_language() -> String {
    "fr".to_string()
}

fn default_show_tray_icon() -> bool {
    true
}

fn default_typing_tool() -> TypingTool {
    TypingTool::Auto
}

pub const SETTINGS_STORE_PATH: &str = "settings_store.json";

pub fn get_default_settings() -> AppSettings {
    #[cfg(target_os = "windows")]
    let default_shortcut = "ctrl+space";
    #[cfg(target_os = "macos")]
    let default_shortcut = "option+space";
    #[cfg(target_os = "linux")]
    let default_shortcut = "ctrl+space";
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    let default_shortcut = "alt+space";

    let mut bindings = HashMap::new();
    bindings.insert(
        "transcribe".to_string(),
        ShortcutBinding {
            id: "transcribe".to_string(),
            name: "Transcribe".to_string(),
            description: "Converts your speech into text.".to_string(),
            default_binding: default_shortcut.to_string(),
            current_binding: default_shortcut.to_string(),
        },
    );
    bindings.insert(
        "cancel".to_string(),
        ShortcutBinding {
            id: "cancel".to_string(),
            name: "Cancel".to_string(),
            description: "Cancels the current recording.".to_string(),
            default_binding: "escape".to_string(),
            current_binding: "escape".to_string(),
        },
    );

    AppSettings {
        bindings,
        push_to_talk: true,
        audio_feedback: false,
        audio_feedback_volume: default_audio_feedback_volume(),
        sound_theme: default_sound_theme(),
        start_hidden: default_start_hidden(),
        autostart_enabled: default_autostart_enabled(),
        update_checks_enabled: default_update_checks_enabled(),
        selected_model: "".to_string(),
        always_on_microphone: false,
        selected_microphone: None,
        clamshell_microphone: None,
        selected_output_device: None,
        translate_to_english: false,
        selected_language: default_selected_language(),
        overlay_position: default_overlay_position(),
        debug_mode: false,
        log_level: default_log_level(),
        custom_words: Vec::new(),
        model_unload_timeout: ModelUnloadTimeout::Never,
        word_correction_threshold: default_word_correction_threshold(),
        history_limit: default_history_limit(),
        recording_retention_period: default_recording_retention_period(),
        paste_method: PasteMethod::default(),
        clipboard_handling: ClipboardHandling::default(),
        auto_submit: default_auto_submit(),
        auto_submit_key: AutoSubmitKey::default(),
        mute_while_recording: false,
        append_trailing_space: false,
        app_language: default_app_language(),
        experimental_enabled: false,
        keyboard_implementation: KeyboardImplementation::default(),
        show_tray_icon: default_show_tray_icon(),
        paste_delay_ms: default_paste_delay_ms(),
        typing_tool: default_typing_tool(),
        external_script_path: None,
        commands_enabled: true,
        commands: Vec::new(),
        commands_llm_model_id: None,
        commands_llm_provider: CommandsLlmProvider::Parlia,
        anthropic_api_key: None,
        anthropic_model: default_anthropic_model(),
        openai_compat_base_url: None,
        openai_compat_api_key: None,
        openai_compat_model: None,
    }
}

fn default_anthropic_model() -> String {
    // Fast + cheap + good at French rewriting. Pinned to the dated model id
    // so upgrades are deliberate.
    "claude-haiku-4-5-20251001".to_string()
}

/// Move a plaintext key from the settings struct into the OS keychain.
/// Returns `true` when the settings object was mutated (so the caller knows
/// to rewrite the on-disk store). No-op on empty values. Logs success/
/// failure without ever printing the value itself.
fn migrate_plaintext_key_to_keychain(
    field: &mut Option<String>,
    secret: crate::secrets::SecretName,
    field_name: &str,
) -> bool {
    let Some(value) = field.as_ref() else {
        return false;
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        // Existing empty/whitespace string — clear the field but no keychain write.
        *field = None;
        return true;
    }
    match crate::secrets::set_secret(secret, trimmed) {
        Ok(()) => {
            log::info!(
                "Migrated {field_name} from settings store to OS keychain (length={})",
                trimmed.len()
            );
            *field = None;
            true
        }
        Err(e) => {
            // Don't blank the field if we couldn't store the key — losing
            // the user's paid API key during a failed migration would be
            // worse than leaving it in plaintext for another try.
            log::warn!("Keychain migration for {field_name} failed: {e}");
            false
        }
    }
}

pub fn load_or_create_app_settings(app: &AppHandle) -> AppSettings {
    // Initialize store
    let store = app
        .store(SETTINGS_STORE_PATH)
        .expect("Failed to initialize store");

    let settings = if let Some(settings_value) = store.get("settings") {
        // Parse the entire settings object
        match serde_json::from_value::<AppSettings>(settings_value) {
            Ok(mut settings) => {
                debug!("Found existing settings: {:?}", settings);
                let default_settings = get_default_settings();
                let mut updated = false;

                // Merge default bindings into existing settings
                for (key, value) in default_settings.bindings {
                    if !settings.bindings.contains_key(&key) {
                        debug!("Adding missing binding: {}", key);
                        settings.bindings.insert(key, value);
                        updated = true;
                    }
                }

                // One-shot migration from plaintext API keys → OS keychain.
                // Pre-v0.7.14 installs persisted Anthropic + OpenAI-compat
                // keys directly inside `settings_store.json`. Move them into
                // the keychain on first launch, then blank the in-memory
                // fields so the rewrite below drops them from disk.
                if migrate_plaintext_key_to_keychain(
                    &mut settings.anthropic_api_key,
                    crate::secrets::SecretName::Anthropic,
                    "anthropic_api_key",
                ) {
                    updated = true;
                }
                if migrate_plaintext_key_to_keychain(
                    &mut settings.openai_compat_api_key,
                    crate::secrets::SecretName::OpenAiCompat,
                    "openai_compat_api_key",
                ) {
                    updated = true;
                }

                if updated {
                    debug!("Settings updated with new bindings");
                    store.set("settings", serde_json::to_value(&settings).unwrap());
                }

                settings
            }
            Err(e) => {
                warn!("Failed to parse settings: {}", e);
                // Fall back to default settings if parsing fails
                let default_settings = get_default_settings();
                store.set("settings", serde_json::to_value(&default_settings).unwrap());
                default_settings
            }
        }
    } else {
        let default_settings = get_default_settings();
        store.set("settings", serde_json::to_value(&default_settings).unwrap());
        default_settings
    };

    settings
}

pub fn get_settings(app: &AppHandle) -> AppSettings {
    let store = app
        .store(SETTINGS_STORE_PATH)
        .expect("Failed to initialize store");

    if let Some(settings_value) = store.get("settings") {
        serde_json::from_value::<AppSettings>(settings_value).unwrap_or_else(|_| {
            let default_settings = get_default_settings();
            store.set("settings", serde_json::to_value(&default_settings).unwrap());
            default_settings
        })
    } else {
        let default_settings = get_default_settings();
        store.set("settings", serde_json::to_value(&default_settings).unwrap());
        default_settings
    }
}

pub fn write_settings(app: &AppHandle, settings: AppSettings) {
    let store = app
        .store(SETTINGS_STORE_PATH)
        .expect("Failed to initialize store");

    store.set("settings", serde_json::to_value(&settings).unwrap());
}

pub fn get_bindings(app: &AppHandle) -> HashMap<String, ShortcutBinding> {
    let settings = get_settings(app);

    settings.bindings
}

pub fn get_stored_binding(app: &AppHandle, id: &str) -> ShortcutBinding {
    let bindings = get_bindings(app);

    let binding = bindings.get(id).unwrap().clone();

    binding
}

pub fn get_history_limit(app: &AppHandle) -> usize {
    let settings = get_settings(app);
    settings.history_limit
}

pub fn get_recording_retention_period(app: &AppHandle) -> RecordingRetentionPeriod {
    let settings = get_settings(app);
    settings.recording_retention_period
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_disable_auto_submit() {
        let settings = get_default_settings();
        assert!(!settings.auto_submit);
        assert_eq!(settings.auto_submit_key, AutoSubmitKey::Enter);
    }

    #[test]
    fn debug_redacts_api_keys() {
        let mut settings = get_default_settings();
        settings.anthropic_api_key = Some("sk-ant-secret-do-not-leak".to_string());
        settings.openai_compat_api_key = Some("sk-or-secret-do-not-leak".to_string());

        let rendered = format!("{:?}", settings);
        assert!(
            !rendered.contains("sk-ant-secret-do-not-leak"),
            "Anthropic key leaked through Debug: {rendered}"
        );
        assert!(
            !rendered.contains("sk-or-secret-do-not-leak"),
            "OpenAI-compat key leaked through Debug: {rendered}"
        );
        assert!(rendered.contains("anthropic_api_key: \"<set>\""));
        assert!(rendered.contains("openai_compat_api_key: \"<set>\""));
    }

    #[test]
    fn debug_marks_empty_keys_as_unset() {
        let settings = get_default_settings();
        let rendered = format!("{:?}", settings);
        assert!(rendered.contains("anthropic_api_key: \"<unset>\""));
        assert!(rendered.contains("openai_compat_api_key: \"<unset>\""));
    }

    #[test]
    fn api_keys_are_never_serialized_to_json() {
        // Post-Keychain migration: the two key fields exist in memory only
        // to read legacy v0.7.13 settings_store.json files on first launch.
        // They MUST NOT round-trip back to disk, otherwise the migration is
        // a no-op the moment write_settings runs.
        let mut settings = get_default_settings();
        settings.anthropic_api_key = Some("sk-ant-should-not-persist".to_string());
        settings.openai_compat_api_key = Some("sk-or-should-not-persist".to_string());

        let json = serde_json::to_string(&settings).expect("serialize");

        assert!(
            !json.contains("anthropic_api_key"),
            "anthropic_api_key leaked to JSON: {json}"
        );
        assert!(
            !json.contains("openai_compat_api_key"),
            "openai_compat_api_key leaked to JSON: {json}"
        );
        assert!(
            !json.contains("sk-ant-should-not-persist"),
            "Anthropic key value leaked to JSON: {json}"
        );
        assert!(
            !json.contains("sk-or-should-not-persist"),
            "OpenAI-compat key value leaked to JSON: {json}"
        );
    }

    #[test]
    fn legacy_json_with_plaintext_keys_still_deserializes() {
        // Forward-compat the other way: a v0.7.13 settings_store.json that
        // has anthropic_api_key in plaintext must parse cleanly so the
        // migration step in `load_or_create_app_settings` can move it.
        let legacy = serde_json::json!({
            "bindings": {},
            "push_to_talk": false,
            "audio_feedback": true,
            "anthropic_api_key": "sk-ant-legacy",
            "openai_compat_api_key": "sk-or-legacy",
        });
        let parsed: AppSettings = serde_json::from_value(legacy).expect("parse");
        assert_eq!(parsed.anthropic_api_key.as_deref(), Some("sk-ant-legacy"));
        assert_eq!(parsed.openai_compat_api_key.as_deref(), Some("sk-or-legacy"));
    }
}
