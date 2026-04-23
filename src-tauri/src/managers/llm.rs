use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::context::LlamaContext;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaChatMessage, LlamaModel, Special};
use llama_cpp_2::sampling::LlamaSampler;
use log::{debug, info, warn};
use once_cell::sync::{Lazy, OnceCell};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use specta::Type;
use std::collections::HashMap;
use std::fs;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Notify;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LlmModelInfo {
    pub id: String,
    pub name: String,
    /// i18n key for the human-readable description. Resolved on the frontend
    /// via `t(\`settings.commands.llmModel.descriptions.${description_key}\`)`.
    pub description_key: String,
    pub filename: String,
    pub url: Option<String>,
    pub size_mb: u64,
    pub is_downloaded: bool,
    pub is_downloading: bool,
    pub partial_size: u64,
    /// Optional pinned SHA-256 for integrity verification. When set, the
    /// downloaded file is rejected if its digest doesn't match.
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LlmDownloadProgress {
    pub model_id: String,
    pub downloaded: u64,
    pub total: u64,
    pub percentage: f64,
}

/// Outcome of a download request, distinguishing user cancellation from a
/// genuine completion at the IPC boundary. Real failures still surface as
/// `Result::Err`.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum DownloadResult {
    Completed,
    Cancelled,
}

/// Maximum number of tokens to generate in a single inference call.
const MAX_GENERATION_TOKENS: usize = 2048;
/// HTTP connect timeout for model downloads.
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// Max time to wait for the server to return response headers after the
/// connection is established. Guards against servers that drip headers
/// indefinitely (where CHUNK_READ_TIMEOUT cannot fire yet).
const RESPONSE_HEADERS_TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// Max time a single chunk read may hang before we bail out.
const CHUNK_READ_TIMEOUT: Duration = Duration::from_secs(60);
/// Tolerance above the advertised `size_mb` before we abort: numerator/denominator
/// gives exact integer math (11/10 = +10%), avoiding float→u64 cast rounding and
/// the silent saturation behaviour of as-cast.
const SIZE_CAP_NUMERATOR: u64 = 11;
const SIZE_CAP_DENOMINATOR: u64 = 10;

/// Cancellation primitive pairing an atomic flag (for poll-style checks
/// inside tight loops) with a `Notify` (to wake `wait()` without polling).
/// Both sides are needed: `load()` is free in hot paths, and `notified()`
/// removes the 50 ms sleep that used to live in `wait_for_cancel`.
#[derive(Clone)]
struct CancelHandle {
    flag: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

impl CancelHandle {
    fn new() -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
            notify: Arc::new(Notify::new()),
        }
    }

    fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }

    fn cancel(&self) {
        self.flag.store(true, Ordering::Relaxed);
        self.notify.notify_waiters();
    }

    /// Resolves as soon as `cancel()` has been (or is) called. The
    /// enable-before-check pattern closes the race where cancel fires
    /// between the flag check and the await.
    async fn wait(&self) {
        loop {
            let notified = self.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.is_cancelled() {
                return;
            }
            notified.await;
        }
    }
}

/// Per-model lifecycle. Transitions are always performed under the manager's
/// single `state` mutex, which gives us atomic check-and-set for download and
/// delete and eliminates TOCTOU races between them.
enum Lifecycle {
    Idle,
    Downloading { cancel: CancelHandle },
    Deleting,
}

struct ManagerState {
    models: HashMap<String, LlmModelInfo>,
    lifecycles: HashMap<String, Lifecycle>,
}

struct LoadedModel {
    id: Option<String>,
    model: Option<Arc<LlamaModel>>,
}

pub struct LlmModelManager {
    app_handle: AppHandle,
    models_dir: PathBuf,
    state: Mutex<ManagerState>,
    backend: OnceCell<LlamaBackend>,
    loaded: Mutex<LoadedModel>,
    /// Serialises concurrent `load_model` calls so a second caller can't kick
    /// off a redundant (and potentially VRAM-blowing) parallel load.
    load_lock: Mutex<()>,
}

