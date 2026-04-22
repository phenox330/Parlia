import React, { useState, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { Trash2 } from "lucide-react";
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

  const handleSave = useCallback(
    async (newKeyword: string, newPrompt: string, newEnabled: boolean) => {
      await commands.updateVoiceCommand(
        command.id,
        newKeyword,
        newPrompt,
        newEnabled,
      );
      onUpdate();
    },
    [command.id, onUpdate],
  );

  const handleKeywordBlur = () => {
    if (keyword !== command.keyword) {
      handleSave(keyword, prompt, enabled);
    }
  };

  const handlePromptBlur = () => {
    if (prompt !== command.prompt) {
      handleSave(keyword, prompt, enabled);
    }
  };

  const handleToggle = async (checked: boolean) => {
    setEnabled(checked);
    await handleSave(keyword, prompt, checked);
  };

  const handleDelete = async () => {
    await commands.deleteVoiceCommand(command.id);
    onDelete();
  };

  return (
    <div className="px-4 py-3 space-y-2">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3 flex-1">
          <input
            type="text"
            value={keyword}
            onChange={(e) => setKeyword(e.target.value)}
            onBlur={handleKeywordBlur}
            placeholder={t("settings.commands.keywordPlaceholder")}
            className="w-32 px-2 py-1 text-sm bg-background border border-border rounded-md focus:outline-none focus:ring-1 focus:ring-logo-primary"
          />
          <label className="inline-flex items-center cursor-pointer">
            <input
              type="checkbox"
              className="sr-only peer"
              checked={enabled}
              onChange={(e) => handleToggle(e.target.checked)}
            />
            <div className="relative w-9 h-5 bg-mid-gray/20 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-background-ui"></div>
          </label>
        </div>
        <Button variant="ghost" size="sm" onClick={handleDelete}>
          <Trash2 size={14} />
        </Button>
      </div>
      <textarea
        value={prompt}
        onChange={(e) => setPrompt(e.target.value)}
        onBlur={handlePromptBlur}
        placeholder={t("settings.commands.promptPlaceholder")}
        rows={3}
        className="w-full px-2 py-1.5 text-sm bg-background border border-border rounded-md focus:outline-none focus:ring-1 focus:ring-logo-primary resize-none"
      />
    </div>
  );
};
