import { useEffect, useState, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import { platform } from "@tauri-apps/plugin-os";
import { exit } from "@tauri-apps/plugin-process";
import {
  checkAccessibilityPermission,
  requestAccessibilityPermission,
  checkMicrophonePermission,
  requestMicrophonePermission,
} from "tauri-plugin-macos-permissions-api";
import { toast } from "sonner";
import { commands } from "@/bindings";
import { useSettingsStore } from "@/stores/settingsStore";
import ParliaTextLogo from "../icons/ParliaTextLogo";
import { Keyboard, Mic, Check, Loader2, RefreshCw } from "lucide-react";

/// macOS TCC caches the result of the first AXIsProcessTrusted call per
/// process. If the user grants Accessibility AFTER the app has already
/// started polling, the polling never flips to "granted" until the
/// process restarts. We show a "Restart Parlia" hint once we've been
/// waiting on a permission for this long.
const STALL_HINT_DELAY_MS = 6000;

interface AccessibilityOnboardingProps {
  onComplete: () => void;
}

type PermissionStatus = "checking" | "needed" | "waiting" | "granted";

interface PermissionsState {
  accessibility: PermissionStatus;
  microphone: PermissionStatus;
}

const AccessibilityOnboarding: React.FC<AccessibilityOnboardingProps> = ({
  onComplete,
}) => {
  const { t } = useTranslation();
  const refreshAudioDevices = useSettingsStore(
    (state) => state.refreshAudioDevices,
  );
  const refreshOutputDevices = useSettingsStore(
    (state) => state.refreshOutputDevices,
  );
  const [isMacOS, setIsMacOS] = useState<boolean | null>(null);
  const [permissions, setPermissions] = useState<PermissionsState>({
    accessibility: "checking",
    microphone: "checking",
  });
  const pollingRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const stallTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const errorCountRef = useRef<number>(0);
  const MAX_POLLING_ERRORS = 3;
  const [showRestartHint, setShowRestartHint] = useState(false);
  const [isRestarting, setIsRestarting] = useState(false);

  const allGranted =
    permissions.accessibility === "granted" &&
    permissions.microphone === "granted";

  // Check platform and permission status on mount
  useEffect(() => {
    const currentPlatform = platform();
    const isMac = currentPlatform === "macos";
    setIsMacOS(isMac);

    // Skip immediately on non-macOS - no permissions needed
    if (!isMac) {
      onComplete();
      return;
    }

    // On macOS, check both permissions
    const checkInitial = async () => {
      try {
        const [accessibilityGranted, microphoneGranted] = await Promise.all([
          checkAccessibilityPermission(),
          checkMicrophonePermission(),
        ]);

        // If accessibility is granted, initialize Enigo and shortcuts
        if (accessibilityGranted) {
          try {
            await Promise.all([
              commands.initializeEnigo(),
              commands.initializeShortcuts(),
            ]);
          } catch (e) {
            console.warn("Failed to initialize after permission grant:", e);
          }
        }

        const newState: PermissionsState = {
          accessibility: accessibilityGranted ? "granted" : "needed",
          microphone: microphoneGranted ? "granted" : "needed",
        };

        setPermissions(newState);

        // If both already granted, refresh audio devices and skip ahead
        if (accessibilityGranted && microphoneGranted) {
          await Promise.all([refreshAudioDevices(), refreshOutputDevices()]);
          timeoutRef.current = setTimeout(() => onComplete(), 300);
        }
      } catch (error) {
        console.error("Failed to check permissions:", error);
        toast.error(t("onboarding.permissions.errors.checkFailed"));
        setPermissions({
          accessibility: "needed",
          microphone: "needed",
        });
      }
    };

    checkInitial();
  }, [onComplete, refreshAudioDevices, refreshOutputDevices, t]);

  // Polling for permissions after user clicks a button
  const startPolling = useCallback(() => {
    if (pollingRef.current) return;

    // Arm the stall timer the first time polling starts. The hint stays
    // hidden until we've genuinely been waiting — quick grants (mic dialog
    // accept) never trigger it.
    if (stallTimerRef.current) clearTimeout(stallTimerRef.current);
    stallTimerRef.current = setTimeout(() => {
      setShowRestartHint(true);
    }, STALL_HINT_DELAY_MS);

    pollingRef.current = setInterval(async () => {
      try {
        const [accessibilityGranted, microphoneGranted] = await Promise.all([
          checkAccessibilityPermission(),
          checkMicrophonePermission(),
        ]);

        setPermissions((prev) => {
          const newState = { ...prev };

          if (accessibilityGranted && prev.accessibility !== "granted") {
            newState.accessibility = "granted";
            // Initialize Enigo and shortcuts when accessibility is granted
            Promise.all([
              commands.initializeEnigo(),
              commands.initializeShortcuts(),
            ]).catch((e) => {
              console.warn("Failed to initialize after permission grant:", e);
            });
          }

          if (microphoneGranted && prev.microphone !== "granted") {
            newState.microphone = "granted";
          }

          return newState;
        });

        // If both granted, stop polling, refresh audio devices, and proceed
        if (accessibilityGranted && microphoneGranted) {
          if (pollingRef.current) {
            clearInterval(pollingRef.current);
            pollingRef.current = null;
          }
          if (stallTimerRef.current) {
            clearTimeout(stallTimerRef.current);
            stallTimerRef.current = null;
          }
          setShowRestartHint(false);
          // Now that we have mic permission, refresh audio devices
          await Promise.all([refreshAudioDevices(), refreshOutputDevices()]);
          timeoutRef.current = setTimeout(() => onComplete(), 500);
        }

        // Reset error count on success
        errorCountRef.current = 0;
      } catch (error) {
        console.error("Error checking permissions:", error);
        errorCountRef.current += 1;

        if (errorCountRef.current >= MAX_POLLING_ERRORS) {
          // Stop polling after too many consecutive errors
          if (pollingRef.current) {
            clearInterval(pollingRef.current);
            pollingRef.current = null;
          }
          toast.error(t("onboarding.permissions.errors.checkFailed"));
        }
      }
    }, 1000);
  }, [onComplete, refreshAudioDevices, refreshOutputDevices, t]);

  // Cleanup polling and timeouts on unmount
  useEffect(() => {
    return () => {
      if (pollingRef.current) {
        clearInterval(pollingRef.current);
      }
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
      if (stallTimerRef.current) {
        clearTimeout(stallTimerRef.current);
      }
    };
  }, []);

  const handleRestart = async () => {
    // Quit only. `relaunch()` doesn't survive `bun run tauri dev` (the
    // cargo wrapper exits with the child and never respawns), leaving the
    // user with a dead webview. `exit(0)` is symmetric across dev and
    // production: the user reopens via the Dock (.app) or re-runs the
    // dev command. The copy in `restartHint` tells them so.
    setIsRestarting(true);
    try {
      await exit(0);
    } catch (error) {
      console.error("Failed to quit Parlia:", error);
      toast.error(t("onboarding.permissions.errors.restartFailed"));
      setIsRestarting(false);
    }
  };

  const handleGrantAccessibility = async () => {
    try {
      await requestAccessibilityPermission();
      setPermissions((prev) => ({ ...prev, accessibility: "waiting" }));
      startPolling();
    } catch (error) {
      console.error("Failed to request accessibility permission:", error);
      toast.error(t("onboarding.permissions.errors.requestFailed"));
    }
  };

  const handleGrantMicrophone = async () => {
    try {
      await requestMicrophonePermission();
      setPermissions((prev) => ({ ...prev, microphone: "waiting" }));
      startPolling();
    } catch (error) {
      console.error("Failed to request microphone permission:", error);
      toast.error(t("onboarding.permissions.errors.requestFailed"));
    }
  };

  // Still checking platform/initial permissions
  if (
    isMacOS === null ||
    (permissions.accessibility === "checking" &&
      permissions.microphone === "checking")
  ) {
    return (
      <div className="h-screen w-screen flex items-center justify-center">
        <Loader2 className="w-8 h-8 animate-spin text-text/50" />
      </div>
    );
  }

  // All permissions granted - show success briefly
  if (allGranted) {
    return (
      <div className="h-screen w-screen flex flex-col items-center justify-center gap-4">
        <div className="p-4 rounded-full bg-emerald-500/20">
          <Check className="w-12 h-12 text-emerald-400" />
        </div>
        <p className="text-lg font-medium text-text">
          {t("onboarding.permissions.allGranted")}
        </p>
      </div>
    );
  }

  // Show permissions request screen
  return (
    <div className="h-screen w-screen flex flex-col p-6 gap-6 items-center justify-center">
      <div className="flex flex-col items-center gap-2">
        <ParliaTextLogo width={200} />
      </div>

      <div className="max-w-md w-full flex flex-col items-center gap-4">
        <div className="text-center mb-2">
          <h2 className="text-xl font-semibold text-text mb-2">
            {t("onboarding.permissions.title")}
          </h2>
          <p className="text-text/70">
            {t("onboarding.permissions.description")}
          </p>
        </div>

        {/* Microphone Permission Card */}
        <div className="w-full p-4 rounded-lg bg-white/5 border border-border">
          <div className="flex items-center gap-4">
            <div className="p-3 rounded-full bg-logo-primary/20 shrink-0">
              <Mic className="w-6 h-6 text-logo-primary" />
            </div>
            <div className="flex-1 min-w-0">
              <h3 className="font-medium text-text">
                {t("onboarding.permissions.microphone.title")}
              </h3>
              <p className="text-sm text-text/60 mb-3">
                {t("onboarding.permissions.microphone.description")}
              </p>
              {permissions.microphone === "granted" ? (
                <div className="flex items-center gap-2 text-emerald-400 text-sm">
                  <Check className="w-4 h-4" />
                  {t("onboarding.permissions.granted")}
                </div>
              ) : permissions.microphone === "waiting" ? (
                <div className="flex items-center gap-2 text-text/50 text-sm">
                  <Loader2 className="w-4 h-4 animate-spin" />
                  {t("onboarding.permissions.waiting")}
                </div>
              ) : (
                <button
                  onClick={handleGrantMicrophone}
                  className="px-4 py-2 rounded-lg bg-logo-primary hover:bg-logo-primary/90 text-white text-sm font-medium transition-colors"
                >
                  {t("onboarding.permissions.grant")}
                </button>
              )}
            </div>
          </div>
        </div>

        {/* Accessibility Permission Card */}
        <div className="w-full p-4 rounded-lg bg-white/5 border border-border">
          <div className="flex items-center gap-4">
            <div className="p-3 rounded-full bg-logo-primary/20 shrink-0">
              <Keyboard className="w-6 h-6 text-logo-primary" />
            </div>
            <div className="flex-1 min-w-0">
              <h3 className="font-medium text-text">
                {t("onboarding.permissions.accessibility.title")}
              </h3>
              <p className="text-sm text-text/60 mb-3">
                {t("onboarding.permissions.accessibility.description")}
              </p>
              {permissions.accessibility === "granted" ? (
                <div className="flex items-center gap-2 text-emerald-400 text-sm">
                  <Check className="w-4 h-4" />
                  {t("onboarding.permissions.granted")}
                </div>
              ) : permissions.accessibility === "waiting" ? (
                <div className="flex items-center gap-2 text-text/50 text-sm">
                  <Loader2 className="w-4 h-4 animate-spin" />
                  {t("onboarding.permissions.waiting")}
                </div>
              ) : (
                <button
                  onClick={handleGrantAccessibility}
                  className="px-4 py-2 rounded-lg bg-logo-primary hover:bg-logo-primary/90 text-white text-sm font-medium transition-colors"
                >
                  {t("onboarding.permissions.grant")}
                </button>
              )}
            </div>
          </div>
        </div>

        {/* macOS TCC restart hint — see comment at the top of the file */}
        {showRestartHint && !allGranted && (
          <div className="w-full p-4 rounded-lg bg-amber-500/10 border border-amber-500/30 text-sm space-y-3">
            <p className="text-text/80">
              {t("onboarding.permissions.restartHint")}
            </p>
            <button
              onClick={() => void handleRestart()}
              disabled={isRestarting}
              className="inline-flex items-center gap-2 px-3 py-1.5 rounded-md bg-amber-500/20 hover:bg-amber-500/30 border border-amber-500/40 text-text text-sm font-medium transition-colors disabled:opacity-60"
            >
              {isRestarting ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <RefreshCw className="w-4 h-4" />
              )}
              {t("onboarding.permissions.restartButton")}
            </button>
          </div>
        )}
      </div>
    </div>
  );
};

export default AccessibilityOnboarding;