/// Lock helper that recovers from poisoning instead of propagating a panic.
/// A poisoned mutex here means another thread panicked mid-critical-section;
/// the worst-case consequence is slightly stale data, not UB.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl LlmModelManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let models_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|e| anyhow!("Failed to get app data dir: {}", e))?
            .join("llm_models");

        if !models_dir.exists() {
            fs::create_dir_all(&models_dir)?;
        }

        let mut models = HashMap::new();

        models.insert(
            "qwen2.5-1.5b-instruct-q4".to_string(),
            LlmModelInfo {
                id: "qwen2.5-1.5b-instruct-q4".to_string(),
                name: "Qwen2.5 1.5B".to_string(),
                description_key: "qwen25_1_5b".to_string(),
                filename: "Qwen2.5-1.5B-Instruct-Q4_K_M.gguf".to_string(),
                url: Some("https://huggingface.co/bartowski/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/Qwen2.5-1.5B-Instruct-Q4_K_M.gguf".to_string()),
                size_mb: 940,
                is_downloaded: false,
                is_downloading: false,
                partial_size: 0,
                // Sourced from huggingface.co/api/models/bartowski/Qwen2.5-1.5B-Instruct-GGUF/tree/main
                // (LFS sha256 for Qwen2.5-1.5B-Instruct-Q4_K_M.gguf). Any future
                // catalog entry MUST also pin a hash — see `run_download`.
                sha256: Some(
                    "1adf0b11065d8ad2e8123ea110d1ec956dab4ab038eab665614adba04b6c3370".to_string(),
                ),
            },
        );

        // Validate filenames at catalog construction — defence in depth in
        // case the catalog ever becomes dynamic (remote JSON, user imports).
        for info in models.values() {
            validate_filename(&info.filename)?;
        }

        let mut lifecycles = HashMap::with_capacity(models.len());
        for id in models.keys() {
            lifecycles.insert(id.clone(), Lifecycle::Idle);
        }

        let manager = Self {
            app_handle: app_handle.clone(),
            models_dir,
            state: Mutex::new(ManagerState { models, lifecycles }),
            backend: OnceCell::new(),
            loaded: Mutex::new(LoadedModel {
                id: None,
                model: None,
            }),
            load_lock: Mutex::new(()),
        };

        manager.reconcile_disk_state();

        Ok(manager)
    }

    /// Sync `is_downloaded` / `partial_size` in `state.models` with what's
    /// actually on disk.
    ///
    /// The stat syscalls run outside the state lock — on slow/networked
    /// filesystems, `metadata()` can block for seconds, and holding the mutex
    /// across that would stall every progress emit and lifecycle check that
    /// also takes the same lock.
    fn reconcile_disk_state(&self) {
        let to_check: Vec<(String, String)> = {
            let state = lock(&self.state);
            state
                .models
                .values()
                .map(|m| (m.id.clone(), m.filename.clone()))
                .collect()
        };

        let facts: Vec<(String, bool, u64)> = to_check
            .into_iter()
            .map(|(id, filename)| {
                let model_path = self.models_dir.join(&filename);
                let partial_path = self.models_dir.join(format!("{}.partial", &filename));
                let exists = model_path.exists();
                let partial_size = partial_path.metadata().map(|md| md.len()).unwrap_or(0);
                (id, exists, partial_size)
            })
            .collect();

        let mut state = lock(&self.state);
        for (id, exists, partial_size) in facts {
            if let Some(m) = state.models.get_mut(&id) {
                m.is_downloaded = exists;
                m.partial_size = partial_size;
            }
        }
    }

    pub fn get_available_models(&self) -> Vec<LlmModelInfo> {
        // Re-sync with disk so out-of-band changes (manual rm of a .partial,
        // unexpected process kill mid-download) are reflected in the UI.
        self.reconcile_disk_state();
        let state = lock(&self.state);
        let mut list: Vec<LlmModelInfo> = state.models.values().cloned().collect();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        list
    }

    pub fn get_model_info(&self, model_id: &str) -> Option<LlmModelInfo> {
        let state = lock(&self.state);
        state.models.get(model_id).cloned()
    }

    pub fn get_model_path(&self, model_id: &str) -> Option<PathBuf> {
        let state = lock(&self.state);
        state
            .models
            .get(model_id)
            .map(|m| self.models_dir.join(&m.filename))
    }

    /// Atomically transition `Idle → Downloading`. Refuses if the model is
    /// already downloading, being deleted, or already on disk. Returns the
    /// info snapshot plus the cancel flag for the worker.
    fn begin_download(&self, model_id: &str) -> Result<BeginDownload> {
        // Snapshot the filename without the state lock so we can stat the
        // disk *before* locking — `exists()` can block for seconds on slow
        // or networked filesystems, and holding `state` across that stalls
        // every progress emit and lifecycle check. The narrow race with a
        // concurrent delete is already accepted elsewhere (reconcile_disk_state).
        let filename = {
            let state = lock(&self.state);
            state
                .models
                .get(model_id)
                .map(|m| m.filename.clone())
                .ok_or_else(|| anyhow!("LLM model not found: {}", model_id))?
        };
        let model_exists = self.models_dir.join(&filename).exists();

        let mut state = lock(&self.state);
        let info = state
            .models
            .get(model_id)
            .cloned()
            .ok_or_else(|| anyhow!("LLM model not found: {}", model_id))?;

        if model_exists {
            if let Some(m) = state.models.get_mut(model_id) {
                m.is_downloaded = true;
                m.is_downloading = false;
                m.partial_size = 0;
            }
            return Ok(BeginDownload::AlreadyDownloaded);
        }

        match state.lifecycles.get(model_id) {
            Some(Lifecycle::Downloading { .. }) => {
                warn!("Refusing to start LLM download for {}: already in progress", model_id);
                return Err(anyhow!(
                    "LLM model {} is already downloading",
                    model_id
                ));
            }
            Some(Lifecycle::Deleting) => {
                warn!("Refusing to start LLM download for {}: delete in progress", model_id);
                return Err(anyhow!(
                    "LLM model {} is being deleted",
                    model_id
                ));
            }
            _ => {}
        }

        let cancel = CancelHandle::new();
        state.lifecycles.insert(
            model_id.to_string(),
            Lifecycle::Downloading {
                cancel: cancel.clone(),
            },
        );
        if let Some(m) = state.models.get_mut(model_id) {
            m.is_downloading = true;
        }

        Ok(BeginDownload::Started { info, cancel })
    }

    /// Reset lifecycle to `Idle` and reconcile in-memory flags with disk.
    /// Called on every download exit (success, cancel, error).
    fn end_download(&self, model_id: &str, outcome: DownloadOutcome) {
        let mut state = lock(&self.state);
        state
            .lifecycles
            .insert(model_id.to_string(), Lifecycle::Idle);

        if let Some(m) = state.models.get_mut(model_id) {
            m.is_downloading = false;
            match outcome {
                DownloadOutcome::Completed => {
                    m.is_downloaded = true;
                    m.partial_size = 0;
                }
                DownloadOutcome::Cancelled | DownloadOutcome::Failed => {
                    // Partials are never resumable (full-stream hash), so leaving
                    // one on disk only misleads the UI. Remove it eagerly.
                    let partial = self.models_dir.join(format!("{}.partial", &m.filename));
                    let _ = fs::remove_file(&partial);
                    m.partial_size = 0;
                }
            }
        }
    }

    pub async fn download_model(&self, model_id: &str) -> Result<DownloadResult> {
        let begin = self.begin_download(model_id)?;
        let (info, cancel) = match begin {
            BeginDownload::AlreadyDownloaded => return Ok(DownloadResult::Completed),
            BeginDownload::Started { info, cancel } => (info, cancel),
        };

        // Cleanup is driven by the Drop impl — runs on every exit path,
        // including panic unwinds and `?` propagations.
        let mut guard = DownloadLifecycleGuard::new(self, model_id.to_string());

        let url = info
            .url
            .clone()
            .ok_or_else(|| anyhow!("No download URL for LLM model"))?;
        let model_path = self.models_dir.join(&info.filename);
        let partial_path = self.models_dir.join(format!("{}.partial", &info.filename));

        let outcome = self
            .run_download(&info, &url, &model_path, &partial_path, cancel)
            .await?;

        match outcome {
            RunOutcome::Completed => {
                guard.set_outcome(DownloadOutcome::Completed);
                Ok(DownloadResult::Completed)
            }
            RunOutcome::Cancelled => {
                info!("Propagating cancellation for LLM download {}", model_id);
                guard.set_outcome(DownloadOutcome::Cancelled);
                Ok(DownloadResult::Cancelled)
            }
        }
    }

    /// Orchestrates the download pipeline: preflight → open stream →
    /// stream-to-partial → verify & finalize. Each stage is a short helper
    /// so this function reads top-to-bottom as a sequence of named steps.
    async fn run_download(
        &self,
        model_info: &LlmModelInfo,
        url: &str,
        model_path: &Path,
        partial_path: &Path,
        cancel: CancelHandle,
    ) -> Result<RunOutcome> {
        let ctx = DownloadContext::prepare(model_info, url)?;

        // A pre-existing `.partial` cannot be resumed because the integrity
        // hash requires every byte to pass through the hasher.
        if partial_path.exists() {
            warn!(
                "Discarding existing partial for {} — full-stream hash required",
                model_info.id
            );
            let _ = fs::remove_file(partial_path);
        }
        info!(
            "Starting fresh LLM model download {} from {}",
            model_info.id,
            scrub_url(&ctx.parsed_url)
        );

        // Fast-out cancel check before we spend time on TCP/TLS handshake.
        if cancel.is_cancelled() {
            info!("LLM model download cancelled before stream opened: {}", model_info.id);
            return Ok(RunOutcome::Cancelled);
        }

        let client = build_download_client()?;
        let (total_size, stream) =
            open_download_stream(&client, &ctx, model_info, cancel.clone()).await?;

        // Second cancel check immediately after headers — avoids entering the
        // stream loop for a cancel that came in during the network round-trip.
        if cancel.is_cancelled() {
            info!("LLM model download cancelled after stream opened: {}", model_info.id);
            return Ok(RunOutcome::Cancelled);
        }

        let StreamOutcome {
            outcome,
            hasher,
            downloaded,
        } = self
            .stream_to_partial(stream, partial_path, model_info, &ctx, total_size, cancel)
            .await?;

        if let RunOutcome::Cancelled = outcome {
            return Ok(RunOutcome::Cancelled);
        }

        self.verify_and_finalize(
            partial_path,
            model_path,
            model_info,
            &ctx,
            downloaded,
            total_size,
            hasher,
        )
        .await?;

        info!("LLM model download complete: {}", model_info.id);
        Ok(RunOutcome::Completed)
    }

    /// Consume the HTTP byte stream into `partial_path`, hashing and emitting
    /// progress events, honouring cancel and the size cap.
    ///
    /// File I/O goes through `tokio::fs` so neither `write_all` nor `sync_all`
    /// stalls the runtime on large files. `hasher.update()` is CPU-bound but
    /// cheap per chunk (≪1 ms at typical 8–64 KB chunk sizes) so we keep it
    /// inline rather than round-tripping through `spawn_blocking`.
    async fn stream_to_partial(
        &self,
        mut stream: impl futures_util::Stream<Item = reqwest::Result<bytes::Bytes>> + Unpin,
        partial_path: &Path,
        model_info: &LlmModelInfo,
        ctx: &DownloadContext,
        total_size: u64,
        cancel: CancelHandle,
    ) -> Result<StreamOutcome> {
        use tokio::io::AsyncWriteExt;

        reject_symlink(partial_path)?;
        let mut file = tokio::fs::File::create(partial_path).await?;
        let mut hasher = Sha256::new();
        let mut downloaded: u64 = 0;

        self.emit_progress(&model_info.id, downloaded, total_size);
        let mut last_emit = std::time::Instant::now();

        loop {
            let chunk_fut = tokio::time::timeout(CHUNK_READ_TIMEOUT, stream.next());
            let next = tokio::select! {
                biased;
                _ = cancel.wait() => {
                    info!("LLM model download cancelled: {}", model_info.id);
                    return Ok(StreamOutcome {
                        outcome: RunOutcome::Cancelled,
                        hasher,
                        downloaded,
                    });
                }
                result = chunk_fut => result.map_err(|_| {
                    anyhow!(
                        "Download stalled (no data for {:?}) for LLM model {}",
                        CHUNK_READ_TIMEOUT,
                        model_info.id
                    )
                })?,
            };

            let chunk = match next {
                Some(Ok(c)) => c,
                Some(Err(e)) => return Err(anyhow!("Download stream error: {}", e)),
                None => break,
            };

            // Reject *before* writing so an overshoot never hits disk.
            let after = downloaded.saturating_add(chunk.len() as u64);
            if after > ctx.size_cap {
                drop(file);
                let _ = tokio::fs::remove_file(partial_path).await;
                return Err(anyhow!(
                    "LLM model {} exceeded size cap ({} > {})",
                    model_info.id,
                    after,
                    ctx.size_cap
                ));
            }

            file.write_all(&chunk).await?;
            hasher.update(&chunk);
            downloaded = after;

            if last_emit.elapsed() >= Duration::from_millis(100) {
                self.emit_progress(&model_info.id, downloaded, total_size);
                last_emit = std::time::Instant::now();
            }
        }

        // Durable write before verification — flush + fsync.
        file.flush()
            .await
            .with_context(|| format!("Failed to flush {}", partial_path.display()))?;
        file.sync_all()
            .await
            .with_context(|| format!("Failed to fsync {}", partial_path.display()))?;
        drop(file);

        Ok(StreamOutcome {
            outcome: RunOutcome::Completed,
            hasher,
            downloaded,
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn verify_and_finalize(
        &self,
        partial_path: &Path,
        model_path: &Path,
        model_info: &LlmModelInfo,
        ctx: &DownloadContext,
        downloaded: u64,
        total_size: u64,
        hasher: Sha256,
    ) -> Result<()> {
        if downloaded != total_size {
            let _ = tokio::fs::remove_file(partial_path).await;
            return Err(anyhow!(
                "LLM model {} download truncated: got {} of {} bytes",
                model_info.id,
                downloaded,
                total_size
            ));
        }

        let digest = hex(hasher.finalize().as_slice());
        if !digest.eq_ignore_ascii_case(&ctx.expected_sha) {
            let _ = tokio::fs::remove_file(partial_path).await;
            return Err(anyhow!(
                "Integrity check failed for {}: expected {}, got {}",
                model_info.id,
                ctx.expected_sha,
                digest
            ));
        }
        info!("Integrity verified for {}: {}", model_info.id, digest);

        reject_symlink(model_path)?;
        if let Err(e) = tokio::fs::rename(partial_path, model_path).await {
            // Rename can fail cross-device or if a dir sneaked into the target.
            // Drop the verified-good .partial so reconcile won't advertise it.
            let _ = tokio::fs::remove_file(partial_path).await;
            return Err(anyhow!(
                "Failed to finalize LLM model {}: rename {} -> {}: {}",
                model_info.id,
                partial_path.display(),
                model_path.display(),
                e
            ));
        }
        self.emit_progress(&model_info.id, total_size, total_size);
        Ok(())
    }

    fn emit_progress(&self, model_id: &str, downloaded: u64, total: u64) {
        let _ = self.app_handle.emit(
            "llm-download-progress",
            LlmDownloadProgress {
                model_id: model_id.to_string(),
                downloaded,
                total,
                percentage: progress_percentage(downloaded, total),
            },
        );
    }

    pub fn cancel_download(&self, model_id: &str) -> Result<()> {
        let state = lock(&self.state);
        if !state.models.contains_key(model_id) {
            return Err(anyhow!("LLM model not found: {}", model_id));
        }
        // Cancel is idempotent: if there is no in-flight download (already
        // finished, cancelled, or never started) we treat the call as a
        // no-op so a race between user-click and natural completion doesn't
        // surface as an error toast.
        if let Some(Lifecycle::Downloading { cancel }) = state.lifecycles.get(model_id) {
            cancel.cancel();
        }
        Ok(())
    }

    /// Atomically transition `Idle → Deleting`. Refuses if the model is
    /// downloading (signals cancel as best-effort) or already being deleted.
    fn begin_delete(&self, model_id: &str) -> Result<LlmModelInfo> {
        let mut state = lock(&self.state);
        let info = state
            .models
            .get(model_id)
            .cloned()
            .ok_or_else(|| anyhow!("LLM model not found: {}", model_id))?;

        match state.lifecycles.get(model_id) {
            Some(Lifecycle::Downloading { cancel }) => {
                // Signal cancel so the user can retry delete after the
                // download unwinds.
                cancel.cancel();
                warn!(
                    "Refusing to delete LLM model {} while download is in flight",
                    model_id
                );
                return Err(anyhow!(
                    "Cannot delete LLM model {} while it is downloading; cancel first",
                    model_id
                ));
            }
            Some(Lifecycle::Deleting) => {
                return Err(anyhow!(
                    "LLM model {} is already being deleted",
                    model_id
                ));
            }
            _ => {}
        }

        state
            .lifecycles
            .insert(model_id.to_string(), Lifecycle::Deleting);
        Ok(info)
    }

    pub fn delete_model(&self, model_id: &str) -> Result<()> {
        // Serialise against `load_model`: without this, a concurrent load could
        // mmap the file between our unload and the filesystem unlink, leaving
        // a live handle to a deleted (or about-to-be-deleted) file.
        let _load_guard = lock(&self.load_lock);

        let info = self.begin_delete(model_id)?;

        // Conditionally unload — re-checks the id under lock to avoid racing
        // an unrelated `load_model` call.
        self.unload_model_if(model_id);

        let model_path = self.models_dir.join(&info.filename);
        let partial_path = self.models_dir.join(format!("{}.partial", &info.filename));

        // Collect errors from both removals without `?` so one failure doesn't
        // skip the other and doesn't skip the state reconciliation.
        let mut errors: Vec<String> = Vec::new();
        if model_path.exists() {
            if let Err(e) = fs::remove_file(&model_path) {
                errors.push(format!("remove {}: {}", model_path.display(), e));
            }
        }
        if partial_path.exists() {
            if let Err(e) = fs::remove_file(&partial_path) {
                errors.push(format!("remove {}: {}", partial_path.display(), e));
            }
        }

        // Stat on-disk state before taking the state lock — avoids holding
        // the mutex across syscalls that may block on slow filesystems.
        let is_downloaded = model_path.exists();
        let partial_size = partial_path.metadata().map(|md| md.len()).unwrap_or(0);
        {
            let mut state = lock(&self.state);
            state
                .lifecycles
                .insert(model_id.to_string(), Lifecycle::Idle);
            if let Some(m) = state.models.get_mut(model_id) {
                m.is_downloaded = is_downloaded;
                m.partial_size = partial_size;
                m.is_downloading = false;
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(anyhow!("Failed to delete LLM model: {}", errors.join("; ")))
        }
    }

    fn backend(&self) -> Result<&LlamaBackend> {
        self.backend.get_or_try_init(|| {
            let mut backend = LlamaBackend::init()
                .map_err(|e| anyhow!("Failed to init llama backend: {}", e))?;
            // llama.cpp's default log callback writes to stderr via fprintf.
            // Its own internal LOG macros have at least one call path that
            // feeds vsnprintf a bad pointer under some build/arch combinations
            // (seen as EXC_BAD_ACCESS in __vfprintf → strlen on macOS aarch64).
            //
            // `void_logs()` only replaces the *llama* log callback. GGML (the
            // tensor library llama.cpp sits on top of) has its own separate
            // log sink, and the crash was in the ggml path during model load.
            // Install a no-op callback on BOTH so neither path ever calls
            // vsnprintf on the bad format arg.
            backend.void_logs();
            unsafe extern "C" fn noop_log(
                _level: llama_cpp_sys_2::ggml_log_level,
                _text: *const std::os::raw::c_char,
                _user_data: *mut std::os::raw::c_void,
            ) {
            }
            unsafe {
                llama_cpp_sys_2::ggml_log_set(Some(noop_log), std::ptr::null_mut());
            }
            Ok(backend)
        })
    }

    pub fn load_model(&self, model_id: &str) -> Result<()> {
        // Serialise concurrent loads so we never allocate two models in VRAM
        // for the same or different ids.
        let _load_guard = lock(&self.load_lock);

        // Fast-path if already loaded.
        {
            let loaded = lock(&self.loaded);
            if loaded.id.as_deref() == Some(model_id) {
                debug!("LLM model {} is already loaded", model_id);
                return Ok(());
            }
        }

        let model_path = self
            .get_model_path(model_id)
            .ok_or_else(|| anyhow!("LLM model not found: {}", model_id))?;

        if !model_path.exists() {
            return Err(anyhow!("LLM model file does not exist on disk"));
        }

        // Tell the UI we started loading; llama.cpp init + mmap of a 2+ GB
        // model can take 5–15 s and the button otherwise looks frozen.
        let _ = self
            .app_handle
            .emit("llm-model-loading", model_id.to_string());

        let backend = self.backend()?;

        // CPU-only by default. Enabling the Metal/CUDA offload path triggered
        // a crash in llama.cpp's own logging on at least macOS aarch64 (M3).
        // Users can opt in to GPU offload by setting `PARLIA_LLM_GPU_LAYERS`.
        let n_gpu_layers: u32 = std::env::var("PARLIA_LLM_GPU_LAYERS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        let filename_for_log = model_path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("<unknown>");
        info!(
            "Loading LLM model file {} (n_gpu_layers={})",
            filename_for_log, n_gpu_layers
        );

        let model_params = LlamaModelParams::default().with_n_gpu_layers(n_gpu_layers);
        let model = LlamaModel::load_from_file(backend, &model_path, &model_params)
            .map_err(|e| anyhow!("Failed to load LLM model: {}", e))?;

        {
            let mut loaded = lock(&self.loaded);
            loaded.model = Some(Arc::new(model));
            loaded.id = Some(model_id.to_string());
        }

        info!("LLM model {} loaded successfully", model_id);
        let _ = self
            .app_handle
            .emit("llm-model-loaded", model_id.to_string());
        Ok(())
    }

    pub fn unload_model(&self) {
        let mut loaded = lock(&self.loaded);
        loaded.model = None;
        loaded.id = None;
        debug!("LLM model unloaded");
    }

    /// Unload only if the currently-loaded model matches `expected_id`.
    /// Avoids wiping an unrelated model loaded concurrently.
    fn unload_model_if(&self, expected_id: &str) {
        let mut loaded = lock(&self.loaded);
        if loaded.id.as_deref() == Some(expected_id) {
            loaded.model = None;
            loaded.id = None;
            debug!("LLM model {} unloaded (id match)", expected_id);
        }
    }

    pub fn get_loaded_model_id(&self) -> Option<String> {
        lock(&self.loaded).id.clone()
    }

    /// Run inference. CPU-heavy and potentially multi-second — callers from
    /// async contexts should wrap this in `tokio::task::spawn_blocking`.
    /// The `Arc<LlamaModel>` is cloned out from under the lock so concurrent
    /// downloads, unloads, and status checks stay responsive during generation.
    pub fn generate(&self, system_prompt: &str, user_text: &str) -> Result<String> {
        let model = {
            let loaded = lock(&self.loaded);
            loaded
                .model
                .as_ref()
                .ok_or_else(|| anyhow!("No LLM model loaded"))?
                .clone()
        };
        let backend = self.backend()?;

        let user_text = sanitize_user_text(user_text);
        let prompt = build_prompt(&model, system_prompt, &user_text)?;
        debug!("LLM prompt length: {} chars", prompt.len());

        let tokens = model
            .str_to_token(&prompt, AddBos::Always)
            .map_err(|e| anyhow!("Failed to tokenize prompt: {}", e))?;
        debug!("LLM prompt tokens: {}", tokens.len());

        let mut ctx = new_context_for_prompt(&model, backend, tokens.len())?;
        decode_prompt(&mut ctx, &tokens)?;
        let output = sample_loop(&mut ctx, &model, tokens.len())?;

        debug!("LLM generation complete: {} chars", output.len());
        Ok(output)
    }
}

enum BeginDownload {
    Started {
        info: LlmModelInfo,
        cancel: CancelHandle,
    },
    AlreadyDownloaded,
}

/// Outcome communicated back up the stack by `run_download`. Genuine failures
/// surface as `Result::Err`, so success and cancellation are the only two
/// non-error variants — keeping this tight means `download_model`'s match is
/// exhaustive with no unreachable arm.
#[derive(Debug, Clone, Copy)]
enum RunOutcome {
    Completed,
    Cancelled,
}

/// Internal lifecycle state the RAII guard uses to decide whether to clean
/// up the `.partial` on Drop. `Failed` is the default and covers panic
/// unwinds and `?` propagations; callers flip it to `Completed`/`Cancelled`
/// before the successful return.
#[derive(Debug, Clone, Copy)]
enum DownloadOutcome {
    Completed,
    Cancelled,
    Failed,
}

/// RAII guard ensuring `end_download` runs on every exit path from
/// `download_model`, including panic unwinds. Defaults to `Failed`; callers
/// flip to `Completed` or `Cancelled` before the successful return.
struct DownloadLifecycleGuard<'a> {
    manager: &'a LlmModelManager,
    model_id: String,
    outcome: DownloadOutcome,
}

impl<'a> DownloadLifecycleGuard<'a> {
    fn new(manager: &'a LlmModelManager, model_id: String) -> Self {
        Self {
            manager,
            model_id,
            outcome: DownloadOutcome::Failed,
        }
    }

    fn set_outcome(&mut self, outcome: DownloadOutcome) {
        self.outcome = outcome;
    }
}

impl Drop for DownloadLifecycleGuard<'_> {
    fn drop(&mut self) {
        self.manager.end_download(&self.model_id, self.outcome);
    }
}

fn new_context_for_prompt<'a>(
    model: &'a LlamaModel,
    backend: &'a LlamaBackend,
    prompt_tokens: usize,
) -> Result<LlamaContext<'a>> {
    let ctx_size = prompt_tokens as u32 + MAX_GENERATION_TOKENS as u32;
    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(NonZeroU32::new(ctx_size))
        .with_n_batch(512);
    model
        .new_context(backend, ctx_params)
        .map_err(|e| anyhow!("Failed to create LLM context: {}", e))
}

