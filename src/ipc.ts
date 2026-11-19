import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ExecuteOutcome, QueryResponse } from "./types";

export async function queryBackend(input: string): Promise<QueryResponse> {
  return invoke<QueryResponse>("query", { input });
}

export async function executeItem(
  itemId: string,
  query: string,
  secondary: boolean
): Promise<ExecuteOutcome> {
  return invoke<ExecuteOutcome>("execute", { itemId, query, secondary });
}

export async function autocompleteQuery(
  input: string,
  selectedId: string | null
): Promise<string | null> {
  return invoke<string | null>("autocomplete", { input, selectedId });
}

export async function hideLauncher(): Promise<void> {
  return invoke<void>("hide_launcher");
}

export async function resizeLauncher(heightPx: number): Promise<void> {
  return invoke<void>("resize_launcher", { heightPx });
}

export async function onLauncherShown(cb: () => void): Promise<UnlistenFn> {
  return listen("launcher://shown", () => cb());
}

export async function onLauncherHidden(cb: () => void): Promise<UnlistenFn> {
  return listen("launcher://hidden", () => cb());
}

export function resolveIconUrl(iconPath: string | null): string | null {
  if (!iconPath) {
    return null;
  }
  try {
    return convertFileSrc(iconPath);
  } catch {
    return null;
  }
}
