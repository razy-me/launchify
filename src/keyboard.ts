import { moveSelection } from "./state";
import type { LauncherState } from "./types";

export interface KeyboardHandlers {
  onMoveSelection: (nextIndex: number) => void;
  onExecute: (secondary: boolean) => void;
  onAutocomplete: () => void;
  onClearInput: () => void;
  onHide: () => void;
}

export function attachKeyboardFlow(
  input: HTMLInputElement,
  state: LauncherState,
  handlers: KeyboardHandlers
): () => void {
  let altDownAlone = false;

  const onKeyDown = (e: KeyboardEvent): void => {
    if (e.key === "Alt" && e.location === 1 && !e.repeat) {
      altDownAlone = true;
      return;
    }
    altDownAlone = false;

    const isCtrlOnly = e.ctrlKey && !e.altKey && !e.metaKey;

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
      if (input.value.length > 0) {
        handlers.onClearInput();
      } else {
        handlers.onHide();
      }
    }
  };

  const onKeyUp = (e: KeyboardEvent): void => {
    if (e.key === "Alt" && e.location === 1 && altDownAlone) {
      altDownAlone = false;
      e.preventDefault();
      handlers.onHide();
    }
  };

  const onWindowBlur = (): void => {
    altDownAlone = false;
    requestAnimationFrame(() => {
      if (!document.hasFocus()) {
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