fn decode_prompt(
    ctx: &mut LlamaContext,
    tokens: &[llama_cpp_2::token::LlamaToken],
) -> Result<()> {
    let mut batch = LlamaBatch::new(512, 1);
    let last_idx = tokens.len() as i32 - 1;
    for (i, &token) in tokens.iter().enumerate() {
        batch
            .add(token, i as i32, &[0], i as i32 == last_idx)
            .map_err(|e| anyhow!("Failed to add token to batch: {}", e))?;
    }
    ctx.decode(&mut batch)
        .map_err(|e| anyhow!("Failed to decode prompt batch: {}", e))?;
    Ok(())
}

fn sample_loop(
    ctx: &mut LlamaContext,
    model: &LlamaModel,
    prompt_len: usize,
) -> Result<String> {
    let mut sampler = LlamaSampler::chain_simple([
        LlamaSampler::temp(0.3),
        LlamaSampler::top_p(0.9, 1),
        LlamaSampler::min_p(0.05, 1),
        LlamaSampler::dist(42),
    ]);

    let mut batch = LlamaBatch::new(512, 1);
    let mut output = String::new();
    let eos_token = model.token_eos();
    let mut n_cur = prompt_len as i32;

    for _ in 0..MAX_GENERATION_TOKENS {
        let new_token = sampler.sample(ctx, -1);
        if new_token == eos_token {
            break;
        }
        match model.token_to_str(new_token, Special::Plaintext) {
            Ok(piece) => output.push_str(&piece),
            Err(e) => {
                warn!("Failed to decode token: {}", e);
                continue;
            }
        }
        batch.clear();
        batch
            .add(new_token, n_cur, &[0], true)
            .map_err(|e| anyhow!("Failed to add generated token: {}", e))?;
        ctx.decode(&mut batch)
            .map_err(|e| anyhow!("Failed to decode generated token: {}", e))?;
        n_cur += 1;
    }

    Ok(output.trim().to_string())
}

