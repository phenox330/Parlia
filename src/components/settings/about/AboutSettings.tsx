import React, { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { getVersion } from "@tauri-apps/api/app";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { SettingContainer } from "../../ui/SettingContainer";
import { AppDataDirectory } from "../AppDataDirectory";
import { AppLanguageSelector } from "../AppLanguageSelector";
import { LogDirectory } from "../debug";
import { AcknowledgmentsModal } from "./AcknowledgmentsModal";

export const AboutSettings: React.FC = () => {
  const { t } = useTranslation();
  const [version, setVersion] = useState("");
  const [creditsOpen, setCreditsOpen] = useState(false);

  useEffect(() => {
    const fetchVersion = async () => {
      try {
        const appVersion = await getVersion();
        setVersion(appVersion);
      } catch (error) {
        console.error("Failed to get app version:", error);
        setVersion("0.1.2");
      }
    };

    fetchVersion();
  }, []);

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SettingsGroup title={t("settings.about.title")}>
        <AppLanguageSelector descriptionMode="tooltip" grouped={true} />
        <SettingContainer
          title={t("settings.about.version.title")}
          description={t("settings.about.version.description")}
          grouped={true}
        >
          {/* eslint-disable-next-line i18next/no-literal-string */}
          <span className="text-sm font-mono">v{version}</span>
        </SettingContainer>

        <AppDataDirectory descriptionMode="tooltip" grouped={true} />
        <LogDirectory grouped={true} />
      </SettingsGroup>

      {/* Intentionally discreet — a single small line at the bottom of
          About. Satisfies the MIT permission-notice requirement (the
          LICENSE file is bundled in src-tauri/resources/) and the
          upstream Handy attribution without crowding the panel. */}
      <p className="text-xs text-text/40 text-center pt-2">
        <button
          type="button"
          onClick={() => setCreditsOpen(true)}
          className="hover:text-text/70 focus:outline-none focus-visible:underline"
        >
          {t("settings.about.acknowledgments.openButton")}
        </button>
      </p>

      <AcknowledgmentsModal
        open={creditsOpen}
        onClose={() => setCreditsOpen(false)}
      />
    </div>
  );
};
