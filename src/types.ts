export type ItemKind =
  | "App"
  | "StoreApp"
  | "SystemSetting"
  | "PowerAction"
  | "Folder"
  | "File"
  | "Workflow"
  | "WebShortcut"
  | "Window"
  | "InlineResult"
  | "ShellCommand";

export type PrefixMode = "Global" | "Shell" | "Calc" | "Path" | "Window" | "Convert";

export interface ConvertResult {
  outputPath: string;
  bytesWritten: number;
}

export interface InlinePreview {
  display: string;
  copy_value: string;
  kind_label: string;
}

export interface SearchResult {
  id: string;
  title: string;
  subtitle: string;
  kind: ItemKind;
  icon_path: string | null;
  secondary_hint: string | null;
  score: number;
  autocomplete_text: string | null;
}

export interface QueryResponse {
  mode: PrefixMode;
  inline_preview: InlinePreview | null;
  results: SearchResult[];
}

export type ExecuteOutcome =
  | { type: "Launched" }
  | { type: "CopiedToClipboard"; value: string }
  | { type: "WindowFocused" };

export interface LauncherState {
  query: string;
  mode: PrefixMode;
  inlinePreview: InlinePreview | null;
  results: SearchResult[];
  selectedIndex: number;
  visible: boolean;
  querySeq: number;
  lastHeightPx: number;
}
