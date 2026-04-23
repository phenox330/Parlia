import React, { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Eye, EyeOff, ExternalLink } from "lucide-react";
import { toast } from "sonner";
import { commands } from "@/bindings";
import type { CommandsLlmProvider, VoiceCommand } from "@/bindings";
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
  const anthropicKey = getSetting("anthropic_api_key") ?? "";
  const customBaseUrl = getSetting("openai_compat_base_url") ?? "";
  const customApiKey = getSetting("openai_compat_api_key") ?? "";
  const customModel = getSetting("openai_compat_model") ?? "";
  const [keyDraft, setKeyDraft] = useState<string>(anthropicKey);
  const [showKey, setShowKey] = useState(false);
  const [baseUrlDraft, setBaseUrlDraft] = useState<string>(customBaseUrl);
  const [customKeyDraft, setCustomKeyDraft] = useState<string>(customApiKey);
  const [showCustomKey, setShowCustomKey] = useState(false);
  const [modelDraft, setModelDraft] = useState<string>(customModel);
  const [voiceCommands, setVoiceCommands] = useState<VoiceCommand[]>([]);
  const addButtonRef = useRef<HTMLButtonElement>(null);

  // Keep the draft in sync when settings refresh (e.g. first load).
  useEffect(() => {
    setKeyDraft(anthropicKey);
  }, [anthropicKey]);

  useEffect(() => {
    setBaseUrlDraft(customBaseUrl);
  }, [customBaseUrl]);

  useEffect(() => {
    setCustomKeyDraft(customApiKey);
  }, [customApiKey]);

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

  const persistKey = () => {
    const trimmed = keyDraft.trim();
    if (trimmed === (anthropicKey ?? "").trim()) return;
    void updateSetting("anthropic_api_key", trimmed === "" ? null : trimmed);
  };

  const persistBaseUrl = () => {
    const trimmed = baseUrlDraft.trim().replace(/\/+$/, "");
    if (trimmed === (customBaseUrl ?? "").trim()) return;
    void updateSetting(
      "openai_compat_base_url",
      trimmed === "" ? null : trimmed,
    );
  };

  const persistCustomKey = () => {
    const trimmed = customKeyDraft.trim();
    if (trimmed === (customApiKey ?? "").trim()) return;
    void updateSetting(
      "openai_compat_api_key",
      trimmed === "" ? null : trimmed,
    );
  };

  const persistModel = () => {
    const trimmed = modelDraft.trim();
    if (trimmed === (customModel ?? "").trim()) return;
    void updateSetting("openai_compat_model", trimmed === "" ? null : trimmed);
  };

  const applyOllamaPreset = () => {
    const url = "http://localhost:11434/v1";
    const model = "qwen2.5:1.5b";
    setBaseUrlDraft(url);
    setModelDraft(model);
    setCustomKeyDraft("");
    void updateSetting("openai_compat_base_url", url);
    void updateSetting("openai_compat_model", model);
    void updateSetting("openai_compat_api_key", null);
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
              <div className="flex items-stretch gap-2">
                <div className="relative flex-1">
                  <input
                    id="anthropic-api-key"
                    type={showKey ? "text" : "password"}
                    value={keyDraft}
                    onChange={(e) => setKeyDraft(e.target.value)}
                    onBlur={persistKey}
                    placeholder="sk-ant-…"
                    autoComplete="off"
                    spellCheck={false}
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
                <div className="relative">
                  <input
                    id="openai-compat-api-key"
                    type={showCustomKey ? "text" : "password"}
                    value={customKeyDraft}
                    onChange={(e) => setCustomKeyDraft(e.target.value)}
                    onBlur={persistCustomKey}
                    placeholder={t(
                      "settings.commands.provider.customApiKeyPlaceholder",
                    )}
                    autoComplete="off"
                    spellCheck={false}
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
