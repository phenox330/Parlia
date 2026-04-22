use anyhow::Result;
use futures_util::StreamExt;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaChatMessage, LlamaModel, Special};
use llama_cpp_2::sampling::LlamaSampler;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::fs;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LlmModelInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub filename: String,
    pub url: Option<String>,
    pub size_mb: u64,
    pub is_downloaded: bool,
    pub is_downloading: bool,
    pub partial_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LlmDownloadProgress {
    pub model_id: String,
    pub downloaded: u64,
    pub total: u64,
    pub percentage: f64,
}

/// Maximum number of tokens to generate in a single inference call.
const MAX_GENERATION_TOKENS: usize = 2048;

pub struct LlmModelManager {
    app_handle: AppHandle,
    models_dir: PathBuf,
    available_models: Mutex<HashMap<String, LlmModelInfo>>,
    cancel_flags: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    backend: Mutex<Option<LlamaBackend>>,
    loaded_model: Mutex<Option<LoadedLlmModel>>,
    loaded_model_id: Mutex<Option<String>>,
}

struct LoadedLlmModel {
    model: LlamaModel,
}

// Safety: LlamaModel/LlamaBackend are thread-safe behind Mutex
unsafe impl Send for LoadedLlmModel {}
unsafe impl Sync for LoadedLlmModel {}

