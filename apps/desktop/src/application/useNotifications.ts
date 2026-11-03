import { useEffect, useRef } from "react";
import type { AppSettings } from "../domain/settings/settingsState.js";
import type { ActivityNotification } from "./workspace/useSessionActivity.js";
import { playSound, prepareSound } from "./sounds.js";

export function useNotifications(
  settings: AppSettings["notifications"],
  subscribe: (listener: (event: ActivityNotification) => void) => () => void,
) {
  const latest = useRef(settings);
  latest.current = settings;
  useEffect(() => {
    let lastSound = 0;
    const unlock = () => {
      if (latest.current.mode !== "off" && latest.current.volume > 0)
        void prepareSound().catch(() => {});
    };
    document.addEventListener("pointerdown", unlock);
    document.addEventListener("keydown", unlock);
    const stop = subscribe((event) => {
      const value = latest.current;
      if (
        value.mode === "off" ||
        value.volume <= 0 ||
        (value.mode === "background" && !event.background)
      )
        return;
      if (event.kind === "shell" && !value.backgroundShells) return;
      if (event.kind === "attention" && !value.inputNeeded) return;
      if (event.outcome === "interrupted") return;
      // Concurrent completions share one gentle cue instead of overlapping.
      if (Date.now() - lastSound < 500) return;
      lastSound = Date.now();
      void playSound(
        value.volume,
        event.kind === "attention" || event.outcome === "failed",
      ).catch(() => {});
    });
    return () => {
      stop();
      document.removeEventListener("pointerdown", unlock);
      document.removeEventListener("keydown", unlock);
    };
  }, []);
}
