import { resolveIconUrl, resizeLauncher } from "./ipc";
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
  const hasResults = state.results.length > 0;
  refs.launcher.classList.toggle("has-results", hasResults);
  refs.resultsList.classList.toggle("animate-enter", !wasOpen && hasResults);
  refs.resultsList.hidden = !hasResults;

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
      fragment.appendChild(
        createResultRow(item, index, index === state.selectedIndex, onSelectAndExecute)
      );
    });
    refs.resultsList.appendChild(fragment);
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
