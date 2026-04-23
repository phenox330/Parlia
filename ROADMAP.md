# Parlia — Roadmap

Last updated: 2026-04-23 (v0.7.10 shipped — adds Ollama / OpenAI-compatible provider)

## TL;DR

Speech-to-text Mac app with AI-powered custom commands (e.g. dictate
"Email …" → get a formatted email). Forked from Handy/Voixy, rebranded.
Users can run commands via Anthropic cloud, any OpenAI-compatible
endpoint (Ollama local, Groq, OpenRouter…), or a local llama.cpp model.
App is signed + notarized and live on GitHub Releases.

---

## Status snapshot

- **Core app** — working (transcription + pastes + history)
- **Voice commands** — Anthropic cloud + OpenAI-compatible provider
  (Ollama / Groq / OpenRouter / LM Studio / DeepSeek / vLLM)
- **Local LLM** — kept as opt-in, still crashes on macOS aarch64
  (llama.cpp vsnprintf bug, not fixable from Rust)
- **Distribution** — v0.7.10 signed + notarized on GitHub Releases
- **Brand** — unified around blue (#116cf5), Parlia identity in place
- **Legal** — MIT licence present in repo, not yet surfaced in the app
  (blocker for public distribution)

---

## What shipped on 2026-04-23 (v0.7.10)

### Custom LLM provider (OpenAI-compatible)
- New `CommandsLlmProvider::Custom` variant — one code path covers
  any `/chat/completions` endpoint: Ollama (local, free, private),
  LM Studio, Groq, OpenRouter, DeepSeek, vLLM…
- `src-tauri/src/cloud_llm.rs` — `generate_openai_compatible()`,
  60 s timeout, 2048 max-tokens cap; API key optional (blank skips
  the `Authorization` header so local Ollama / LM Studio work out
  of the box)
- `actions.rs` dispatches by provider, with friendly error surfaces
  when base URL or model are missing
- Settings UI: dropdown gains "Custom (Ollama, Groq, OpenRouter…)"
  with Base URL / Model / API Key inputs and a one-click Ollama
  preset (`http://localhost:11434/v1` + `qwen2.5:1.5b`)
- EN + FR translations
- Removes the "bring your own Anthropic key" friction — users with
  Ollama already installed are zero-cost, zero-friction

### Build + release
- Signed + notarized `.app` and `.dmg` for v0.7.10 (aarch64)
- Uploaded to GitHub Releases tag `v0.7.10`
- Landing page `parlia_lp` download CTA updated to v0.7.10 DMG

---

## What shipped on 2026-04-23 (v0.7.9)

### Voice commands feature
- Added master toggle "Voice Commands" in Settings > Commands (was
  invisible in UI before — `commands_enabled` had no switch)
- Surface LLM failures via `llm-error` Tauri event → toast in the UI
  (previously silent fallback to raw transcription)
- Loading toast during model activation (local path still applies)
- Improved logging: `set_active_llm_model invoked` entry log, emit
  `llm-model-loading` event so UI can show progress

### Cloud LLM (Anthropic)
- New `CommandsLlmProvider` enum (`Local` | `Anthropic`), defaults to
  Anthropic
- `src-tauri/src/cloud_llm.rs` — reqwest client against Anthropic
  Messages API (`claude-haiku-4-5-20251001`, 2048 max tokens, 60 s
  timeout), error surface with friendly messages
- `actions.rs` dispatches by provider
- Settings UI: provider dropdown + masked API-key input + "Get a key"
  link to `console.anthropic.com/settings/keys`

### Local LLM (kept as opt-in, still broken on this Mac)
- Bumped `llama-cpp-2` 0.1.139 → 0.1.145 (did not fix the crash)
- Silenced both `llama_log_set` and `ggml_log_set` callbacks (did not
  fix the crash — vsnprintf runs unconditionally before the callback)
- Swapped default catalog from Phi-3 Mini (2.3 GB) to Qwen2.5-1.5B
  (940 MB) — crashes in the same spot, proving it's not model-specific
- Added `xethub.hf.co` to the download host allowlist (bartowski
  repos redirect there; previously the download was silently rejected
  by the redirect policy)
- **Root cause identified**: `llama_log_internal_v` in
  `llama-impl.cpp:42` calls `vsnprintf` unconditionally on a format
  string whose `%s` arg is null during model load on macOS aarch64.
  Cannot be silenced from Rust. Would require forking llama-cpp-sys-2
  and patching C++ — deferred as low ROI.

### Brand rebrand
- Icons rebuilt from `parlia-mark.svg` via Sharp (rectangle + centered
  dot, elongated rect for better menu-bar visibility)
- Tray icon sets: light (`#ffffff`) for dark menu bars, dark (`#1d1d1f`)
  for light menu bars, three states (idle / recording / transcribing)
- Overlay + audio player: pink (`#FAA2CA`) → brand blue (`#116cf5`)
- Sidebar: ParliaMark at 24 px, Lucide icons at 16 px, all in a fixed
  24 px slot so labels align horizontally

### Dev-ops
- 3 commits pushed to `main`:
  - `c2d7ec9` feat: add Anthropic cloud provider for voice commands
  - `d240a63` feat: unify brand identity around blue (#116cf5)
  - `1ac935d` fix(sidebar): align icon sizes and row alignment
- Old `parlia.iconset/` scratch folder deleted

### Build + install workflow validated
- `bun run tauri build` → `.app` + `.dmg` (DMG bundling fails on
  `bundle_dmg.sh`, likely leftover mount — not blocking, `.app`
  is fine)
- Drag-and-drop install into `/Applications` works

### Signing + notarization (Developer ID)
- Apple Developer Program validated (Team ID `HWYPH89BH9`)
- Created `Developer ID Application` cert via G2 Sub-CA, installed
  intermediates (`DeveloperIDG2CA`, `AppleWWDRCAG3`, `AppleRootCA-G3`)
  locally — Xcode normally auto-installs these, but we bypass Xcode
- `tauri.conf.json` : `signingIdentity` set to
  `Developer ID Application: Anthony Gombert (HWYPH89BH9)`
- `createUpdaterArtifacts` temporarily set to `false` (no updater
  signing keys yet — revisit when we add auto-update)
- Notarization via `APPLE_ID` / `APPLE_PASSWORD` (app-specific) /
  `APPLE_TEAM_ID` env vars — `.app` notarized automatically by
  Tauri; `.dmg` notarized manually via `xcrun notarytool submit`
- Both `.app` and `.dmg` staple + validate, `spctl` reports
  `source=Notarized Developer ID`, opens on a fresh Mac install
  without any Gatekeeper warning

---

## Up next

### Can do now — legal compliance (required before public
distribution)

- **Surface MIT licence in the app**: Add an "Open source & credits"
  section in `AboutSettings` that shows:
  - Copyright © 2025 CJ Pais (original Handy author, MIT)
  - List of major dependencies (llama.cpp, Tauri, Whisper, reqwest,
    React, etc.) with their licences
  - Full MIT text in a modal or linked page
- **Bundle LICENSE file inside `.app`**: currently not copied into
  `Parlia.app/Contents/Resources/`. Configure `tauri.conf.json` to
  include it as a resource, or generate a `LICENSES.md` at build time
  via `cargo-about`.
- **Optional**: credits line in the landing-page footer

### Can do now — product work

- **Landing page** (priority — needed to surface the download link):
  - Headline + 15-30 s demo GIF showing "Email …" flow
  - Download CTA wired to the release DMG:
    ```html
    <a href="https://github.com/phenox330/Parlia/releases/download/v0.7.9/Parlia_0.7.9_aarch64.dmg"
       download>
      Download for Mac
    </a>
    ```
  - MIT footer + credit line (CJ Pais, original Handy author)
  - The `download` attribute + GitHub's `Content-Disposition: attachment`
    header means clicking triggers a direct DMG download — no detour
    through the GitHub release page
- **Thorough dogfooding**: use Parlia daily for 2-3 days, keep a list
  of friction points and bugs
- **Business-model decisions**:
  - Price model (free / freemium / one-time / subscription)
  - Provider UX: keep user-supplied Anthropic key (current) vs build
    a hosted proxy (so non-tech users don't need a key) — impacts
    architecture significantly
  - Positioning (vs Superwhisper / Whisper Flow / Raycast AI)

### Release — ready to ship

1. ~~Configure signing + notarization~~ ✅ done
2. ~~First signed + notarized build~~ ✅ done
3. ~~Upload `.dmg` to GitHub Releases (tag `v0.7.9`)~~ ✅ done
   — live at https://github.com/phenox330/Parlia/releases/tag/v0.7.9
4. **Build the landing page and wire the download CTA**
   to the release URL (see Landing page section above)
5. End-to-end test on a second Mac (ideally not yours) to validate
   Gatekeeper UX on a truly fresh machine
6. Revoke the exposed app-specific password and regenerate a new
   one before the next build (was shared in chat)

---

## Future (post-MVP)

- **Universal binary** (arm64 + Intel x86_64) — covers 100% of the
  Mac install base. One-line change in build config.
- **Auto-updater** via Tauri's updater plugin — users get updates
  automatically. Requires signing update bundles + a small manifest
  served from your own URL.
- **Hosted proxy backend** — remove the API-key friction for non-tech
  users. User signs in (magic link), gets a quota, can upgrade to a
  paid plan. ~2-3 days of backend work when you're ready to move
  from "API-key UX" to "account UX".
- **Windows build** (mois 3-6 after PMF) — needs Windows code-signing
  cert (~200-400 $/yr) and CI with a Windows runner. Tauri handles
  most of the cross-platform work.
- **Linux** — probably never for a productivity tool; ship source
  only if demand arises.
- **Revisit local LLM** if upstream llama.cpp patches the macOS
  aarch64 vsnprintf crash, or if a different runtime (MLC, Candle)
  becomes viable.

---

## Known issues carried forward

- `bundle_dmg.sh` fails during `tauri build` — `.app` is produced
  correctly, `.dmg` isn't. Likely a leftover mount in `/Volumes`.
  Low priority; only blocks if you want the DMG specifically.
  Workaround: `hdiutil detach /Volumes/Parlia` before rebuild.
- Local LLM "Activate" button still triggers a llama.cpp crash on
  macOS aarch64 even with all log callbacks silenced. Feature kept
  behind the provider dropdown so users can still opt in if they're
  on a machine/OS combo where it works.
