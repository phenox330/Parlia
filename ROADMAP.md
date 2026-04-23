# Parlia — Roadmap

Last updated: 2026-04-23 (v0.7.12 shipped — Phase 0 hygiene: token rotation + per-IP rate limit)

## TL;DR

Speech-to-text Mac app with AI-powered custom commands (e.g. dictate
"Email …" → get a formatted email). Forked from Handy/Voixy, rebranded.
Default provider is now Parlia Cloud — a hosted Vercel Edge proxy that
relays to Groq Llama 3.1 8B Instant with sub-second latency, no user
config required. Users can still opt into Anthropic (BYO key),
OpenAI-compatible endpoints (Ollama/Groq/OpenRouter), or local llama.cpp.
Proxy is gated by a shared Bearer token embedded in the binary, throttled
to 20 req/min per IP via Upstash Redis. Per-user magic-link auth + quotas
land in Tranche 2.

---

## Status snapshot

- **Core app** — working (transcription + pastes + history)
- **Voice commands** — Parlia Cloud (default, zero-config),
  Anthropic (BYO key), OpenAI-compatible (Ollama/Groq/OpenRouter/etc.)
- **Local LLM** — kept as opt-in, still crashes on macOS aarch64
  (llama.cpp vsnprintf bug, not fixable from Rust)
- **Parlia Cloud backend** — Vercel Edge route at
  `www.parlia.fr/api/v1/commands` relays to Groq Llama 3.1 8B Instant;
  Upstash Redis per-IP rate limit (20 req/min/IP); health endpoint at
  `/api/v1/health` gated by the shared token
- **Distribution** — v0.7.12 signed + notarized on GitHub Releases;
  landing at www.parlia.fr
