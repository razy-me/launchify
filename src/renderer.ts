import { getCurrentWebview } from "@tauri-apps/api/webview";
import { convertFile, getConvertTargets, resolveIconUrl, resizeLauncher, revealPath } from "./ipc";
import { computeTargetHeight } from "./state";
import type { ItemKind, LauncherState, PrefixMode, SearchResult } from "./types";

export interface DomRefs {
  launcher: HTMLElement;
  input: HTMLInputElement;
  modeBadge: HTMLElement;
  inlinePreview: HTMLElement;
  resultsList: HTMLElement;
}

const MODE_BADGE_MAP: Record<PrefixMode, string> = {
  Global: "",
  Shell: ">",
  Calc: "=",
  Path: "/",
  Window: "@",
  Convert: "⇄",
};

const KIND_FALLBACK_GLYPH: Record<ItemKind, string> = {
  App: "A",
  StoreApp: "S",
  SystemSetting: "⚙",
  PowerAction: "⏻",
  Folder: "📁",
  File: "📄",
  Workflow: "⚡",
  WebShortcut: "↗",
  Window: "❐",
  InlineResult: "=",
  ShellCommand: ">_",
};

export function getDomRefs(): DomRefs {
  const launcher = document.getElementById("launcher");
  const input = document.getElementById("search-input") as HTMLInputElement | null;
  const modeBadge = document.getElementById("mode-badge");
  const inlinePreview = document.getElementById("inline-preview");
  const resultsList = document.getElementById("results-list");

  if (!launcher || !input || !modeBadge || !inlinePreview || !resultsList) {
    throw new Error("Launchify DOM elements missing");
  }

  return { launcher, input, modeBadge, inlinePreview, resultsList };
}

export function renderVisibility(refs: DomRefs, visible: boolean): void {
  refs.launcher.classList.toggle("is-visible", visible);
}

export function renderState(
  refs: DomRefs,
  state: LauncherState,
  onSelectAndExecute: (index: number, secondary: boolean) => void
): void {
  const badgeText = MODE_BADGE_MAP[state.mode];
  if (badgeText) {
    refs.modeBadge.textContent = badgeText;
    refs.modeBadge.hidden = false;
  } else {
    refs.modeBadge.textContent = "";
    refs.modeBadge.hidden = true;
  }

  if (state.inlinePreview) {
    refs.inlinePreview.textContent = state.inlinePreview.display;
    refs.inlinePreview.title = state.inlinePreview.kind_label;
    refs.inlinePreview.hidden = false;
  } else {
    refs.inlinePreview.textContent = "";
    refs.inlinePreview.hidden = true;
  }

  const wasOpen = refs.launcher.classList.contains("has-results");
  if (state.mode === "Convert") {
    refs.launcher.classList.remove("has-results");
    refs.launcher.classList.add("has-convert");
    refs.resultsList.hidden = true;
    refs.resultsList.classList.remove("animate-enter");

    const panel = ensureConvertPanel(refs.launcher);
    panel.hidden = false;

    const stripped = state.query.replace(/^convert\s*/i, "").trim();
    if (stripped && stripped !== convertLastPrefill) {
      convertLastPrefill = stripped;
      setConvertSourcePath(stripped);
    }

    const targetHeight = 380;
    if (targetHeight !== state.lastHeightPx) {
      state.lastHeightPx = targetHeight;
      void resizeLauncher(targetHeight).catch(() => {
        // Ignore resize errors when running in browser preview without Tauri
      });
    }
    return;
  }

  refs.launcher.classList.remove("has-convert");
  const existingConvertPanel = document.getElementById("convert-panel");
  if (existingConvertPanel) {
    existingConvertPanel.hidden = true;
  }
  convertLastPrefill = "";

  const hasResults = state.results.length > 0;
  refs.launcher.classList.toggle("has-results", hasResults);
  refs.resultsList.classList.toggle("animate-enter", !wasOpen && hasResults);
  refs.resultsList.hidden = !hasResults;

  // FLIP Step 1 (First): capture existing row positions & reuse DOM nodes by item id
  const prevTops = new Map<string, number>();
  const existingRows = new Map<string, HTMLElement>();
  if (wasOpen && hasResults) {
    refs.resultsList.querySelectorAll<HTMLElement>(".result-row").forEach((row) => {
      const id = row.dataset.id;
      if (id) {
        prevTops.set(id, row.getBoundingClientRect().top);
        existingRows.set(id, row);
      }
    });
  }

  const reusedMoved: { row: HTMLElement; oldTop: number }[] = [];
  const enteringRows: HTMLElement[] = [];

  refs.resultsList.replaceChildren();
  if (hasResults) {
    const indicator = document.createElement("div");
    indicator.id = "selection-indicator";
    if (state.selectedIndex >= 0) {
      indicator.style.transform = `translateY(${state.selectedIndex * 48}px)`;
      indicator.style.opacity = "1";
    } else {
      indicator.style.opacity = "0";
    }

    const fragment = document.createDocumentFragment();
    fragment.appendChild(indicator);
    state.results.forEach((item, index) => {
      const isSelected = index === state.selectedIndex;
      const existing = existingRows.get(item.id);
      if (existing) {
        existing.dataset.index = String(index);
        existing.classList.toggle("is-selected", isSelected);
        existing.classList.remove("is-entering");
        existing.setAttribute("aria-selected", isSelected ? "true" : "false");
        fragment.appendChild(existing);
        const oldTop = prevTops.get(item.id);
        if (oldTop !== undefined) {
          reusedMoved.push({ row: existing, oldTop });
        }
      } else {
        const row = createResultRow(item, index, isSelected, onSelectAndExecute);
        row.dataset.id = item.id;
        if (wasOpen) {
          row.classList.add("is-entering");
          enteringRows.push(row);
        }
        fragment.appendChild(row);
      }
    });
    refs.resultsList.appendChild(fragment);

    // FLIP Step 2 & 3 (Last, Invert, Play)
    if (reusedMoved.length > 0 || enteringRows.length > 0) {
      for (const { row, oldTop } of reusedMoved) {
        const newTop = row.getBoundingClientRect().top;
        const deltaY = oldTop - newTop;
        if (Math.abs(deltaY) > 0.5) {
          row.style.transition = "none";
          row.style.transform = `translateY(${deltaY}px)`;
        }
      }
      requestAnimationFrame(() => {
        for (const { row } of reusedMoved) {
          if (row.style.transform) {
            row.style.transition = "transform 240ms cubic-bezier(0.22, 1, 0.36, 1)";
            row.style.transform = "";
          }
        }
        for (const row of enteringRows) {
          row.classList.remove("is-entering");
        }
      });
    }
  }

  const targetHeight = computeTargetHeight(state.results.length);
  if (targetHeight !== state.lastHeightPx) {
    state.lastHeightPx = targetHeight;
    void resizeLauncher(targetHeight).catch(() => {
      // Ignore resize errors when running in browser preview without Tauri
    });
  }
}

