import React, { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Download, Check, Trash2, X } from "lucide-react";
import { toast } from "sonner";
import { commands } from "@/bindings";
import type { LlmModelInfo } from "@/bindings";
import { listen } from "@tauri-apps/api/event";
import { Button } from "../../ui/Button";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { AlertDialog } from "../../ui/AlertDialog";

interface DownloadProgress {
  model_id: string;
  downloaded: number;
  total: number;
  percentage: number;
}

interface ModelRowProps {
  model: LlmModelInfo;
  isActive: boolean;
  isDownloading: boolean;
  isPending: boolean;
  isCancelling: boolean;
  progress: DownloadProgress | null;
  onDownload: (id: string, name: string) => void;
  onCancel: (id: string) => void;
  onActivate: (id: string, name: string) => void;
  onDelete: (id: string) => void;
}

const formatSize = (sizeMb: number, locale: string) => {
  const fmt = new Intl.NumberFormat(locale, { maximumFractionDigits: 1 });
  return sizeMb >= 1024
    ? `${fmt.format(sizeMb / 1024)} GB`
    : `${fmt.format(sizeMb)} MB`;
};

const ModelRow: React.FC<ModelRowProps> = ({
  model,
  isActive,
  isDownloading,
  isPending,
  isCancelling,
  progress,
  onDownload,
  onCancel,
  onActivate,
  onDelete,
}) => {
  const { t, i18n } = useTranslation();

  return (
    <div className="px-4 py-3">
      <div className="flex items-center justify-between">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <h3 className="text-sm font-medium">{model.name}</h3>
            <span className="text-xs text-text/40">
              {formatSize(model.size_mb, i18n.language)}
            </span>
            {isActive && (
              <span className="flex items-center gap-1 text-xs text-green-500">
                <Check size={12} />
                {t("settings.commands.llmModel.ready")}
              </span>
            )}
          </div>
          <p className="text-xs text-text/50 mt-0.5">
            {t(
              `settings.commands.llmModel.descriptions.${model.description_key}`,
              {
                defaultValue: t(
                  "settings.commands.llmModel.descriptions.unknown",
                ),
              },
            )}
          </p>
        </div>
        <div className="flex items-center gap-2 ml-4">
          {isDownloading ? (
            <Button
              variant="ghost"
              size="sm"
              disabled={isCancelling}
              aria-busy={isCancelling}
              onClick={() => onCancel(model.id)}
              aria-label={t("settings.commands.llmModel.cancelDownloadAria")}
            >
              <X size={14} />
            </Button>
          ) : model.is_downloaded ? (
            <>
              {!isActive && (
                <Button
                  variant="secondary"
                  size="sm"
                  disabled={isPending}
                  aria-busy={isPending}
                  onClick={() => onActivate(model.id, model.name)}
                >
                  {t("settings.commands.llmModel.activate")}
                </Button>
              )}
              <Button
                variant="ghost"
                size="sm"
                disabled={isPending}
                aria-busy={isPending}
                onClick={() => onDelete(model.id)}
                aria-label={t("settings.commands.llmModel.deleteModelAria")}
              >
                <Trash2 size={14} />
              </Button>
            </>
          ) : (
            <Button
              variant="primary"
              size="sm"
              disabled={isPending}
              aria-busy={isPending}
              onClick={() => onDownload(model.id, model.name)}
            >
              <Download size={14} className="mr-1" />
              {t("settings.commands.llmModel.download")}
            </Button>
          )}
        </div>
      </div>
      {isDownloading && progress && (
        <div className="mt-2">
          <div
            role="progressbar"
            aria-valuenow={Math.round(progress.percentage)}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuetext={`${Math.round(progress.percentage)}%`}
            aria-label={t("settings.commands.llmModel.progressAria", {
              name: model.name,
            })}
            className="w-full bg-mid-gray/20 rounded-full h-1.5"
          >
            <div
              className="bg-logo-primary h-1.5 rounded-full motion-safe:transition-all motion-safe:duration-300"
              style={{ width: `${progress.percentage}%` }}
            />
          </div>
          {/*
            Visual-only — the progressbar above carries aria-valuetext
            so screen readers announce updates exactly once.
          */}
          <p className="text-xs text-text/40 mt-1" aria-hidden="true">
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
};

export const LlmModelSelector: React.FC = () => {
  const { t } = useTranslation();
  const [models, setModels] = useState<LlmModelInfo[]>([]);
  const [loadedModelId, setLoadedModelId] = useState<string | null>(null);
  const [downloadProgress, setDownloadProgress] =
    useState<DownloadProgress | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  // Per-model in-flight set. Prevents double-clicks from firing overlapping
  // Tauri commands on the same row while letting different rows act
  // concurrently (e.g. activating model B while model A downloads).
  const [pendingModelIds, setPendingModelIds] = useState<Set<string>>(
    () => new Set(),
  );
  const [cancellingModelId, setCancellingModelId] = useState<string | null>(
    null,
  );
  const [deleteConfirmId, setDeleteConfirmId] = useState<string | null>(null);
  const mountedRef = useRef(true);
  const sectionRef = useRef<HTMLDivElement>(null);
  // Last announcement text rendered in the aria-live region. Coarse-grained
  // (updated at 25% milestones) to avoid flooding screen readers on every tick.
  const [announcement, setAnnouncement] = useState("");
  const lastAnnouncedBucketRef = useRef<number>(-1);

  const refreshModels = useCallback(async () => {
    const [listResult, statusResult] = await Promise.all([
      commands.getAvailableLlmModels(),
      commands.getLlmModelStatus(),
    ]);
    if (!mountedRef.current) return;
    if (listResult.status === "ok") {
      setModels(listResult.data);
    } else {
      toast.error(t("settings.commands.llmModel.errors.loadFailed"));
    }
    if (statusResult.status === "ok") {
      setLoadedModelId(statusResult.data);
    }
    setIsLoading(false);
  }, [t]);

  useEffect(() => {
    mountedRef.current = true;
    // Register the listener BEFORE kicking off the initial refresh so
    // progress events fired in the gap aren't dropped.
    const unlistenPromise = listen<DownloadProgress>(
      "llm-download-progress",
      (event) => {
        if (!mountedRef.current) return;
        setDownloadProgress(event.payload);
        const pct = event.payload.percentage;
        const bucket = Math.min(4, Math.floor(pct / 25));
        if (bucket > lastAnnouncedBucketRef.current && pct < 100) {
          lastAnnouncedBucketRef.current = bucket;
          setModels((current) => {
            const m = current.find((x) => x.id === event.payload.model_id);
            if (m) {
              setAnnouncement(
                t("settings.commands.llmModel.announce.downloading", {
                  name: m.name,
                  percent: Math.round(pct),
                }),
              );
            }
            return current;
          });
        }
        if (pct >= 100) {
          lastAnnouncedBucketRef.current = -1;
          setDownloadProgress(null);
          if (mountedRef.current) void refreshModels();
        }
      },
    );
    void refreshModels();
    return () => {
      mountedRef.current = false;
      void unlistenPromise.then((fn) => fn());
    };
  }, [refreshModels]);

  const runWithPending = async (modelId: string, fn: () => Promise<void>) => {
    let claimed = false;
    setPendingModelIds((prev) => {
      if (prev.has(modelId)) return prev;
      claimed = true;
      const next = new Set(prev);
      next.add(modelId);
      return next;
    });
    if (!claimed) return;
    try {
      await fn();
    } finally {
      if (mountedRef.current) {
        setPendingModelIds((prev) => {
          if (!prev.has(modelId)) return prev;
          const next = new Set(prev);
          next.delete(modelId);
          return next;
        });
      }
    }
  };

  const handleDownload = (modelId: string, modelName: string) =>
    runWithPending(modelId, async () => {
      toast.info(
        t("settings.commands.llmModel.toasts.downloadStarted", {
          name: modelName,
        }),
      );
      const result = await commands.downloadLlmModel(modelId);
      if (result.status === "error") {
        toast.error(t("settings.commands.llmModel.errors.downloadFailed"));
      } else if (result.data === "Completed") {
        toast.success(
          t("settings.commands.llmModel.toasts.downloadComplete", {
            name: modelName,
          }),
        );
      } else if (result.data === "Cancelled") {
        toast.info(
          t("settings.commands.llmModel.toasts.downloadCancelled", {
            name: modelName,
          }),
        );
      }
      void refreshModels();
    });

  const handleCancelDownload = async (modelId: string) => {
    // Cancel must not go through runWithPending: the in-flight download holds
    // pendingModelId for its entire duration, so a gated cancel can never fire.
    // Cancel is idempotent (sets an AtomicBool) so we only need local guarding
    // against double-clicks on the cancel button itself.
    if (cancellingModelId === modelId) return;
    setCancellingModelId(modelId);
    try {
      const result = await commands.cancelLlmDownload(modelId);
      if (result.status === "error") {
        toast.error(t("settings.commands.llmModel.errors.cancelFailed"));
      }
      setDownloadProgress(null);
      void refreshModels();
    } finally {
      if (mountedRef.current) setCancellingModelId(null);
    }
  };

  const performDelete = (modelId: string) =>
    runWithPending(modelId, async () => {
      const result = await commands.deleteLlmModel(modelId);
      if (result.status === "error") {
        toast.error(t("settings.commands.llmModel.errors.deleteFailed"));
        return;
      }
      if (loadedModelId === modelId) setLoadedModelId(null);
      // Move focus to the section container before the row unmounts — otherwise
      // keyboard/AT users are stranded on document.body after the delete button
      // disappears along with its row.
      sectionRef.current?.focus();
      void refreshModels();
    });

  const handleDelete = (modelId: string) => {
    setDeleteConfirmId(modelId);
  };

  const handleActivate = (modelId: string, modelName: string) =>
    runWithPending(modelId, async () => {
      // Loading a 2+ GB GGUF through llama.cpp takes 5–15 s with no visible
      // feedback otherwise — the button just sits disabled. A loading toast
      // gives the user a reason to wait instead of clicking again.
      const loadingId = toast.loading(
        t("settings.commands.llmModel.toasts.activating", {
          name: modelName,
          defaultValue: `Activating ${modelName}…`,
        }),
      );
      try {
        const result = await commands.setActiveLlmModel(modelId);
        if (result.status === "ok") {
          setLoadedModelId(modelId);
          toast.success(
            t("settings.commands.llmModel.toasts.activated", {
              name: modelName,
            }),
            { id: loadingId },
          );
        } else {
          toast.error(
            t("settings.commands.llmModel.errors.activateFailed"),
            { id: loadingId },
          );
        }
      } catch (e) {
        toast.error(
          t("settings.commands.llmModel.errors.activateFailed"),
          { id: loadingId },
        );
        throw e;
      }
    });

  return (
    <div
      ref={sectionRef}
      tabIndex={-1}
      aria-label={t("settings.commands.llmModel.title")}
      className="outline-none focus-visible:ring-2 focus-visible:ring-logo-primary rounded-lg"
    >
      <div role="status" aria-live="polite" className="sr-only">
        {announcement}
      </div>
      <SettingsGroup title={t("settings.commands.llmModel.title")}>
        {isLoading ? (
          <div className="px-4 py-3 text-sm text-text/50">
            {t("settings.commands.llmModel.loading")}
          </div>
        ) : (
          models.map((model) => {
            // Prefer the live stream; fall back to the persisted flag only when
            // the stream hasn't published yet (e.g. between mount and first
            // event, or after a failure that drops the stream early).
            const progress =
              downloadProgress?.model_id === model.id ? downloadProgress : null;
            const isDownloading = progress
              ? progress.percentage < 100
              : model.is_downloading;
            return (
              <ModelRow
                key={model.id}
                model={model}
                isActive={loadedModelId === model.id}
                isDownloading={isDownloading}
                isPending={pendingModelIds.has(model.id)}
                isCancelling={cancellingModelId === model.id}
                progress={progress}
                onDownload={handleDownload}
                onCancel={handleCancelDownload}
                onActivate={handleActivate}
                onDelete={handleDelete}
              />
            );
          })
        )}
      </SettingsGroup>
      <AlertDialog
        open={deleteConfirmId !== null}
        title={t("settings.commands.llmModel.deleteConfirmTitle")}
        description={t("settings.commands.llmModel.deleteConfirm")}
        confirmLabel={t("settings.commands.llmModel.confirmDelete")}
        cancelLabel={t("common.cancel")}
        confirmVariant="danger"
        onConfirm={() => {
          const id = deleteConfirmId;
          setDeleteConfirmId(null);
          if (id) void performDelete(id);
        }}
        onCancel={() => setDeleteConfirmId(null)}
      />
    </div>
  );
};
