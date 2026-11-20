import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ActionKind, ConvertResult, ExecuteOutcome, QueryResponse } from "./types";

export async function queryBackend(input: string): Promise<QueryResponse> {
  return invoke<QueryResponse>("query", { raw: input });
}

export async function executeItem(
  itemId: string,
  _query: string,
  secondary: boolean
): Promise<ExecuteOutcome> {
  return invoke<ExecuteOutcome>("execute", {
    itemId,
    action: secondary ? "Secondary" : "Primary",
  });
}

export async function executeItemWithAction(
  itemId: string,
  action: ActionKind
): Promise<ExecuteOutcome> {
  return invoke<ExecuteOutcome>("execute", {
    itemId,
    action,
  });
}

export async function autocompleteQuery(
  raw: string,
  topItemTitle: string | null
): Promise<string | null> {
  return invoke<string | null>("autocomplete", { raw, topItemTitle });
}

export async function hideLauncher(): Promise<void> {
  return invoke<void>("hide_launcher");
}

export async function resizeLauncher(heightPx: number, widthPx?: number): Promise<void> {
  return invoke<void>("resize_launcher", { height: heightPx, width: widthPx });
}

export async function getConvertTargets(sourcePath: string): Promise<string[]> {
  return invoke<string[]>("get_convert_targets", { sourcePath });
}

export async function convertFile(
  sourcePath: string,
  targetFormat: string
): Promise<ConvertResult> {
  return invoke<ConvertResult>("convert_file_cmd", { sourcePath, targetFormat });
}

export async function revealPath(path: string): Promise<void> {
  return invoke("reveal_path", { path });
}

export async function pickConvertFile(): Promise<string | null> {
  return invoke("pick_convert_file");
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
  if (iconPath.startsWith("data:")) {
    return iconPath;
  }
  try {
    return convertFileSrc(iconPath);
  } catch {
    return null;
  }
}