/// Strip chat-template sentinels (e.g. `<|end|>`, `<|im_start|>`) and
/// ASCII control characters from untrusted voice-command input, and clamp
/// the result to a byte budget. Without this, transcribed text could inject
/// role turns into the prompt or prematurely end generation.
fn sanitize_user_text(input: &str) -> String {
    const MAX_USER_TEXT_BYTES: usize = 4096;
    static TEMPLATE_SENTINEL: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"<\|[^|>\n]{1,64}\|>").expect("static regex"));

    let stripped = TEMPLATE_SENTINEL.replace_all(input, "");
    let cleaned: String = stripped
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect();

    if cleaned.len() <= MAX_USER_TEXT_BYTES {
        return cleaned;
    }
    let mut cut = MAX_USER_TEXT_BYTES;
    while cut > 0 && !cleaned.is_char_boundary(cut) {
        cut -= 1;
    }
    cleaned[..cut].to_string()
}

fn build_prompt(model: &LlamaModel, system_prompt: &str, user_text: &str) -> Result<String> {
    let template = model
        .chat_template(None)
        .map_err(|e| anyhow!("Failed to get chat template: {}", e))?;

    let messages = vec![
        LlamaChatMessage::new("system".to_string(), system_prompt.to_string())
            .map_err(|e| anyhow!("Failed to create system message: {}", e))?,
        LlamaChatMessage::new("user".to_string(), user_text.to_string())
            .map_err(|e| anyhow!("Failed to create user message: {}", e))?,
    ];

    model
        .apply_chat_template(&template, &messages, true)
        .map_err(|e| anyhow!("Failed to apply chat template: {}", e))
}