impl LlmModelManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let models_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|e| anyhow::anyhow!("Failed to get app data dir: {}", e))?
            .join("llm_models");

        if !models_dir.exists() {
            fs::create_dir_all(&models_dir)?;
        }

        let mut available_models = HashMap::new();

        available_models.insert(
            "phi-3-mini-4k-q4".to_string(),
            LlmModelInfo {
                id: "phi-3-mini-4k-q4".to_string(),
                name: "Phi-3 Mini".to_string(),
                description: "Modèle compact et rapide (3.8B paramètres). Bon pour les transformations de texte.".to_string(),
                filename: "Phi-3-mini-4k-instruct-q4.gguf".to_string(),
                url: Some("https://huggingface.co/microsoft/Phi-3-mini-4k-instruct-gguf/resolve/main/Phi-3-mini-4k-instruct-q4.gguf".to_string()),
                size_mb: 2300,
                is_downloaded: false,
                is_downloading: false,
                partial_size: 0,
            },
        );

        let manager = Self {
            app_handle: app_handle.clone(),
            models_dir: models_dir.clone(),
            available_models: Mutex::new(available_models),
            cancel_flags: Arc::new(Mutex::new(HashMap::new())),
            backend: Mutex::new(None),
            loaded_model: Mutex::new(None),
            loaded_model_id: Mutex::new(None),
        };

        manager.update_download_status()?;

        Ok(manager)
    }

    fn update_download_status(&self) -> Result<()> {
        let mut models = self.available_models.lock().unwrap();
        for model in models.values_mut() {
            let model_path = self.models_dir.join(&model.filename);
            let partial_path = self.models_dir.join(format!("{}.partial", &model.filename));

            model.is_downloaded = model_path.exists();

            if partial_path.exists() {
                if let Ok(metadata) = partial_path.metadata() {
                    model.partial_size = metadata.len();
                }
            } else {
                model.partial_size = 0;
            }
        }
        Ok(())
    }

    pub fn get_available_models(&self) -> Vec<LlmModelInfo> {
        let models = self.available_models.lock().unwrap();
        let mut list: Vec<LlmModelInfo> = models.values().cloned().collect();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        list
    }

    pub fn get_model_info(&self, model_id: &str) -> Option<LlmModelInfo> {
        let models = self.available_models.lock().unwrap();
        models.get(model_id).cloned()
    }

    pub fn get_model_path(&self, model_id: &str) -> Option<PathBuf> {
        let models = self.available_models.lock().unwrap();
        models
            .get(model_id)
            .map(|m| self.models_dir.join(&m.filename))
    }

    pub async fn download_model(&self, model_id: &str) -> Result<()> {
        let model_info = {
            let models = self.available_models.lock().unwrap();
            models.get(model_id).cloned()
        };

        let model_info =
            model_info.ok_or_else(|| anyhow::anyhow!("LLM model not found: {}", model_id))?;

        let url = model_info
            .url
            .ok_or_else(|| anyhow::anyhow!("No download URL for LLM model"))?;
        let model_path = self.models_dir.join(&model_info.filename);
        let partial_path = self
            .models_dir
            .join(format!("{}.partial", &model_info.filename));

        if model_path.exists() {
            if partial_path.exists() {
                let _ = fs::remove_file(&partial_path);
            }
            self.update_download_status()?;
            return Ok(());
        }

        let mut resume_from = if partial_path.exists() {
            let size = partial_path.metadata()?.len();
            info!(
                "Resuming LLM model download {} from byte {}",
                model_id, size
            );
            size
        } else {
            info!("Starting fresh LLM model download {} from {}", model_id, url);
            0
        };

        // Mark as downloading
        {
            let mut models = self.available_models.lock().unwrap();
            if let Some(model) = models.get_mut(model_id) {
                model.is_downloading = true;
            }
        }

        let cancel_flag = Arc::new(AtomicBool::new(false));
        {
            let mut flags = self.cancel_flags.lock().unwrap();
            flags.insert(model_id.to_string(), cancel_flag.clone());
        }

        let client = reqwest::Client::new();
        let mut request = client.get(&url);

        if resume_from > 0 {
            request = request.header("Range", format!("bytes={}-", resume_from));
        }

        let mut response = request.send().await?;

        if resume_from > 0 && response.status() == reqwest::StatusCode::OK {
            warn!(
                "Server doesn't support range requests for LLM model {}, restarting",
                model_id
            );
            drop(response);
            let _ = fs::remove_file(&partial_path);
            resume_from = 0;
            response = client.get(&url).send().await?;
        }

        if !response.status().is_success()
            && response.status() != reqwest::StatusCode::PARTIAL_CONTENT
        {
            let mut models = self.available_models.lock().unwrap();
            if let Some(model) = models.get_mut(model_id) {
                model.is_downloading = false;
            }
            return Err(anyhow::anyhow!(
                "Failed to download LLM model: HTTP {}",
                response.status()
            ));
        }

        let total_size = if resume_from > 0 {
            resume_from + response.content_length().unwrap_or(0)
        } else {
            response.content_length().unwrap_or(0)
        };

        let mut downloaded = resume_from;
        let mut stream = response.bytes_stream();

        let mut file = if resume_from > 0 {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&partial_path)?
        } else {
            std::fs::File::create(&partial_path)?
        };

        let _ = self.app_handle.emit(
            "llm-download-progress",
            LlmDownloadProgress {
                model_id: model_id.to_string(),
                downloaded,
                total: total_size,
                percentage: if total_size > 0 {
                    (downloaded as f64 / total_size as f64) * 100.0
                } else {
                    0.0
                },
            },
        );

        let mut last_emit = std::time::Instant::now();

        while let Some(chunk) = stream.next().await {
            if cancel_flag.load(Ordering::Relaxed) {
                info!("LLM model download cancelled: {}", model_id);
                let mut models = self.available_models.lock().unwrap();
                if let Some(model) = models.get_mut(model_id) {
                    model.is_downloading = false;
                }
                return Ok(());
            }

            let chunk = chunk?;
            use std::io::Write;
            file.write_all(&chunk)?;
            downloaded += chunk.len() as u64;

            if last_emit.elapsed() >= std::time::Duration::from_millis(100) {
                let _ = self.app_handle.emit(
                    "llm-download-progress",
                    LlmDownloadProgress {
                        model_id: model_id.to_string(),
                        downloaded,
                        total: total_size,
                        percentage: if total_size > 0 {
                            (downloaded as f64 / total_size as f64) * 100.0
                        } else {
                            0.0
                        },
                    },
                );
                last_emit = std::time::Instant::now();
            }
        }

        // Rename partial to final
        fs::rename(&partial_path, &model_path)?;

        // Update status
        {
            let mut models = self.available_models.lock().unwrap();
            if let Some(model) = models.get_mut(model_id) {
                model.is_downloaded = true;
                model.is_downloading = false;
                model.partial_size = 0;
            }
        }

        let _ = self.app_handle.emit(
            "llm-download-progress",
            LlmDownloadProgress {
                model_id: model_id.to_string(),
                downloaded: total_size,
                total: total_size,
                percentage: 100.0,
            },
        );

        info!("LLM model download complete: {}", model_id);
        Ok(())
    }

    pub fn cancel_download(&self, model_id: &str) -> Result<()> {
        let flags = self.cancel_flags.lock().unwrap();
        if let Some(flag) = flags.get(model_id) {
            flag.store(true, Ordering::Relaxed);
        }
        Ok(())
    }

    pub fn delete_model(&self, model_id: &str) -> Result<()> {
        // Unload if this model is currently loaded
        {
            let loaded_id = self.loaded_model_id.lock().unwrap();
            if loaded_id.as_deref() == Some(model_id) {
                drop(loaded_id);
                self.unload_model();
            }
        }

        let model_info = {
            let models = self.available_models.lock().unwrap();
            models.get(model_id).cloned()
        };

        if let Some(info) = model_info {
            let model_path = self.models_dir.join(&info.filename);
            let partial_path = self.models_dir.join(format!("{}.partial", &info.filename));

            if model_path.exists() {
                fs::remove_file(&model_path)?;
            }
            if partial_path.exists() {
                fs::remove_file(&partial_path)?;
            }

            let mut models = self.available_models.lock().unwrap();
            if let Some(model) = models.get_mut(model_id) {
                model.is_downloaded = false;
                model.partial_size = 0;
            }
        }

        Ok(())
    }

    fn ensure_backend(&self) -> Result<()> {
        let mut backend = self.backend.lock().unwrap();
        if backend.is_none() {
            let b = LlamaBackend::init().map_err(|e| anyhow::anyhow!("Failed to init llama backend: {}", e))?;
            *backend = Some(b);
        }
        Ok(())
    }

    pub fn load_model(&self, model_id: &str) -> Result<()> {
        // Check if already loaded
        {
            let loaded_id = self.loaded_model_id.lock().unwrap();
            if loaded_id.as_deref() == Some(model_id) {
                debug!("LLM model {} is already loaded", model_id);
                return Ok(());
            }
        }

        let model_path = self
            .get_model_path(model_id)
            .ok_or_else(|| anyhow::anyhow!("LLM model not found: {}", model_id))?;

        if !model_path.exists() {
            return Err(anyhow::anyhow!(
                "LLM model file does not exist: {:?}",
                model_path
            ));
        }

        self.ensure_backend()?;

        info!("Loading LLM model from {:?}", model_path);

        let backend = self.backend.lock().unwrap();
        let backend_ref = backend.as_ref().unwrap();

        let model_params = LlamaModelParams::default().with_n_gpu_layers(99);

        let model = LlamaModel::load_from_file(backend_ref, &model_path, &model_params)
            .map_err(|e| anyhow::anyhow!("Failed to load LLM model: {}", e))?;

        {
            let mut loaded = self.loaded_model.lock().unwrap();
            *loaded = Some(LoadedLlmModel { model });
        }
        {
            let mut loaded_id = self.loaded_model_id.lock().unwrap();
            *loaded_id = Some(model_id.to_string());
        }

        info!("LLM model {} loaded successfully", model_id);
        Ok(())
    }

    pub fn unload_model(&self) {
        let mut loaded = self.loaded_model.lock().unwrap();
        let mut loaded_id = self.loaded_model_id.lock().unwrap();
        *loaded = None;
        *loaded_id = None;
        debug!("LLM model unloaded");
    }

    pub fn get_loaded_model_id(&self) -> Option<String> {
        let loaded_id = self.loaded_model_id.lock().unwrap();
        loaded_id.clone()
    }

    pub fn generate(&self, system_prompt: &str, user_text: &str) -> Result<String> {
        let loaded = self.loaded_model.lock().unwrap();
        let loaded_model = loaded
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No LLM model loaded"))?;

        let backend = self.backend.lock().unwrap();
        let backend_ref = backend
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Backend not initialized"))?;

        let model = &loaded_model.model;

        // Get the chat template from the model
        let template = model
            .chat_template(None)
            .map_err(|e| anyhow::anyhow!("Failed to get chat template: {}", e))?;

        // Build chat messages
        let messages = vec![
            LlamaChatMessage::new("system".to_string(), system_prompt.to_string())
                .map_err(|e| anyhow::anyhow!("Failed to create system message: {}", e))?,
            LlamaChatMessage::new("user".to_string(), user_text.to_string())
                .map_err(|e| anyhow::anyhow!("Failed to create user message: {}", e))?,
        ];

        // Apply chat template to get the prompt string
        let prompt = model
            .apply_chat_template(&template, &messages, true)
            .map_err(|e| anyhow::anyhow!("Failed to apply chat template: {}", e))?;

        debug!("LLM prompt length: {} chars", prompt.len());

        // Tokenize the prompt
        let tokens = model
            .str_to_token(&prompt, AddBos::Always)
            .map_err(|e| anyhow::anyhow!("Failed to tokenize prompt: {}", e))?;

        debug!("LLM prompt tokens: {}", tokens.len());

        // Create context with enough space for prompt + generation
        let ctx_size = tokens.len() as u32 + MAX_GENERATION_TOKENS as u32;
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(ctx_size))
            .with_n_batch(512);

        let mut ctx = model
            .new_context(backend_ref, ctx_params)
            .map_err(|e| anyhow::anyhow!("Failed to create LLM context: {}", e))?;

        // Create a batch and add all prompt tokens
        let mut batch = LlamaBatch::new(512, 1);
        let last_idx = tokens.len() as i32 - 1;
        for (i, &token) in tokens.iter().enumerate() {
            batch
                .add(token, i as i32, &[0], i as i32 == last_idx)
                .map_err(|e| anyhow::anyhow!("Failed to add token to batch: {}", e))?;
        }

        // Decode the prompt batch
        ctx.decode(&mut batch)
            .map_err(|e| anyhow::anyhow!("Failed to decode prompt batch: {}", e))?;

        // Set up sampler: temp + top_p + dist
        let mut sampler = LlamaSampler::chain_simple([
            LlamaSampler::temp(0.3),
            LlamaSampler::top_p(0.9, 1),
            LlamaSampler::min_p(0.05, 1),
            LlamaSampler::dist(42),
        ]);

        // Generate tokens
        let mut output = String::new();
        let eos_token = model.token_eos();
        let mut n_cur = tokens.len() as i32;

        for _ in 0..MAX_GENERATION_TOKENS {
            let new_token = sampler.sample(&ctx, -1);

            // Check for end of sequence
            if new_token == eos_token {
                break;
            }

            // Decode token to string
            match model.token_to_str(new_token, Special::Plaintext) {
                Ok(piece) => output.push_str(&piece),
                Err(e) => {
                    warn!("Failed to decode token: {}", e);
                    continue;
                }
            }

            // Prepare next batch with just this token
            batch.clear();
            batch
                .add(new_token, n_cur, &[0], true)
                .map_err(|e| anyhow::anyhow!("Failed to add generated token: {}", e))?;

            ctx.decode(&mut batch)
                .map_err(|e| anyhow::anyhow!("Failed to decode generated token: {}", e))?;

            n_cur += 1;
        }

        let output = output.trim().to_string();
        debug!("LLM generation complete: {} chars", output.len());
        Ok(output)
    }
}
