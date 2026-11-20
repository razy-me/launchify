import type { LauncherState, SearchResult } from "./types";

export const MAX_VISIBLE_RESULTS = 8;
export const IDLE_HEIGHT_PX = 56;
export const ROW_HEIGHT_PX = 48;
export const RESULTS_PADDING_PX = 9; // 1px divider + 8px bottom padding

export function createInitialState(): LauncherState {
  return {
    query: "",
    mode: "Global",
    inlinePreview: null,
    results: [],
    selectedIndex: -1,
    visible: false,
    querySeq: 0,
    lastHeightPx: IDLE_HEIGHT_PX,
    actionMenuOpen: false,
    actionIndex: 0,
  };
}

export function clampResults(
  results: SearchResult[],
  max: number = MAX_VISIBLE_RESULTS
): SearchResult[] {
  if (results.length <= max) {
    return results;
  }
  return results.slice(0, max);
}

export function moveSelection(
  current: number,
  count: number,
  delta: 1 | -1
): number {
  if (count <= 0) {
    return -1;
  }
  if (current < 0 || current >= count) {
    return delta === 1 ? 0 : count - 1;
  }
  return (current + delta + count) % count;
}

export function computeTargetHeight(resultCount: number): number {
  const clamped = Math.min(Math.max(0, resultCount), MAX_VISIBLE_RESULTS);
  if (clamped === 0) {
    return IDLE_HEIGHT_PX;
  }
  return IDLE_HEIGHT_PX + RESULTS_PADDING_PX + clamped * ROW_HEIGHT_PX;
}

export function resetQueryState(state: LauncherState): void {
  state.query = "";
  state.mode = "Global";
  state.inlinePreview = null;
  state.results = [];
  state.selectedIndex = -1;
  state.querySeq += 1;
  state.actionMenuOpen = false;
  state.actionIndex = 0;
}