fn progress_percentage(downloaded: u64, total: u64) -> f64 {
    if total > 0 {
        (downloaded as f64 / total as f64) * 100.0
    } else {
        0.0
    }
}

/// Format a URL for logs and user-visible errors without its query string.
/// HF LFS redirects embed short-lived signed query params (X-Amz-Signature,
/// Expires, etc.) which must not end up in bug reports or error chains.
fn scrub_url(url: &reqwest::Url) -> String {
    format!("{}://{}{}", url.scheme(), url.host_str().unwrap_or("?"), url.path())
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
    }
    s
}

/// Per-download invariants computed once up front (SHA pin, size cap,
/// validated URL). Threaded through the streaming and finalization helpers.
struct DownloadContext {
    expected_sha: String,
    size_cap: u64,
    parsed_url: reqwest::Url,
}

impl DownloadContext {
    fn prepare(model_info: &LlmModelInfo, url: &str) -> Result<Self> {
        let expected_sha = model_info
            .sha256
            .as_deref()
            .ok_or_else(|| {
                anyhow!(
                    "LLM model {} has no pinned SHA-256 — refusing to download unverified weights",
                    model_info.id
                )
            })?
            .to_ascii_lowercase();

        if model_info.size_mb == 0 {
            return Err(anyhow!(
                "LLM model {} has an invalid zero size_mb",
                model_info.id
            ));
        }
        // Size cap in integer space: reject catalog values that would overflow
        // u64 before we can compare against the absolute ceiling.
        let per_model_cap = model_info
            .size_mb
            .checked_mul(1024 * 1024)
            .and_then(|b| b.checked_mul(SIZE_CAP_NUMERATOR))
            .map(|b| b / SIZE_CAP_DENOMINATOR)
            .ok_or_else(|| {
                anyhow!(
                    "LLM model {} size_mb ({}) overflows size-cap computation",
                    model_info.id,
                    model_info.size_mb
                )
            })?;
        let size_cap = per_model_cap.min(ABSOLUTE_SIZE_CEILING_BYTES);

        let parsed_url = reqwest::Url::parse(url)
            .map_err(|e| anyhow!("Invalid LLM download URL {}: {}", url, e))?;
        validate_download_url(&parsed_url)?;

        Ok(Self {
            expected_sha,
            size_cap,
            parsed_url,
        })
    }
}

