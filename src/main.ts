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

  const runQuery = async (rawInput: string): Promise<void> => {
    state.query = rawInput;
    const seq = ++state.querySeq;

    if (rawInput.length === 0) {
      resetQueryState(state);
      renderState(refs, state, handleSelectAndExecute);
      return;
    }

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
      try {
        await executeItem(selected.id, state.query, secondary);
      } catch {
        // Backend logs execution errors; keep UI responsive
      }
      return;
    }

    if (state.inlinePreview) {
      try {
        await executeItem("__inline__", state.query, secondary);
      } catch {
        // Ignore
      }
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
    renderVisibility(refs, false);
    void hideLauncher().catch(() => {
      // Ignore when running outside Tauri
    });
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
      void autocompleteQuery(refs.input.value, selected ? selected.id : null)
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
    state.visible = true;
    renderVisibility(refs, true);
    refs.input.focus();
    refs.input.select();
  });

  void onLauncherHidden(() => {
    state.visible = false;
    renderVisibility(refs, false);
    clearInput();
  });
}

bootstrap();
