import "./styles.css";
import {
  autocompleteQuery,
  executeItem,
  executeItemWithAction,
  hideLauncher,
  onLauncherHidden,
  onLauncherShown,
  queryBackend,
} from "./ipc";
import { attachKeyboardFlow } from "./keyboard";
import {
  getActionsForItem,
  getDomRefs,
  renderActionMenu,
  renderState,
  renderVisibility,
  updateSelectionOnly,
} from "./renderer";
import { clampResults, createInitialState, resetQueryState } from "./state";
import type { ActionKind } from "./types";

function bootstrap(): void {
  const refs = getDomRefs();
  const state = createInitialState();
  let executeTimer: number | null = null;

  const runQuery = async (rawInput: string): Promise<void> => {
    state.query = rawInput;
    state.actionMenuOpen = false;
    state.actionIndex = 0;
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
    if (!state.visible) {
      return;
    }
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

  const executeWithAction = async (
    index: number,
    action: ActionKind
  ): Promise<void> => {
    if (!state.visible) {
      return;
    }
    const selected = state.results[index];
    if (!selected) {
      return;
    }
    state.actionMenuOpen = false;
    state.actionIndex = 0;
    renderActionMenu(refs, state, (act) => {
      void executeWithAction(state.selectedIndex, act);
    });

    const rows = refs.resultsList.querySelectorAll<HTMLElement>(".result-row");
    const row = rows[index];

    state.visible = false;
    if (row) {
      row.classList.add("is-launched", "is-green");
    }
    refs.launcher.classList.add("is-launch-fading");

    void executeItemWithAction(selected.id, action).catch(() => {
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
  };

  const handleSelectAndExecute = (index: number, secondary: boolean): void => {
    if (state.actionMenuOpen) {
      state.actionMenuOpen = false;
      renderActionMenu(refs, state, () => {});
    }
    state.selectedIndex = index;
    updateSelectionOnly(refs, index);
    void executeSelected(index, secondary);
  };

  document.addEventListener("mousedown", (e) => {
    if (!state.actionMenuOpen) {
      return;
    }
    const target = e.target as HTMLElement | null;
    if (target && target.closest(".action-menu")) {
      return;
    }
    state.actionMenuOpen = false;
    renderActionMenu(refs, state, () => {});
  });

  const clearInput = (): void => {
    refs.input.value = "";
    resetQueryState(state);
    renderState(refs, state, handleSelectAndExecute);
  };

  const INACTIVITY_TIMEOUT_MS = 15_000;
  let inactivityTimer: number | null = null;

  const clearInactivityTimer = (): void => {
    if (inactivityTimer !== null) {
      window.clearTimeout(inactivityTimer);
      inactivityTimer = null;
    }
  };

  const resetInactivityTimer = (): void => {
    clearInactivityTimer();
    if (!state.visible || state.mode === "Convert") {
      return;
    }
    inactivityTimer = window.setTimeout(() => {
      inactivityTimer = null;
      if (state.mode === "Convert") {
        return;
      }
      hide();
    }, INACTIVITY_TIMEOUT_MS);
  };

  const hide = (): void => {
    clearInactivityTimer();
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

  for (const eventName of ["keydown", "pointerdown", "wheel"] as const) {
    document.addEventListener(eventName, () => {
      resetInactivityTimer();
    });
  }

  refs.input.addEventListener("input", () => {
    resetInactivityTimer();
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
      const rawAtRequest = refs.input.value;
      const seqAtRequest = state.querySeq;
      const selected = state.results[state.selectedIndex];
      void autocompleteQuery(rawAtRequest, selected ? selected.title : null)
        .then((completed) => {
          if (
            !state.visible ||
            refs.input.value !== rawAtRequest ||
            state.querySeq !== seqAtRequest
          ) {
            return;
          }
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
    onOpenActionMenu: () => {
      state.actionMenuOpen = true;
      state.actionIndex = 0;
      renderActionMenu(refs, state, (act) => {
        void executeWithAction(state.selectedIndex, act);
      });
    },
    onCloseActionMenu: () => {
      state.actionMenuOpen = false;
      state.actionIndex = 0;
      renderActionMenu(refs, state, (act) => {
        void executeWithAction(state.selectedIndex, act);
      });
      refs.input.focus();
    },
    onMoveActionSelection: (nextIndex) => {
      state.actionIndex = nextIndex;
      renderActionMenu(refs, state, (act) => {
        void executeWithAction(state.selectedIndex, act);
      });
    },
    onExecuteAction: () => {
      const selected = state.results[state.selectedIndex];
      if (!selected) {
        return;
      }
      const actions = getActionsForItem(selected);
      const chosen = actions[state.actionIndex];
      if (chosen) {
        void executeWithAction(state.selectedIndex, chosen.action);
      }
    },
    getActionCount: () => {
      const selected = state.results[state.selectedIndex];
      return selected ? getActionsForItem(selected).length : 0;
    },
  });

  // Initial visible state for dev/preview; Tauri events control real show/hide
  state.visible = true;
  renderVisibility(refs, true);
  renderState(refs, state, handleSelectAndExecute);
  resetInactivityTimer();

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
    resetInactivityTimer();
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
    clearInactivityTimer();
    state.visible = false;
    renderVisibility(refs, false);
    clearInput();
  });
}

bootstrap();