export function updateSelectionOnly(refs: DomRefs, selectedIndex: number): void {
  const indicator = document.getElementById("selection-indicator");
  if (indicator) {
    if (selectedIndex >= 0) {
      indicator.style.transform = `translateY(${selectedIndex * 48}px)`;
      indicator.style.opacity = "1";
    } else {
      indicator.style.opacity = "0";
    }
  }

  const rows = refs.resultsList.querySelectorAll<HTMLElement>(".result-row");
  rows.forEach((row, i) => {
    const isSelected = i === selectedIndex;
    row.classList.toggle("is-selected", isSelected);
    row.setAttribute("aria-selected", isSelected ? "true" : "false");
    if (isSelected) {
      row.scrollIntoView({ block: "nearest" });
    }
  });
}

function createResultRow(
  item: SearchResult,
  index: number,
  isSelected: boolean,
  onSelectAndExecute: (index: number, secondary: boolean) => void
): HTMLElement {
  const row = document.createElement("div");
  row.className = isSelected ? "result-row is-selected" : "result-row";
  row.setAttribute("role", "option");
  row.setAttribute("aria-selected", isSelected ? "true" : "false");
  row.dataset.index = String(index);
  row.style.setProperty("--i", String(index));

  const iconWrap = document.createElement("div");
  iconWrap.className = "result-icon";
  const iconUrl = resolveIconUrl(item.icon_path);
  if (iconUrl) {
    const img = document.createElement("img");
    img.src = iconUrl;
    img.alt = "";
    img.loading = "lazy";
    img.decoding = "async";
    img.addEventListener(
      "error",
      () => {
        iconWrap.replaceChildren(createFallbackGlyph(item.kind));
      },
      { once: true }
    );
    iconWrap.appendChild(img);
  } else {
    iconWrap.appendChild(createFallbackGlyph(item.kind));
  }

  const textWrap = document.createElement("div");
  textWrap.className = "result-text";

  const titleEl = document.createElement("div");
  titleEl.className = "result-title";
  titleEl.textContent = item.title;
  textWrap.appendChild(titleEl);

  if (item.subtitle) {
    const subEl = document.createElement("div");
    subEl.className = "result-subtitle";
    subEl.textContent = item.subtitle;
    textWrap.appendChild(subEl);
  }

  row.appendChild(iconWrap);
  row.appendChild(textWrap);

  if (item.secondary_hint) {
    const hintEl = document.createElement("div");
    hintEl.className = "result-hint";
    hintEl.textContent = item.secondary_hint;
    row.appendChild(hintEl);
  }

  row.addEventListener("mousedown", (e) => {
    e.preventDefault();
    const secondary = e.shiftKey || e.ctrlKey;
    onSelectAndExecute(index, secondary);
  });

  return row;
}

