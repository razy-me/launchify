import { moveSelection } from "./state";
import type { LauncherState } from "./types";

export interface KeyboardHandlers {
  onMoveSelection: (nextIndex: number) => void;
  onExecute: (secondary: boolean) => void;
  onAutocomplete: () => void;
  onClearInput: () => void;
  onHide: () => void;
  onOpenActionMenu: () => void;
  onCloseActionMenu: () => void;
  onMoveActionSelection: (nextIndex: number) => void;
  onExecuteAction: () => void;
  getActionCount: () => number;
}

export function attachKeyboardFlow(
  _input: HTMLInputElement,
  state: LauncherState,
  handlers: KeyboardHandlers
): () => void {
  let altDownAlone = false;

  const onKeyDown = (e: KeyboardEvent): void => {
    if (!state.visible) {
      return;
    }
    if (
      ((e.key === "Alt" && e.location === 1) || (e.key === "Shift" && e.location === 2)) &&
      !e.repeat
    ) {
      altDownAlone = true;
      return;
    }
    altDownAlone = false;

    const isCtrlOnly = e.ctrlKey && !e.altKey && !e.metaKey;

    if (state.actionMenuOpen) {
      const count = handlers.getActionCount();
      if (e.key === "ArrowDown" || (isCtrlOnly && (e.key === "j" || e.key === "J"))) {
        e.preventDefault();
        if (count > 0) {
          const next = moveSelection(state.actionIndex, count, 1);
          handlers.onMoveActionSelection(next);
        }
        return;
      }
      if (e.key === "ArrowUp" || (isCtrlOnly && (e.key === "k" || e.key === "K"))) {
        e.preventDefault();
        if (count > 0) {
          const next = moveSelection(state.actionIndex, count, -1);
          handlers.onMoveActionSelection(next);
        }
        return;
      }
      if (e.key === "Enter") {
        e.preventDefault();
        handlers.onExecuteAction();
        return;
      }
      if (e.key === "ArrowLeft" || e.key === "Escape") {
        e.preventDefault();
        handlers.onCloseActionMenu();
        return;
      }
      return;
    }

    if (
      e.key === "ArrowRight" &&
      state.mode !== "Convert" &&
      state.results.length > 0 &&
      state.selectedIndex >= 0
    ) {
      const len = _input.value.length;
      const atEnd =
        _input.selectionStart === null ||
        (_input.selectionStart === len && _input.selectionEnd === len);
      if (atEnd) {
        e.preventDefault();
        handlers.onOpenActionMenu();
        return;
      }
    }

    if (e.key === "ArrowDown" || (isCtrlOnly && (e.key === "j" || e.key === "J"))) {
      e.preventDefault();
      if (state.results.length > 0) {
        const next = moveSelection(state.selectedIndex, state.results.length, 1);
        handlers.onMoveSelection(next);
      }
      return;
    }

    if (e.key === "ArrowUp" || (isCtrlOnly && (e.key === "k" || e.key === "K"))) {
      e.preventDefault();
      if (state.results.length > 0) {
        const next = moveSelection(state.selectedIndex, state.results.length, -1);
        handlers.onMoveSelection(next);
      }
      return;
    }

    if (e.key === "Enter") {
      if (state.mode === "Convert") {
        if (document.activeElement instanceof HTMLButtonElement) {
          return;
        }
        e.preventDefault();
        window.dispatchEvent(new CustomEvent("launchify:convert-submit"));
        return;
      }
      e.preventDefault();
      const secondary = e.shiftKey || e.ctrlKey;
      handlers.onExecute(secondary);
      return;
    }

    if (e.key === "Tab") {
      e.preventDefault();
      handlers.onAutocomplete();
      return;
    }

    if (e.key === "Escape") {
      e.preventDefault();
      handlers.onHide();
    }
  };

  const onKeyUp = (e: KeyboardEvent): void => {
    if (
      ((e.key === "Alt" && e.location === 1) || (e.key === "Shift" && e.location === 2)) &&
      altDownAlone
    ) {
      altDownAlone = false;
      e.preventDefault();
      handlers.onHide();
    }
  };

  const onWindowBlur = (): void => {
    altDownAlone = false;
    if (state.mode === "Convert") {
      return;
    }
    requestAnimationFrame(() => {
      if (!document.hasFocus() && state.mode !== "Convert") {
        handlers.onHide();
      }
    });
  };

  window.addEventListener("keydown", onKeyDown);
  window.addEventListener("keyup", onKeyUp);
  window.addEventListener("blur", onWindowBlur);

  return () => {
    window.removeEventListener("keydown", onKeyDown);
    window.removeEventListener("keyup", onKeyUp);
    window.removeEventListener("blur", onWindowBlur);
  };
}
