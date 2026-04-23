# Parlia — Roadmap

Last updated: 2026-04-23

## TL;DR

Speech-to-text Mac app with AI-powered custom commands (e.g. dictate
"Email …" → get a formatted email). Forked from Handy/Voixy, rebranded.
Core flow works end-to-end via Anthropic cloud inference. Shipping the
public version blocked on Apple Developer validation (signing +
notarization).

---

## Status snapshot

- **Core app** — working (transcription + pastes + history)
- **Voice commands** — working via Anthropic cloud
- **Local LLM** — kept as opt-in, still crashes on macOS aarch64
  (llama.cpp vsnprintf bug, not fixable from Rust)
- **Distribution** — blocked on Apple Developer Program validation
- **Brand** — unified around blue (#116cf5), Parlia identity in place
- **Legal** — MIT licence present in repo, not yet surfaced in the app
  (blocker for public distribution)

---

## What shipped on 2026-04-23

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

---

## Up next

### Blocked on external dependencies

- **Apple Developer Program validation** (enrolled, awaiting
  confirmation — usually 24-48 h, sometimes 3-5 days)

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

### Can do now — product work while waiting

- **Landing page**: headline + 15-30 s demo GIF showing "Email …"
  flow + Download button (provisionally disabled) + MIT footer
- **Thorough dogfooding**: use Parlia daily for 2-3 days, keep a list
  of friction points and bugs
- **Business-model decisions**:
  - Price model (free / freemium / one-time / subscription)
  - Provider UX: keep user-supplied Anthropic key (current) vs build
    a hosted proxy (so non-tech users don't need a key) — impacts
    architecture significantly
  - Positioning (vs Superwhisper / Whisper Flow / Raycast AI)

### Unblocks once Apple Developer validated

1. Configure signing in `tauri.conf.json`:
   - Replace `signingIdentity: "-"` (adhoc) with real Developer ID
   - Wire up notarization env vars
     (`APPLE_ID`/`APPLE_PASSWORD`/`APPLE_TEAM_ID` or API-key trio)
2. First signed + notarized build → staple the ticket
3. Upload `.dmg` to GitHub Releases (tag `v0.7.9` or bump to `v0.8.0`)
4. Wire the Download button on the landing page to the Release URL
5. End-to-end test on a second Mac (ideally not yours) to validate
   Gatekeeper UX

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
