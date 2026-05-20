# Build Instructions

This guide covers how to set up the development environment and build Parlia from source across different platforms.

## Prerequisites

### All Platforms

- [Rust](https://rustup.rs/) (latest stable)
- [Bun](https://bun.sh/) package manager
- [Tauri Prerequisites](https://tauri.app/start/prerequisites/)

### Platform-Specific Requirements

#### macOS

- Xcode Command Line Tools
- Install with: `xcode-select --install`

#### Windows

- Microsoft C++ Build Tools
- Visual Studio 2019/2022 with C++ development tools
- Or Visual Studio Build Tools 2019/2022

#### Linux

- Build essentials
- ALSA development libraries
- Install with:

  ```bash
  # Ubuntu/Debian
  sudo apt update
  sudo apt install build-essential libasound2-dev pkg-config libssl-dev libvulkan-dev vulkan-tools glslc libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libgtk-layer-shell0 libgtk-layer-shell-dev patchelf cmake

  # Fedora/RHEL
  sudo dnf groupinstall "Development Tools"
  sudo dnf install alsa-lib-devel pkgconf openssl-devel vulkan-devel \
    gtk3-devel webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel \
    gtk-layer-shell gtk-layer-shell-devel \
    cmake

  # Arch Linux
  sudo pacman -S base-devel alsa-lib pkgconf openssl vulkan-devel \
    gtk3 webkit2gtk-4.1 libappindicator-gtk3 librsvg gtk-layer-shell \
    cmake
  ```

## Setup Instructions

### 1. Clone the Repository

```bash
git clone <your-parlia-repo-url>
cd Parlia
```

### 2. Install Dependencies

```bash
bun install
```

### 3. Start Dev Server

```bash
bun tauri dev
```

### 4. Build for Production

```bash
bun run tauri build
```

This compiles a release binary and generates platform-specific bundles (deb, rpm, AppImage on Linux; dmg on macOS; msi on Windows).

## Linux Install (from source)

The raw binary (`src-tauri/target/release/parlia`) cannot run standalone — it needs Tauri resource files (tray icons, sounds, VAD model) to be co-located at the expected path.

**Install from the deb bundle** (works on any Linux distro):

```bash
cd /tmp
ar x /path/to/Parlia/src-tauri/target/release/bundle/deb/Parlia_*_amd64.deb data.tar.gz
tar xzf data.tar.gz
sudo cp usr/bin/parlia /usr/bin/
sudo cp -r usr/lib/Parlia /usr/lib/
sudo cp -r usr/share/icons/hicolor/* /usr/share/icons/hicolor/
sudo cp usr/share/applications/Parlia.desktop /usr/share/applications/
```

After subsequent rebuilds, only the binary needs re-copying:

```bash
sudo cp src-tauri/target/release/parlia /usr/bin/
```

Resources only need re-copying if they change upstream (new icons, sounds, etc.).

## Releasing a new version (macOS)

Each release builds + signs + notarizes + publishes an auto-updatable
DMG. The Tauri updater verifies the binary against a public key
embedded in the app, so the matching private key must sign every
release.

### One-time setup (already done)

- `tauri.conf.json` has `createUpdaterArtifacts: true`, the updater
  public key, and points `endpoints` at
  `https://www.parlia.fr/api/v1/updater/latest.json`.
- The updater signing private key lives at
  `~/.tauri/parlia-updater.key` (NOT in this repo, NEVER committed).
  Its password is in 1Password under "Parlia updater key password".
- The Apple notarization credential is stored in macOS Keychain under
  profile name `parlia-notarization` (see `xcrun notarytool
  store-credentials` history).

### Per-release flow

1. **Bump the version** in three places so they stay in lockstep:

   ```bash
   # package.json, src-tauri/Cargo.toml, src-tauri/tauri.conf.json
   # Pick the new version (e.g. 0.7.14) and update each file's
   # "version" field.
   ```

2. **Build the signed + updater-signed bundle.** Tauri reads
   `TAURI_SIGNING_PRIVATE_KEY_PATH` + `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
   to sign the updater artifact, alongside the Developer ID code-sign
   that ships with `signingIdentity`. Export both before building:

   ```bash
   export TAURI_SIGNING_PRIVATE_KEY_PATH="$HOME/.tauri/parlia-updater.key"
   # Pull the password from 1Password CLI — never type it inline.
   # Replace the op:// path with your actual 1Password item reference.
   export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(op read 'op://Private/Parlia updater key password/password')"
   bun run tauri build
   ```

   Output lands under
   `src-tauri/target/release/bundle/dmg/Parlia_<version>_aarch64.dmg`
   and a sibling `.sig` file at
   `src-tauri/target/release/bundle/macos/Parlia.app.tar.gz.sig`.

3. **Notarize the DMG** using the keychain profile (no password on
   the command line):

   ```bash
   xcrun notarytool submit \
     src-tauri/target/release/bundle/dmg/Parlia_<version>_aarch64.dmg \
     --keychain-profile "parlia-notarization" --wait
   xcrun stapler staple \
     src-tauri/target/release/bundle/dmg/Parlia_<version>_aarch64.dmg
   ```

4. **Publish the DMG to GitHub Releases** (manual via the web UI, or
   `gh release create v<version> Parlia_<version>_aarch64.dmg
   --notes "..."`). Note the public download URL — it'll be
   `https://github.com/phenox330/Parlia/releases/download/v<version>/Parlia_<version>_aarch64.dmg`.

5. **Update the updater manifest** at
   `parlia_lp/public/api/v1/updater/latest.json`:

   - `version`: the new version (e.g. `0.7.14`)
   - `notes`: short user-facing changelog line
   - `pub_date`: today's ISO date (`date -u +%Y-%m-%dT%H:%M:%SZ`)
   - `platforms.darwin-aarch64.url`: the GitHub Releases DMG URL from
     step 4
   - `platforms.darwin-aarch64.signature`: **the entire contents** of
     `src-tauri/target/release/bundle/macos/Parlia.app.tar.gz.sig`
     (`cat` it and paste — it's a base64 blob that already contains a
     header line; preserve it verbatim)

6. **Commit + push parlia_lp.** Vercel auto-deploys within ~30 s and
   the new manifest is live. Existing installs poll the endpoint per
   their `update_checks_enabled` setting and will prompt the user to
   apply the update.

### Sanity check before publishing

- `curl -s https://www.parlia.fr/api/v1/updater/latest.json | jq .`
  returns the new version + non-empty signature + non-404 URL.
- `spctl -a -vv Parlia_<version>_aarch64.dmg` reports
  `source=Notarized Developer ID`.
- Smoke-test the auto-update on a previous-version install before
  the release becomes broadly visible.

### What v0.7.13 users see

v0.7.13 was built before `createUpdaterArtifacts` was enabled, so
its embedded pubkey was empty and the updater plugin can't verify
any update. Those users need to download v0.7.14 manually one last
time. From v0.7.14 onwards, all updates are automatic.

## Troubleshooting

### AppImage build fails on Arch / rolling-release distros

`linuxdeploy` bundles its own `strip` binary which is too old to process system libraries built with newer toolchains on rolling-release distros (Arch, CachyOS, Manjaro, EndeavourOS).

The error from Tauri:

```
Bundling Parlia_*_amd64.AppImage
failed to bundle project `failed to run linuxdeploy`
```

Tauri swallows the real linuxdeploy error. To see it, run linuxdeploy manually:

```bash
cd src-tauri/target/release/bundle/appimage
~/.cache/tauri/linuxdeploy-x86_64.AppImage --appimage-extract-and-run \
  --appdir Parlia.AppDir --plugin gtk --output appimage
```

**Workaround:** The binary, deb, and rpm bundles all build fine — only the AppImage step fails. To skip it:

```bash
bun run tauri build -- --bundles deb
```

Then install using the deb extraction method above.
