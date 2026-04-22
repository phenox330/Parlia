import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Download, Loader2, Check, Trash2, X } from "lucide-react";
import { commands } from "@/bindings";
import type { LlmModelInfo } from "@/bindings";
import { listen } from "@tauri-apps/api/event";
import { Button } from "../../ui/Button";
import { SettingsGroup } from "../../ui/SettingsGroup";

interface DownloadProgress {
  model_id: string;
  downloaded: number;
  total: number;
  percentage: number;
}

export const LlmModelSelector: React.FC = () => {
  const { t } = useTranslation();
  const [models, setModels] = useState<LlmModelInfo[]>([]);
  const [loadedModelId, setLoadedModelId] = useState<string | null>(null);
  const [downloadProgress, setDownloadProgress] =
    useState<DownloadProgress | null>(null);

  const refreshModels = async () => {
    const result = await commands.getAvailableLlmModels();
    if (result.status === "ok") {
      setModels(result.data);
    }
    const statusResult = await commands.getLlmModelStatus();
    if (statusResult.status === "ok") {
      setLoadedModelId(statusResult.data);
    }
  };

  useEffect(() => {
    refreshModels();
    const unlisten = listen<DownloadProgress>(
      "llm-download-progress",
      (event) => {
        setDownloadProgress(event.payload);
        if (event.payload.percentage >= 100) {
          setDownloadProgress(null);
          refreshModels();
        }
      },
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const handleDownload = async (modelId: string) => {
    await commands.downloadLlmModel(modelId);
    refreshModels();
  };

  const handleCancelDownload = async (modelId: string) => {
    await commands.cancelLlmDownload(modelId);
    setDownloadProgress(null);
    refreshModels();
  };

  const handleDelete = async (modelId: string) => {
    await commands.deleteLlmModel(modelId);
    if (loadedModelId === modelId) {
      setLoadedModelId(null);
    }
    refreshModels();
  };

  const handleActivate = async (modelId: string) => {
    const result = await commands.setActiveLlmModel(modelId);
    if (result.status === "ok") {
      setLoadedModelId(modelId);
    }
  };

  return (
    <SettingsGroup title={t("settings.commands.llmModel.title")}>
      {models.map((model) => {
        const isDownloading =
          model.is_downloading ||
          (downloadProgress?.model_id === model.id &&
            (downloadProgress?.percentage ?? 0) < 100);
        const isActive = loadedModelId === model.id;
        const progress = downloadProgress?.model_id === model.id ? downloadProgress : null;

        return (
          <div key={model.id} className="px-4 py-3">
            <div className="flex items-center justify-between">
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <h3 className="text-sm font-medium">{model.name}</h3>
                  <span className="text-xs text-text/40">
                    {model.size_mb >= 1000
                      ? `${(model.size_mb / 1000).toFixed(1)} GB`
                      : `${model.size_mb} MB`}
                  </span>
                  {isActive && (
                    <span className="flex items-center gap-1 text-xs text-green-500">
                      <Check size={12} />
                      {t("settings.commands.llmModel.ready")}
                    </span>
                  )}
                </div>
                <p className="text-xs text-text/50 mt-0.5">
                  {model.description}
                </p>
              </div>
              <div className="flex items-center gap-2 ml-4">
                {isDownloading ? (
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => handleCancelDownload(model.id)}
                  >
                    <X size={14} />
                  </Button>
                ) : model.is_downloaded ? (
                  <>
                    {!isActive && (
                      <Button
                        variant="secondary"
                        size="sm"
                        onClick={() => handleActivate(model.id)}
                      >
                        {t("settings.commands.llmModel.activate")}
                      </Button>
                    )}
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => handleDelete(model.id)}
                    >
                      <Trash2 size={14} />
                    </Button>
                  </>
                ) : (
                  <Button
                    variant="primary"
                    size="sm"
                    onClick={() => handleDownload(model.id)}
                  >
                    <Download size={14} className="mr-1" />
                    {t("settings.commands.llmModel.download")}
                  </Button>
                )}
              </div>
            </div>
            {isDownloading && progress && (
              <div className="mt-2">
                <div className="w-full bg-mid-gray/20 rounded-full h-1.5">
                  <div
                    className="bg-logo-primary h-1.5 rounded-full transition-all duration-300"
                    style={{ width: `${progress.percentage}%` }}
                  />
                </div>
                <p className="text-xs text-text/40 mt-1">
                  {Math.round(progress.percentage)}%
                </p>
              </div>
            )}
            {!model.is_downloaded && !isDownloading && model.partial_size > 0 && (
              <div className="mt-1">
                <p className="text-xs text-text/40">
                  {t("settings.commands.llmModel.partial")}
                </p>
              </div>
            )}
          </div>
        );
      })}
    </SettingsGroup>
  );
};
