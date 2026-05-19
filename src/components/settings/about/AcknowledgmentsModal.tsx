import React, { useEffect, useId, useRef } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { X, ExternalLink } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";

interface Props {
  open: boolean;
  onClose: () => void;
}

/// Verbatim MIT licence text — inlined here so the modal renders even
/// without filesystem access. The legally-binding copy ships as
/// `src-tauri/resources/LICENSE` (bundled into the .app via
/// tauri.conf.json's `resources` glob) and the root LICENSE file in
/// the repo. Keep all three in sync if anyone ever amends the licence.
const MIT_LICENSE_TEXT = `MIT License

Copyright (c) 2025 CJ Pais

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.`;

interface Credit {
  name: string;
  license: string;
  url: string;
}

/// Major third-party libraries Parlia depends on. Names + license shorthand
/// surface attribution; URLs let curious users dig in. The full per-crate
/// licence chain lives in the dependency lockfiles (Cargo.lock / bun.lock).
const CREDITS: readonly Credit[] = [
  {
    name: "Handy",
    license: "MIT — © 2025 CJ Pais",
    url: "https://github.com/cjpais/Handy",
  },
  {
    name: "Whisper.cpp",
    license: "MIT — Georgi Gerganov & contributors",
    url: "https://github.com/ggerganov/whisper.cpp",
  },
  {
    name: "ggml",
    license: "MIT — Georgi Gerganov & contributors",
    url: "https://github.com/ggerganov/ggml",
  },
  {
    name: "llama.cpp",
    license: "MIT — Georgi Gerganov & contributors",
    url: "https://github.com/ggerganov/llama.cpp",
  },
  {
    name: "Silero VAD",
    license: "MIT — Silero AI",
    url: "https://github.com/snakers4/silero-vad",
  },
  {
    name: "transcribe-rs",
    license: "Apache-2.0",
    url: "https://github.com/useful-sensors/transcribe-rs",
  },
  {
    name: "Tauri",
    license: "Apache-2.0 / MIT",
    url: "https://tauri.app",
  },
] as const;

export const AcknowledgmentsModal: React.FC<Props> = ({ open, onClose }) => {
  const { t } = useTranslation();
  const dialogRef = useRef<HTMLDivElement>(null);
  const closeRef = useRef<HTMLButtonElement>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);
  const titleId = useId();

  useEffect(() => {
    if (!open) return;
    previousFocusRef.current = document.activeElement as HTMLElement | null;
    closeRef.current?.focus();
    return () => {
      previousFocusRef.current?.focus();
    };
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  const handleBackdrop = (e: React.MouseEvent<HTMLDivElement>) => {
    if (e.target === e.currentTarget) onClose();
  };

  const handleLinkClick = (url: string) => {
    void openUrl(url);
  };

  return createPortal(
    <div
      className="fixed inset-0 z-50 bg-black/70 backdrop-blur-sm flex items-center justify-center p-6"
      onClick={handleBackdrop}
      role="presentation"
    >
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className="bg-background border border-border rounded-lg shadow-xl w-full max-w-lg max-h-[80vh] flex flex-col"
      >
        <div className="flex items-center justify-between px-5 py-3 border-b border-border">
          <h2 id={titleId} className="text-sm font-semibold text-text">
            {t("settings.about.acknowledgments.title")}
          </h2>
          <button
            ref={closeRef}
            type="button"
            onClick={onClose}
            aria-label={t("settings.about.acknowledgments.close")}
            className="text-text/60 hover:text-text rounded-md focus:outline-none focus-visible:ring-2 focus-visible:ring-logo-primary"
          >
            <X size={16} />
          </button>
        </div>

        <div className="overflow-y-auto px-5 py-4 space-y-4 text-sm">
          <p className="text-text/80 leading-relaxed">
            {t("settings.about.acknowledgments.intro")}
          </p>

          <div>
            <h3 className="text-xs font-semibold text-text/70 uppercase tracking-wide mb-2">
              {t("settings.about.acknowledgments.depsHeading")}
            </h3>
            <ul className="space-y-1.5">
              {CREDITS.map((credit) => (
                <li
                  key={credit.name}
                  className="flex items-start justify-between gap-3"
                >
                  <div className="min-w-0">
                    <button
                      type="button"
                      onClick={() => handleLinkClick(credit.url)}
                      className="text-text hover:text-logo-primary inline-flex items-center gap-1 focus:outline-none focus-visible:underline"
                    >
                      <span className="font-medium">{credit.name}</span>
                      <ExternalLink size={10} className="opacity-60" />
                    </button>
                    <p className="text-text/50 text-xs">{credit.license}</p>
                  </div>
                </li>
              ))}
            </ul>
          </div>

          <div>
            <h3 className="text-xs font-semibold text-text/70 uppercase tracking-wide mb-2">
              {t("settings.about.acknowledgments.licenseHeading")}
            </h3>
            <pre className="text-[11px] leading-relaxed text-text/70 whitespace-pre-wrap font-mono bg-mid-gray/10 border border-border rounded-md p-3 max-h-48 overflow-y-auto">
              {MIT_LICENSE_TEXT}
            </pre>
          </div>
        </div>
      </div>
    </div>,
    document.body,
  );
};
