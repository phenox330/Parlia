import React, { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Trash2 } from "lucide-react";
import { toast } from "sonner";
import type { VoiceCommand } from "@/bindings";
import { commands } from "@/bindings";
import { Button } from "../../ui/Button";

interface CommandCardProps {
  command: VoiceCommand;
  onUpdate: () => void;
  onDelete: () => void;
}

export const CommandCard: React.FC<CommandCardProps> = ({
  command,
  onUpdate,
  onDelete,
}) => {
  const { t } = useTranslation();
  const [keyword, setKeyword] = useState(command.keyword);
  const [prompt, setPrompt] = useState(command.prompt);
  const [enabled, setEnabled] = useState(command.enabled);
  const [isDeleting, setIsDeleting] = useState(false);

  // Re-sync local state when the prop changes (e.g. after a refresh returns
  // a server-normalised value, or the list re-renders with different data).
  useEffect(() => {
    setKeyword(command.keyword);
    setPrompt(command.prompt);
    setEnabled(command.enabled);
  }, [command.id, command.keyword, command.prompt, command.enabled]);

  const persist = useCallback(
    async (nextKeyword: string, nextPrompt: string, nextEnabled: boolean) => {
      const result = await commands.updateVoiceCommand(
        command.id,
        nextKeyword,
        nextPrompt,
        nextEnabled,
      );
      if (result.status === "error") {
        toast.error(t("settings.commands.errors.updateFailed"));
        return;
      }
      onUpdate();
    },
    [command.id, onUpdate, t],
  );

  const maybePersist = () => {
    if (
      keyword !== command.keyword ||
      prompt !== command.prompt ||
      enabled !== command.enabled
    ) {
      void persist(keyword, prompt, enabled);
    }
  };

  const handleToggle = (checked: boolean) => {
    setEnabled(checked);
    void persist(keyword, prompt, checked);
  };

  const performDelete = async () => {
    if (isDeleting) return;
    setIsDeleting(true);
    try {
      const result = await commands.deleteVoiceCommand(command.id);
      if (result.status === "error") {
        toast.error(t("settings.commands.errors.deleteFailed"));
        return;
      }
      onDelete();
    } finally {
      setIsDeleting(false);
    }
  };

  const handleDelete = () => {
    // Destructive action — surface a themed toast with a confirm action
    // rather than a native `window.confirm`. Uses `warning` style and
    // `duration: Infinity` so the confirmation can't time out.
    toast.warning(t("settings.commands.deleteConfirm"), {
      duration: Infinity,
      action: {
        label: t("settings.commands.confirmDelete"),
        onClick: () => {
          void performDelete();
        },
      },
      cancel: {
        label: t("common.cancel"),
        onClick: () => undefined,
      },
    });
  };

  const keywordLabel = keyword.trim()
    ? `${t("settings.commands.enableAria")} (${keyword.trim()})`
    : t("settings.commands.enableAria");

  const keywordId = `command-keyword-${command.id}`;
  const promptId = `command-prompt-${command.id}`;

  return (
    <div className="px-4 py-3 space-y-2">
      <div className="flex items-end justify-between gap-3">
        <div className="flex items-end gap-3 flex-1">
          <div>
            <label
              htmlFor={keywordId}
              className="block text-xs text-text/60 mb-1"
            >
              {t("settings.commands.keyword")}
            </label>
            <input
              id={keywordId}
              type="text"
              value={keyword}
              onChange={(e) => setKeyword(e.target.value)}
              onBlur={maybePersist}
              placeholder={t("settings.commands.keywordPlaceholder")}
              className="w-32 px-2 py-1 text-sm bg-background border border-border rounded-md focus:outline-none focus:ring-1 focus:ring-logo-primary"
            />
          </div>
          <label className="inline-flex items-center cursor-pointer pb-1">
            <input
              type="checkbox"
              className="sr-only peer"
              checked={enabled}
              onChange={(e) => handleToggle(e.target.checked)}
              aria-label={keywordLabel}
            />
            <div className="relative w-9 h-5 bg-mid-gray/20 peer-focus-visible:ring-2 peer-focus-visible:ring-logo-primary peer-focus-visible:ring-offset-2 peer-focus-visible:ring-offset-background rounded-full peer peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-background-ui"></div>
          </label>
        </div>
        <Button
          variant="ghost"
          size="sm"
          disabled={isDeleting}
          aria-busy={isDeleting}
          onClick={handleDelete}
          aria-label={t("settings.commands.deleteAria")}
        >
          <Trash2 size={14} />
        </Button>
      </div>
      <div>
        <label
          htmlFor={promptId}
          className="block text-xs text-text/60 mb-1"
        >
          {t("settings.commands.prompt")}
        </label>
        <textarea
          id={promptId}
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          onBlur={maybePersist}
          placeholder={t("settings.commands.promptPlaceholder")}
          rows={3}
          className="w-full px-2 py-1.5 text-sm bg-background border border-border rounded-md focus:outline-none focus:ring-1 focus:ring-logo-primary resize-none"
        />
      </div>
    </div>
  );
};