/// Return value of the streaming stage. `hasher` is consumed by the
/// finalization stage which finalises it and compares to `expected_sha`.
struct StreamOutcome {
    outcome: RunOutcome,
    hasher: Sha256,
    downloaded: u64,
}

/// Build the HTTP client used for model downloads. Redirect policy re-
/// validates each hop against the host allowlist so a redirect can't
/// bypass the check the initial URL passed.
fn build_download_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(HTTP_CONNECT_TIMEOUT)
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 8 {
                return attempt.error("too many redirects");
            }
            match validate_download_url(attempt.url()) {
                Ok(()) => attempt.follow(),
                Err(e) => attempt.error(e),
            }
        }))
        .build()
        .context("Failed to build HTTP client")
}

/// Send the GET, validate the status and Content-Length, and hand back the
/// advertised size plus the chunked byte stream.
async fn open_download_stream(
    client: &reqwest::Client,
    ctx: &DownloadContext,
    model_info: &LlmModelInfo,
    cancel: CancelHandle,
) -> Result<(u64, impl futures_util::Stream<Item = reqwest::Result<bytes::Bytes>> + Unpin)> {
    // Race the HTTP send against cancel so the user doesn't have to wait for
    // the headers timeout when they click Cancel mid-handshake.
    let send_fut = tokio::time::timeout(
        RESPONSE_HEADERS_TIMEOUT,
        client.get(ctx.parsed_url.clone()).send(),
    );
    let response = tokio::select! {
        r = send_fut => r
            .map_err(|_| anyhow!(
                "Server did not return headers within {:?} for {}",
                RESPONSE_HEADERS_TIMEOUT,
                scrub_url(&ctx.parsed_url)
            ))?
            .with_context(|| format!("HTTP request failed for {}", scrub_url(&ctx.parsed_url)))?,
        _ = cancel.wait() => {
            return Err(anyhow!("LLM model {} download cancelled", model_info.id));
        }
    };

    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to download LLM model: HTTP {}",
            response.status()
        ));
    }

    // Require Content-Length so the pre-stream size check is meaningful.
    // Without it the server could push arbitrary bytes until the per-chunk
    // guard fires.
    let total_size = response.content_length().ok_or_else(|| {
        anyhow!(
            "Server did not provide Content-Length for {}",
            model_info.id
        )
    })?;

    if total_size == 0 || total_size > ctx.size_cap {
        return Err(anyhow!(
            "LLM model {} reports size {} bytes, outside allowed range [1, {}]",
            model_info.id,
            total_size,
            ctx.size_cap
        ));
    }

    Ok((total_size, response.bytes_stream()))
}

