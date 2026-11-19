import "./styles.css";
import {
  autocompleteQuery,
  executeItem,
  hideLauncher,
  onLauncherHidden,
  onLauncherShown,
  queryBackend,
} from "./ipc";
import { attachKeyboardFlow } from "./keyboard";
import {
  getDomRefs,
  renderState,
  renderVisibility,
  updateSelectionOnly,
} from "./renderer";
import { clampResults, createInitialState, resetQueryState } from "./state";

function bootstrap(): void {
  const refs = getDomRefs();
  const state = createInitialState();
  let executeTimer: number | null = null;

  const runQuery = async (rawInput: string): Promise<void> => {
    state.query = rawInput;
    const seq = ++state.querySeq;

    try {
      const response = await queryBackend(rawInput);
      if (seq !== state.querySeq) {
        return;
      }
      state.mode = response.mode;
      state.inlinePreview = response.inline_preview;
      state.results = clampResults(response.results);
      state.selectedIndex = state.results.length > 0 ? 0 : -1;
      renderState(refs, state, handleSelectAndExecute);
    } catch {
      if (seq !== state.querySeq) {
        return;
      }
      state.results = [];
      state.selectedIndex = -1;
      state.inlinePreview = null;
      renderState(refs, state, handleSelectAndExecute);
    }
  };

  const executeSelected = async (
    index: number,
    secondary: boolean
  ): Promise<void> => {
    const selected = state.results[index];
    if (selected) {
      const rows = refs.resultsList.querySelectorAll<HTMLElement>(".result-row");
      const row = rows[index];

      state.visible = false;
      if (row) {
        row.classList.add("is-launched", "is-green");
      }
      refs.launcher.classList.add("is-launch-fading");

      void executeItem(selected.id, state.query, secondary).catch(() => {
        // Backend logs execution errors; keep UI responsive
      });

      if (executeTimer !== null) {
        window.clearTimeout(executeTimer);
      }
      executeTimer = window.setTimeout(() => {
        executeTimer = null;
        if (!state.visible) {
          refs.launcher.classList.remove("is-launch-fading", "is-closing");
          renderVisibility(refs, false);
          void hideLauncher().catch(() => {
            // Ignore
          });
        }
      }, 650);
      return;
    }

    if (state.inlinePreview) {
      try {
        await executeItem("__inline__", state.query, secondary);
      } catch {
        // Ignore
      }
      hide();
    }
  };

  const handleSelectAndExecute = (index: number, secondary: boolean): void => {
    state.selectedIndex = index;
    updateSelectionOnly(refs, index);
    void executeSelected(index, secondary);
  };

  const clearInput = (): void => {
    refs.input.value = "";
    resetQueryState(state);
    renderState(refs, state, handleSelectAndExecute);
  };

  const hide = (): void => {
    if (!state.visible) {
      return;
    }
    state.visible = false;
    refs.launcher.classList.add("is-closing");
    renderVisibility(refs, false);
    window.setTimeout(() => {
      if (!state.visible) {
        refs.launcher.classList.remove("is-closing");
        void hideLauncher().catch(() => {
          // Ignore when running outside Tauri
        });
      }
    }, 185);
  };

  refs.input.addEventListener("input", () => {
    void runQuery(refs.input.value);
  });

  attachKeyboardFlow(refs.input, state, {
    onMoveSelection: (nextIndex) => {
      state.selectedIndex = nextIndex;
      updateSelectionOnly(refs, nextIndex);
    },
    onExecute: (secondary) => {
      void executeSelected(state.selectedIndex, secondary);
    },
    onAutocomplete: () => {
      const selected = state.results[state.selectedIndex];
      void autocompleteQuery(refs.input.value, selected ? selected.title : null)
        .then((completed) => {
          if (completed !== null && completed !== refs.input.value) {
            refs.input.value = completed;
            refs.input.setSelectionRange(completed.length, completed.length);
            void runQuery(completed);
          }
        })
        .catch(() => {
          // Ignore
        });
    },
    onClearInput: clearInput,
    onHide: hide,
  });

  // Initial visible state for dev/preview; Tauri events control real show/hide
  state.visible = true;
  renderVisibility(refs, true);
  renderState(refs, state, handleSelectAndExecute);

  void onLauncherShown(() => {
    if (executeTimer !== null) {
      window.clearTimeout(executeTimer);
      executeTimer = null;
    }
    state.visible = true;
    refs.launcher.classList.remove("is-closing", "is-launch-fading");
    refs.input.value = "";
    resetQueryState(state);
    renderVisibility(refs, false);
    void runQuery("");
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        if (state.visible) {
          renderVisibility(refs, true);
          refs.input.focus();
          refs.input.select();
        }
      });
    });
  });

  void onLauncherHidden(() => {
    state.visible = false;
    renderVisibility(refs, false);
    clearInput();
  });
}

bootstrap();
