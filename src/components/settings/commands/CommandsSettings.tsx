import React, { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Eye, EyeOff, ExternalLink, Check, X } from "lucide-react";
import { toast } from "sonner";
import { commands } from "@/bindings";
import type {
  CommandsLlmProvider,
  SecretStatus,
  VoiceCommand,
} from "@/bindings";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { Button } from "../../ui/Button";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { useSettings } from "../../../hooks/useSettings";
import { LlmModelSelector } from "./LlmModelSelector";
import { CommandCard } from "./CommandCard";

export const CommandsSettings: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const commandsEnabled = getSetting("commands_enabled") ?? true;
  const provider: CommandsLlmProvider =
    (getSetting("commands_llm_provider") as CommandsLlmProvider | undefined) ??
    "parlia";
  const customBaseUrl = getSetting("openai_compat_base_url") ?? "";
  const customModel = getSetting("openai_compat_model") ?? "";
  // API keys live in the OS keychain (v0.7.14+). The settings store no
  // longer echoes them back, so the UI tracks "configured ✓ / not set"
  // via `commands.getSecretStatus()` and only shows the entry input when
  // the user is creating or replacing a key.
  const [secretStatus, setSecretStatus] = useState<SecretStatus | null>(null);
  const [editingAnthropic, setEditingAnthropic] = useState(false);
  const [editingOpenAiCompat, setEditingOpenAiCompat] = useState(false);
  const [keyDraft, setKeyDraft] = useState<string>("");
  const [showKey, setShowKey] = useState(false);
  const [baseUrlDraft, setBaseUrlDraft] = useState<string>(customBaseUrl);
  const [customKeyDraft, setCustomKeyDraft] = useState<string>("");
  const [showCustomKey, setShowCustomKey] = useState(false);
  const [modelDraft, setModelDraft] = useState<string>(customModel);
  const [voiceCommands, setVoiceCommands] = useState<VoiceCommand[]>([]);
  const addButtonRef = useRef<HTMLButtonElement>(null);

  const refreshSecretStatus = useCallback(async () => {
    try {
      setSecretStatus(await commands.getSecretStatus());
    } catch {
      // Surface a single toast — keychain access can fail if the user
      // denies the permission prompt on first read. Silent failure here
      // would leave the UI stuck in "unknown" state forever.
      toast.error(t("settings.commands.provider.keychainReadFailed"));
    }
  }, [t]);

  useEffect(() => {
    void refreshSecretStatus();
  }, [refreshSecretStatus]);

  useEffect(() => {
    setBaseUrlDraft(customBaseUrl);
  }, [customBaseUrl]);

  useEffect(() => {
    setModelDraft(customModel);
  }, [customModel]);

  const focusAddButton = useCallback(() => {
    addButtonRef.current?.focus();
  }, []);

  const refreshCommands = useCallback(async () => {
    const result = await commands.getVoiceCommands();
    if (result.status === "ok") {
      setVoiceCommands(result.data);
    } else {
      toast.error(t("settings.commands.errors.loadFailed"));
    }
  }, [t]);

  useEffect(() => {
    void refreshCommands();
  }, [refreshCommands]);

  const hasDraft = voiceCommands.some(
    (c) => c.keyword.trim() === "" && c.prompt.trim() === "",
  );

  const handleAddCommand = async () => {
    if (hasDraft) return;
    const result = await commands.addVoiceCommand("", "");
    if (result.status === "error") {
      toast.error(t("settings.commands.errors.addFailed"));
      return;
    }
    void refreshCommands();
  };

  const persistKey = async () => {
    const trimmed = keyDraft.trim();
    if (trimmed === "") {
      // Empty draft on blur: cancel the edit, don't touch the keychain.
      setEditingAnthropic(false);
      return;
    }
    await updateSetting("anthropic_api_key", trimmed);
    setKeyDraft("");
    setEditingAnthropic(false);
    await refreshSecretStatus();
  };

  const removeAnthropicKey = async () => {
    await updateSetting("anthropic_api_key", null);
    setKeyDraft("");
    setEditingAnthropic(false);
    await refreshSecretStatus();
  };

  const persistBaseUrl = () => {
    const trimmed = baseUrlDraft.trim().replace(/\/+$/, "");
    if (trimmed === (customBaseUrl ?? "").trim()) return;
    void updateSetting(
      "openai_compat_base_url",
      trimmed === "" ? null : trimmed,
    );
  };

  const persistCustomKey = async () => {
    const trimmed = customKeyDraft.trim();
    if (trimmed === "") {
      setEditingOpenAiCompat(false);
      return;
    }
    await updateSetting("openai_compat_api_key", trimmed);
    setCustomKeyDraft("");
    setEditingOpenAiCompat(false);
    await refreshSecretStatus();
  };

  const removeOpenAiCompatKey = async () => {
    await updateSetting("openai_compat_api_key", null);
    setCustomKeyDraft("");
    setEditingOpenAiCompat(false);
    await refreshSecretStatus();
  };

  const persistModel = () => {
    const trimmed = modelDraft.trim();
    if (trimmed === (customModel ?? "").trim()) return;
    void updateSetting("openai_compat_model", trimmed === "" ? null : trimmed);
  };

  const applyOllamaPreset = async () => {
    const url = "http://localhost:11434/v1";
    const model = "qwen2.5:1.5b";
    setBaseUrlDraft(url);
    setModelDraft(model);
    setCustomKeyDraft("");
    void updateSetting("openai_compat_base_url", url);
    void updateSetting("openai_compat_model", model);
    await updateSetting("openai_compat_api_key", null);
    setEditingOpenAiCompat(false);
    await refreshSecretStatus();
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SettingsGroup title={t("settings.commands.enable")}>
        <div className="px-4 py-3">
          <ToggleSwitch
            checked={commandsEnabled}
            onChange={(enabled) => updateSetting("commands_enabled", enabled)}
            isUpdating={isUpdating("commands_enabled")}
            label={t("settings.commands.enable")}
            description={t("settings.commands.enableDescription")}
            descriptionMode="inline"
            grouped
          />
        </div>
      </SettingsGroup>

      <SettingsGroup title={t("settings.commands.provider.title")}>
        <div className="px-4 py-3 space-y-3">
          <div>
            <label
              htmlFor="llm-provider"
              className="block text-xs text-text/60 mb-1"
            >
              {t("settings.commands.provider.label")}
            </label>
            <select
              id="llm-provider"
              value={provider}
              onChange={(e) =>
                updateSetting(
                  "commands_llm_provider",
                  e.target.value as CommandsLlmProvider,
                )
              }
              className="w-full px-2 py-1.5 text-sm bg-background border border-border rounded-md focus:outline-none focus:ring-1 focus:ring-logo-primary"
            >
              <option value="parlia">
                {t("settings.commands.provider.parlia")}
              </option>
              <option value="anthropic">
                {t("settings.commands.provider.anthropic")}
              </option>
              <option value="custom">
                {t("settings.commands.provider.custom")}
              </option>
              <option value="local">
                {t("settings.commands.provider.local")}
              </option>
            </select>
            <p className="text-xs text-text/50 mt-1">
              {provider === "parlia"
                ? t("settings.commands.provider.parliaDescription")
                : provider === "anthropic"
                  ? t("settings.commands.provider.anthropicDescription")
                  : provider === "custom"
                    ? t("settings.commands.provider.customDescription")
                    : t("settings.commands.provider.localDescription")}
            </p>
          </div>

          {provider === "anthropic" && (
            <div>
              <label
                htmlFor="anthropic-api-key"
                className="block text-xs text-text/60 mb-1"
              >
                {t("settings.commands.provider.apiKeyLabel")}
              </label>
              {secretStatus?.anthropic_set && !editingAnthropic ? (
                <div className="flex items-center gap-2 px-2 py-1.5 border border-border rounded-md bg-mid-gray/5">
                  <Check
                    size={14}
                    className="text-logo-primary shrink-0"
                    aria-hidden
                  />
                  <span className="text-sm text-text/80 flex-1">
                    {t("settings.commands.provider.keyConfigured")}
                  </span>
                  <button
                    type="button"
                    onClick={() => {
                      setKeyDraft("");
                      setEditingAnthropic(true);
                    }}
                    className="text-xs px-2 py-1 border border-border rounded-md hover:bg-mid-gray/10 text-text/80"
                  >
                    {t("settings.commands.provider.replace")}
                  </button>
                  <button
                    type="button"
                    onClick={() => void removeAnthropicKey()}
                    className="text-xs px-2 py-1 border border-border rounded-md hover:bg-red-500/10 hover:border-red-500/50 text-text/70 inline-flex items-center gap-1"
                    aria-label={t("settings.commands.provider.remove")}
                  >
                    <X size={12} />
                    {t("settings.commands.provider.remove")}
                  </button>
                </div>
              ) : (
                <div className="flex items-stretch gap-2">
                  <div className="relative flex-1">
                    <input
                      id="anthropic-api-key"
                      type={showKey ? "text" : "password"}
                      value={keyDraft}
                      onChange={(e) => setKeyDraft(e.target.value)}
                      onBlur={() => void persistKey()}
                      placeholder="sk-ant-…"
                      autoComplete="off"
                      spellCheck={false}
                      autoFocus={editingAnthropic}
                      className="w-full px-2 py-1.5 pr-8 text-sm bg-background border border-border rounded-md focus:outline-none focus:ring-1 focus:ring-logo-primary"
                    />
                    <button
                      type="button"
                      onClick={() => setShowKey((s) => !s)}
                      aria-label={
                        showKey
                          ? t("settings.commands.provider.hideKey")
                          : t("settings.commands.provider.showKey")
                      }
                      className="absolute right-2 top-1/2 -translate-y-1/2 text-text/40 hover:text-text"
                    >
                      {showKey ? <EyeOff size={14} /> : <Eye size={14} />}
                    </button>
                  </div>
                  <a
                    href="https://console.anthropic.com/settings/keys"
                    target="_blank"
                    rel="noopener noreferrer"
                    className="inline-flex items-center gap-1 text-xs px-2 py-1.5 border border-border rounded-md hover:bg-mid-gray/10 text-text/70"
                  >
                    <ExternalLink size={12} />
                    {t("settings.commands.provider.getKey")}
                  </a>
                </div>
              )}
              <p className="text-xs text-text/50 mt-1">
                {t("settings.commands.provider.apiKeyHelp")}
              </p>
            </div>
          )}

          {provider === "custom" && (
            <div className="space-y-3">
              <div className="flex flex-wrap gap-2">
                <button
                  type="button"
                  onClick={applyOllamaPreset}
                  className="text-xs px-2 py-1 border border-border rounded-md hover:bg-mid-gray/10 text-text/80"
                >
                  {t("settings.commands.provider.ollamaPreset")}
                </button>
              </div>

              <div>
                <label
                  htmlFor="openai-compat-base-url"
                  className="block text-xs text-text/60 mb-1"
                >
                  {t("settings.commands.provider.customBaseUrlLabel")}
                </label>
                <input
                  id="openai-compat-base-url"
                  type="text"
                  value={baseUrlDraft}
                  onChange={(e) => setBaseUrlDraft(e.target.value)}
                  onBlur={persistBaseUrl}
                  placeholder="http://localhost:11434/v1"
                  autoComplete="off"
                  spellCheck={false}
                  className="w-full px-2 py-1.5 text-sm bg-background border border-border rounded-md focus:outline-none focus:ring-1 focus:ring-logo-primary"
                />
                <p className="text-xs text-text/50 mt-1">
                  {t("settings.commands.provider.customBaseUrlHelp")}
                </p>
              </div>

              <div>
                <label
                  htmlFor="openai-compat-model"
                  className="block text-xs text-text/60 mb-1"
                >
                  {t("settings.commands.provider.customModelLabel")}
                </label>
                <input
                  id="openai-compat-model"
                  type="text"
                  value={modelDraft}
                  onChange={(e) => setModelDraft(e.target.value)}
                  onBlur={persistModel}
                  placeholder="qwen2.5:1.5b"
                  autoComplete="off"
                  spellCheck={false}
                  className="w-full px-2 py-1.5 text-sm bg-background border border-border rounded-md focus:outline-none focus:ring-1 focus:ring-logo-primary"
                />
                <p className="text-xs text-text/50 mt-1">
                  {t("settings.commands.provider.customModelHelp")}
                </p>
              </div>

              <div>
                <label
                  htmlFor="openai-compat-api-key"
                  className="block text-xs text-text/60 mb-1"
                >
                  {t("settings.commands.provider.customApiKeyLabel")}
                </label>
                {secretStatus?.openai_compat_set && !editingOpenAiCompat ? (
                  <div className="flex items-center gap-2 px-2 py-1.5 border border-border rounded-md bg-mid-gray/5">
                    <Check
                      size={14}
                      className="text-logo-primary shrink-0"
                      aria-hidden
                    />
                    <span className="text-sm text-text/80 flex-1">
                      {t("settings.commands.provider.keyConfigured")}
                    </span>
                    <button
                      type="button"
                      onClick={() => {
                        setCustomKeyDraft("");
                        setEditingOpenAiCompat(true);
                      }}
                      className="text-xs px-2 py-1 border border-border rounded-md hover:bg-mid-gray/10 text-text/80"
                    >
                      {t("settings.commands.provider.replace")}
                    </button>
                    <button
                      type="button"
                      onClick={() => void removeOpenAiCompatKey()}
                      className="text-xs px-2 py-1 border border-border rounded-md hover:bg-red-500/10 hover:border-red-500/50 text-text/70 inline-flex items-center gap-1"
                      aria-label={t("settings.commands.provider.remove")}
                    >
                      <X size={12} />
                      {t("settings.commands.provider.remove")}
                    </button>
                  </div>
                ) : (
                  <div className="relative">
                    <input
                      id="openai-compat-api-key"
                      type={showCustomKey ? "text" : "password"}
                      value={customKeyDraft}
                      onChange={(e) => setCustomKeyDraft(e.target.value)}
                      onBlur={() => void persistCustomKey()}
                      placeholder={t(
                        "settings.commands.provider.customApiKeyPlaceholder",
                      )}
                      autoComplete="off"
                      spellCheck={false}
                      autoFocus={editingOpenAiCompat}
                      className="w-full px-2 py-1.5 pr-8 text-sm bg-background border border-border rounded-md focus:outline-none focus:ring-1 focus:ring-logo-primary"
                    />
                    <button
                      type="button"
                      onClick={() => setShowCustomKey((s) => !s)}
                      aria-label={
                        showCustomKey
                          ? t("settings.commands.provider.hideKey")
                          : t("settings.commands.provider.showKey")
                      }
                      className="absolute right-2 top-1/2 -translate-y-1/2 text-text/40 hover:text-text"
                    >
                      {showCustomKey ? <EyeOff size={14} /> : <Eye size={14} />}
                    </button>
                  </div>
                )}
                <p className="text-xs text-text/50 mt-1">
                  {t("settings.commands.provider.customApiKeyHelp")}
                </p>
              </div>
            </div>
          )}
        </div>
      </SettingsGroup>

      {provider === "local" && <LlmModelSelector />}

      <SettingsGroup
        title={t("settings.commands.title")}
        description={t("settings.commands.description")}
      >
        {voiceCommands.length === 0 ? (
          <div className="px-4 py-6 text-sm text-text/50">
            {t("settings.commands.empty")}
          </div>
        ) : (
          voiceCommands.map((cmd) => (
            <CommandCard
              key={cmd.id}
              command={cmd}
              onUpdate={refreshCommands}
              onDelete={() => {
                void refreshCommands();
                focusAddButton();
              }}
            />
          ))
        )}
        <div className="px-4 py-2">
          <Button
            ref={addButtonRef}
            variant="secondary"
            size="sm"
            onClick={handleAddCommand}
            disabled={hasDraft}
          >
            <Plus size={14} className="mr-1" />
            {t("settings.commands.addCommand")}
          </Button>
        </div>
      </SettingsGroup>
    </div>
  );
};
