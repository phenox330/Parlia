import React, { useEffect, useState, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { Plus } from "lucide-react";
import { commands } from "@/bindings";
import type { VoiceCommand } from "@/bindings";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { Button } from "../../ui/Button";
import { LlmModelSelector } from "./LlmModelSelector";
import { CommandCard } from "./CommandCard";

export const CommandsSettings: React.FC = () => {
  const { t } = useTranslation();
  const [voiceCommands, setVoiceCommands] = useState<VoiceCommand[]>([]);

  const refreshCommands = useCallback(async () => {
    const result = await commands.getVoiceCommands();
    if (result.status === "ok") {
      setVoiceCommands(result.data);
    }
  }, []);

  useEffect(() => {
    refreshCommands();
  }, [refreshCommands]);

  const handleAddCommand = async () => {
    await commands.addVoiceCommand("", "");
    refreshCommands();
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <LlmModelSelector />
      <SettingsGroup
        title={t("settings.commands.title")}
        description={t("settings.commands.description")}
      >
        {voiceCommands.map((cmd) => (
          <CommandCard
            key={cmd.id}
            command={cmd}
            onUpdate={refreshCommands}
            onDelete={refreshCommands}
          />
        ))}
        <div className="px-4 py-2">
          <Button variant="secondary" size="sm" onClick={handleAddCommand}>
            <Plus size={14} className="mr-1" />
            {t("settings.commands.addCommand")}
          </Button>
        </div>
      </SettingsGroup>
    </div>
  );
};