function createFallbackGlyph(kind: ItemKind): HTMLElement {
  const span = document.createElement("span");
  span.className = "result-icon-fallback";
  span.textContent = KIND_FALLBACK_GLYPH[kind] ?? "•";
  return span;
}

let convertLastPrefill = "";
let convertPanelEl: HTMLElement | null = null;
let convertPathInputEl: HTMLInputElement | null = null;
let convertSourceExtEl: HTMLElement | null = null;
let convertTargetSelectEl: HTMLSelectElement | null = null;
let convertSubmitBtnEl: HTMLButtonElement | null = null;
let convertStatusEl: HTMLElement | null = null;
let convertTargetsSeq = 0;

function extractExt(filePath: string): string {
  const trimmed = filePath.trim().replace(/^["']|["']$/g, "");
  const dot = trimmed.lastIndexOf(".");
  if (dot < 0 || dot === trimmed.length - 1) {
    return "";
  }
  const slash = Math.max(trimmed.lastIndexOf("/"), trimmed.lastIndexOf("\\"));
  if (dot < slash) {
    return "";
  }
  return trimmed.slice(dot + 1).toLowerCase();
}

function setConvertStatus(message: string, kind: "success" | "error" | "info"): void {
  if (!convertStatusEl) {
    return;
  }
  convertStatusEl.textContent = message;
  convertStatusEl.className = `convert-status is-${kind}`;
  convertStatusEl.hidden = !message;
}

async function refreshConvertTargets(sourcePath: string): Promise<void> {
  if (!convertSourceExtEl || !convertTargetSelectEl || !convertSubmitBtnEl) {
    return;
  }
  const cleanPath = sourcePath.trim().replace(/^["']|["']$/g, "");
  const ext = extractExt(cleanPath);
  convertSourceExtEl.textContent = ext ? `.${ext}` : "—";

  const seq = ++convertTargetsSeq;
  if (!cleanPath || !ext) {
    convertTargetSelectEl.replaceChildren();
    convertTargetSelectEl.disabled = true;
    convertSubmitBtnEl.disabled = true;
    return;
  }

  try {
    const targets = await getConvertTargets(cleanPath);
    if (seq !== convertTargetsSeq) {
      return;
    }
    convertTargetSelectEl.replaceChildren();
    if (targets.length === 0) {
      const opt = document.createElement("option");
      opt.value = "";
      opt.textContent = "No targets";
      convertTargetSelectEl.appendChild(opt);
      convertTargetSelectEl.disabled = true;
      convertSubmitBtnEl.disabled = true;
    } else {
      const ffmpegFormats = new Set([
        "mp4", "mkv", "webm", "mov", "avi", "flv", "wmv", "m4v",
        "mp3", "wav", "flac", "ogg", "aac", "m4a", "opus", "wma",
      ]);
      const pandocFormats = new Set(["docx", "odt", "rtf", "epub", "rst", "tex"]);
      for (const fmt of targets) {
        const opt = document.createElement("option");
        opt.value = fmt;
        const upper = fmt.toUpperCase();
        if (ffmpegFormats.has(fmt)) {
          opt.textContent = `${upper} (ffmpeg)`;
        } else if (pandocFormats.has(fmt)) {
          opt.textContent = `${upper} (pandoc)`;
        } else {
          opt.textContent = upper;
        }
        convertTargetSelectEl.appendChild(opt);
      }
      convertTargetSelectEl.disabled = false;
      convertSubmitBtnEl.disabled = false;
    }
  } catch {
    if (seq !== convertTargetsSeq) {
      return;
    }
    convertTargetSelectEl.replaceChildren();
    convertTargetSelectEl.disabled = true;
    convertSubmitBtnEl.disabled = true;
  }
}

function setConvertSourcePath(path: string): void {
  if (!convertPathInputEl) {
    return;
  }
  convertPathInputEl.value = path;
  setConvertStatus("", "info");
  void refreshConvertTargets(path);
}

async function runConvert(): Promise<void> {
  if (!convertPathInputEl || !convertTargetSelectEl || !convertSubmitBtnEl) {
    return;
  }
  const sourcePath = convertPathInputEl.value.trim().replace(/^["']|["']$/g, "");
  const targetFormat = convertTargetSelectEl.value;
  if (!sourcePath || !targetFormat) {
    return;
  }

  convertSubmitBtnEl.disabled = true;
  setConvertStatus("Converting...", "info");
  try {
    const res = await convertFile(sourcePath, targetFormat);
    setConvertStatus(
      `Saved: ${res.output_path} (${res.bytes_written.toLocaleString()} bytes) `,
      "success"
    );
    if (convertStatusEl) {
      const revealBtn = document.createElement("button");
      revealBtn.type = "button";
      revealBtn.className = "convert-reveal-btn";
      revealBtn.textContent = "Reveal in Explorer";
      revealBtn.addEventListener("click", () => {
        void revealPath(res.output_path);
      });
      convertStatusEl.appendChild(revealBtn);
    }
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    setConvertStatus(`Error: ${msg}`, "error");
  } finally {
    convertSubmitBtnEl.disabled = false;
  }
}

function ensureConvertPanel(launcher: HTMLElement): HTMLElement {
  if (convertPanelEl) {
    return convertPanelEl;
  }

  const panel = document.createElement("section");
  panel.id = "convert-panel";
  panel.hidden = true;

  const dropzone = document.createElement("div");
  dropzone.id = "convert-dropzone";
  dropzone.textContent = "Drop a file here, or paste/browse a path below";

  const pathRow = document.createElement("div");
  pathRow.className = "convert-row";

  const pathInput = document.createElement("input");
  pathInput.id = "convert-path-input";
  pathInput.type = "text";
  pathInput.placeholder = "C:\\path\\to\\file.png";
  pathInput.autocomplete = "off";
  pathInput.spellcheck = false;

  const filePicker = document.createElement("input");
  filePicker.type = "file";
  filePicker.hidden = true;

  const browseBtn = document.createElement("button");
  browseBtn.id = "convert-browse-btn";
  browseBtn.type = "button";
  browseBtn.textContent = "Browse";

  pathRow.append(pathInput, browseBtn, filePicker);

  const formatRow = document.createElement("div");
  formatRow.className = "convert-row convert-format-row";

  const sourceExt = document.createElement("span");
  sourceExt.id = "convert-source-ext";
  sourceExt.textContent = "—";

  const arrow = document.createElement("span");
  arrow.className = "convert-arrow";
  arrow.textContent = "→";

  const targetSelect = document.createElement("select");
  targetSelect.id = "convert-target-select";
  targetSelect.disabled = true;

  const submitBtn = document.createElement("button");
  submitBtn.id = "convert-submit-btn";
  submitBtn.type = "button";
  submitBtn.textContent = "Convert";
  submitBtn.disabled = true;

  formatRow.append(sourceExt, arrow, targetSelect, submitBtn);

  const statusEl = document.createElement("div");
  statusEl.id = "convert-status";
  statusEl.className = "convert-status";
  statusEl.hidden = true;

  panel.append(dropzone, pathRow, formatRow, statusEl);
  launcher.appendChild(panel);

  convertPanelEl = panel;
  convertPathInputEl = pathInput;
  convertSourceExtEl = sourceExt;
  convertTargetSelectEl = targetSelect;
  convertSubmitBtnEl = submitBtn;
  convertStatusEl = statusEl;

  pathInput.addEventListener("input", () => {
    setConvertStatus("", "info");
    void refreshConvertTargets(pathInput.value);
  });

  browseBtn.addEventListener("click", () => {
    filePicker.click();
  });

  filePicker.addEventListener("change", () => {
    const file = filePicker.files?.[0] as (File & { path?: string }) | undefined;
    if (file) {
      setConvertSourcePath(file.path ?? file.name);
    }
    filePicker.value = "";
  });

  submitBtn.addEventListener("click", () => {
    void runConvert();
  });

  window.addEventListener("launchify:convert-submit", () => {
    if (!panel.hidden) {
      void runConvert();
    }
  });

  // HTML5 Drag & Drop fallback
  dropzone.addEventListener("dragover", (e) => {
    e.preventDefault();
    dropzone.classList.add("is-drag-over");
  });
  dropzone.addEventListener("dragleave", () => {
    dropzone.classList.remove("is-drag-over");
  });
  dropzone.addEventListener("drop", (e) => {
    e.preventDefault();
    dropzone.classList.remove("is-drag-over");
    const file = e.dataTransfer?.files?.[0] as (File & { path?: string }) | undefined;
    if (file) {
      setConvertSourcePath(file.path ?? file.name);
    }
  });

  // Tauri v2 native drag-drop event
  try {
    void getCurrentWebview()
      .onDragDropEvent((event) => {
        if (panel.hidden) {
          return;
        }
        if (event.payload.type === "enter" || event.payload.type === "over") {
          dropzone.classList.add("is-drag-over");
        } else if (event.payload.type === "leave") {
          dropzone.classList.remove("is-drag-over");
        } else if (event.payload.type === "drop") {
          dropzone.classList.remove("is-drag-over");
          const firstPath = event.payload.paths[0];
          if (firstPath) {
            setConvertSourcePath(firstPath);
          }
        }
      })
      .catch(() => {
        // Ignore outside Tauri
      });
  } catch {
    // Ignore outside Tauri
  }

  return panel;
}