- **Brand** — unified around blue (#116cf5), Parlia identity in place
- **Legal** — MIT licence present in repo, not yet surfaced in the app
  (blocker for public distribution)

---

## What shipped on 2026-04-23 (v0.7.12)

### Phase 0 hygiene — token rotation + rate limit
- Rotated the Parlia Cloud shared Bearer token (old v0.7.10/v0.7.11
  token was exposed in authoring chats and could no longer be
  trusted). Old token now returns 401, new token works end-to-end.
- Per-IP rate limit via Upstash Redis on `/api/v1/commands`: 20
  requests per IP per 60 s sliding window. Coarse but protects the
  Groq free-tier budget while per-user auth lands in tranche 2.
- Tolerates both env var conventions for Upstash (`UPSTASH_REDIS_REST_*`
  manual + `KV_REST_API_*` Vercel Marketplace integration), so the
  same code works regardless of how Upstash was connected.
- New diagnostic endpoint at `/api/v1/health` (gated by the shared
  token): reports which env vars are present and whether Upstash is
  reachable without leaking any values. Useful for future auth debug.
- Fails open if Upstash is unreachable / misconfigured — better a
  rate-limit miss than a user-facing outage.
- v0.7.11 installs stop serving Parlia Cloud commands until they
  update to v0.7.12. Given the user base is ~1 person today, no
  grace window.

---

## What shipped on 2026-04-23 (v0.7.11)

### Parlia Cloud — hosted proxy (new default)
- Vercel Edge route at `www.parlia.fr/api/v1/commands` relays to
  Groq Llama 3.1 8B Instant (<500 ms p50, free tier)
- Auth is a single Bearer token shared between the app binary and
  the Vercel env (`PARLIA_SHARED_TOKEN`). Trivial to extract from a
  decompiled build — rotate before scaling and move to per-user
  magic-link auth in the next tranche.
- Env vars: `GROQ_API_KEY` + `PARLIA_SHARED_TOKEN` on the
  `parlia_lp` Vercel project (same project that serves the
  landing page — pragmatic for tranche 1, will be split into a
  dedicated `parlia-api` repo when auth + billing land).
- New `CommandsLlmProvider::Parlia` variant, set as default for new
  installs and first in the provider dropdown
- Tested end-to-end: 158-239 ms round-trip, responds in FR as
  expected

### SEO fix
- `metadataBase` in `parlia_lp/src/app/layout.tsx` was pointing at
  `parlia.app` (a domain owned by another product — a Spanish AAC
  app). Corrected to `www.parlia.fr` (the real production domain).

### Known follow-ups
- Shared Bearer token is in plaintext in the binary. OK for testing;
  **must** be rotated + wrapped by real auth before a broad launch.
- Rate-limiting on `/api/v1/commands` is none for now. Add Upstash
  Redis + per-IP throttle if abuse shows up.
- `bundle_dmg.sh` still flaky (needs a clean `/Volumes` before each
  build). Worked around manually this release.

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

### Dogfooding window (this week — cheap, high signal)

- Use Parlia Cloud daily for 48-72 h; keep a running list of
  friction points, bugs, and quality issues with Groq Llama 3.1 8B
- End-to-end test on a **fresh Mac** (not the dev machine) with the
  landing-page download flow — validates Gatekeeper UX on a real
  user's first install
- Try Parlia Cloud vs Anthropic Haiku on 5-10 of your own commands
  side-by-side and write down the quality delta — informs whether
  Groq is good enough as the default for tranche 2 or whether we
  should route free users to Anthropic via the proxy instead

### Tranche 2 — auth + quota (~2-3 days, blocked on decisions below)

Replaces the shared-token MVP with per-user magic-link auth and
per-user quotas. Detailed plan in chat; summary here:

**Phase A — backend on `parlia_lp`** (~1 day)
- `POST /api/v1/auth/request` (email → magic link via Resend)
- `GET  /api/v1/auth/verify` (token → JWT → redirect to
  `parlia://auth?jwt=…`)
- `GET  /api/v1/me` (JWT → email, plan, quota used/limit)
- `POST /api/v1/commands`: swap Bearer shared token for per-user JWT,
  `INCR quota:{user_id}:{YYYY-MM-DD}` with 48 h TTL, 429 on overage
- Storage: Upstash Redis (same instance as the rate limit); schema
  all hash-based, zero migrations

**Phase B — desktop app** (~1 day)
- `tauri-plugin-deep-link` registering `parlia://` scheme
- New `auth.rs` manager storing JWT in macOS Keychain via the
  `keyring` crate
- Login screen when Parlia Cloud is selected and no JWT exists
- Quota bar in Settings (`43/50 used today`), toast on 429
- `generate_parlia_cloud()` sends `Bearer <JWT>` instead of the
  shared token; intercept 401 → emit `auth-required` event

**Phase C — ship v0.8.0** (~½ day)
- Remove the shared token entirely (no grace window)
- Build, sign, notarize, release, landing page bump

**Decisions to take before starting**:
- **Quota** — proposed 50 commands/day free, Pro unlimited or
  1 500/mo. Ratify or adjust.
- **Email sender** — `hello@parlia.fr` (needs SPF + DKIM DNS records
  on the .fr domain, ~5 min) or start with `onboarding@resend.dev`
  and migrate later.
- **Accounts needed** — Upstash (already done), Resend (free tier,
  3 000 emails/month — to create).
- **When to split the API** — proxy code currently lives in
  `parlia_lp/src/app/api/v1/…`. Extract into its own `parlia-api`
  repo at the same time as Tranche 2 backend (clean boundary) or
  defer to Tranche 3 (one less move).

### Tranche 3 — billing (~1-2 days, after Tranche 2 has 10+ users)

- Stripe Checkout + `customer.subscription.updated` webhook
- Pricing page on the landing (`/pricing`)
- Upgrade CTA in the app when quota is exhausted
- CGU + politique de confidentialité (France / RGPD) — you're
  processing user data from here on

### Legal / distribution prerequisites (still open)

- **Surface MIT licence in the app**: "Open source & credits"
  section in `AboutSettings` with copyright (© 2025 CJ Pais,
  original Handy author), major deps + their licences, full MIT
  text in a modal or linked page
- **Bundle LICENSE inside `.app`**: currently not copied into
  `Parlia.app/Contents/Resources/`. Either list it as a Tauri
  resource, or generate `LICENSES.md` at build time via
  `cargo-about`
- **Credit line in the landing-page footer** (optional but cheap)

### Operational chores

- **Revoke the old Apple Developer app-specific password**
  `fmhn-bokc-cvvd-gpll` on https://account.apple.com — it was
  shared multiple times in chat. Generate a fresh one for the
  next release.
- **Landing page polish**: 15-30 s demo GIF ("Email …" flow),
  visible value prop above the fold, MIT footer.
- **Vercel domain consistency**: `parlia.fr` → 307 → `www.parlia.fr`.
  Decide whether to keep the www prefix as canonical or flatten to
  apex. Only a UX / SEO detail, not blocking.
- **Split `parlia-api` from `parlia_lp`** — do it at the same
  commit that introduces Tranche 2 auth (clean architectural
  boundary + separate deploy cadence).
- **Auto-updater** — `createUpdaterArtifacts` is still `false` in
  `tauri.conf.json`. Users have to manually re-download each
  release. Wire this when releases become more frequent (after
  Tranche 2 probably).

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
