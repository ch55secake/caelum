import type { ProviderKind } from "../../domain/sessions/workspaceState.js";
export function createPublication(
  publish: () => void,
  schedule?: typeof setTimeout,
  cancel?: typeof clearTimeout,
): { defer: () => void; flush: () => void; cancel: () => void };
export function deferredOutput(provider: ProviderKind, method: string): boolean;
