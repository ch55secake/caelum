import { useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { AppInfo, UpdateCheck } from "../domain/settings/about.js";

// The settings dialog owns this lifetime, so checks survive section navigation
// and dismissed dialogs cannot replace the result in a later one.
export function useAbout() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [infoError, setInfoError] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);
  const [checking, setChecking] = useState(false);
  const [update, setUpdate] = useState<
    (UpdateCheck & { checkedAt: Date }) | null
  >(null);
  const [updateError, setUpdateError] = useState<string | null>(null);
  const pending = useRef(false);
  const generation = useRef(0);
  useEffect(() => {
    let disposed = false;
    setInfoError(null);
    if (!isTauri()) {
      setInfoError(
        "Open the desktop app to see its version and check for updates.",
      );
      return;
    }
    void invoke<AppInfo>("app_info").then(
      (result) => {
        if (!disposed) setInfo(result);
      },
      (reason) => {
        if (!disposed) setInfoError(String(reason));
      },
    );
    return () => {
      disposed = true;
    };
  }, [attempt]);
  useEffect(
    () => () => {
      generation.current++;
      pending.current = false;
    },
    [],
  );
  async function check() {
    if (!info || pending.current) return;
    pending.current = true;
    const request = ++generation.current;
    setChecking(true);
    setUpdateError(null);
    try {
      const result = await invoke<UpdateCheck>("check_for_updates");
      if (generation.current === request)
        setUpdate({ ...result, checkedAt: new Date() });
    } catch (reason) {
      if (generation.current === request) setUpdateError(String(reason));
    } finally {
      if (generation.current === request) {
        pending.current = false;
        setChecking(false);
      }
    }
  }
  return {
    info,
    infoError,
    retryInfo: () => setAttempt((value) => value + 1),
    checking,
    update,
    updateError,
    check,
  };
}