/// Allowlist of download hosts. Keep it narrow — any compromise of these
/// CDNs is already a hash-verification problem, but we at least stop the
/// app from contacting arbitrary hosts (SSRF / pivot) via a future user-
/// editable catalog.
const DOWNLOAD_HOST_ALLOWLIST: &[&str] = &[
    "huggingface.co",
    "cdn-lfs.huggingface.co",
    "cdn-lfs.hf.co",
    // HF's newer Xet-backed storage — bartowski/* and a growing number of
    // community repos redirect LFS downloads here instead of cdn-lfs.
    "xethub.hf.co",
];

/// Hard ceiling applied on top of the per-model `size_mb` cap. Protects against
/// a malformed catalog entry with size_mb = 0 or an absurdly small value that
/// would let a chunked response write gigabytes before any check fires.
const ABSOLUTE_SIZE_CEILING_BYTES: u64 = 20 * 1024 * 1024 * 1024; // 20 GB

/// Parse and allowlist a download URL. Enforces https + known host.
/// Called for the initial URL AND for every redirect hop.
fn validate_download_url(url: &reqwest::Url) -> Result<()> {
    if url.scheme() != "https" {
        return Err(anyhow!(
            "Download URL must use https, got {:?}",
            url.scheme()
        ));
    }
    // Reject embedded credentials: a catalog entry like
    // `https://user:token@huggingface.co/...` would forward the Authorization
    // header through every redirect and hand creds to the redirect target.
    if !url.username().is_empty() || url.password().is_some() {
        return Err(anyhow!("Download URL must not contain userinfo"));
    }
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("Download URL has no host"))?
        .to_ascii_lowercase();
    let ok = DOWNLOAD_HOST_ALLOWLIST
        .iter()
        .any(|h| host == *h || host.ends_with(&format!(".{}", h)));
    if !ok {
        return Err(anyhow!(
            "Download host {:?} is not in the allowlist",
            host
        ));
    }
    Ok(())
}

/// Reject a path target that has been replaced by a symlink. Opening or
/// renaming follows symlinks by default, which would let an attacker with
/// write access to the models directory redirect our writes to arbitrary
/// files (e.g. ~/.ssh/authorized_keys). `symlink_metadata` inspects the
/// link itself without following — if the path doesn't exist at all, that's
/// fine (the create call will make a regular file).
fn reject_symlink(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(md) if md.file_type().is_symlink() => Err(anyhow!(
            "Refusing to write through a symlink at {}",
            path.display()
        )),
        _ => Ok(()),
    }
}

/// Reject filenames that could escape `models_dir` (defence in depth).
fn validate_filename(name: &str) -> Result<()> {
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
        || Path::new(name).is_absolute()
    {
        return Err(anyhow!("Unsafe LLM model filename: {:?}", name));
    }
    Ok(())
}
