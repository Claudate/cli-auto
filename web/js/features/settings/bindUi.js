/**
 * [INPUT]: createUiActions · classic globals · bindUiClick
 * [OUTPUT]: bindGlobalUI / wire — 事件委托只绑意图表（无 invoke）
 * [POS]: A5-2d features/settings；P-ship-D click 纵切 → bindUiClick
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
import {
  createUiActions,
  $,
  $$,
  state,
  toast,
  call,
  g,
} from "./uiActions.js";
import { attachDocumentClick } from "./bindUiClick.js";

let UI_ACTIONS = null;
function actions() {
  if (!UI_ACTIONS) UI_ACTIONS = createUiActions();
  return UI_ACTIONS;
}

/** Global click/change/paste — intention-only via cco* / classic. */
export function bindGlobalUI() {
  if (window.__ccoUiBound) return;
  window.__ccoUiBound = true;
  const UI_ACTIONS = actions();
  window.UI_ACTIONS = UI_ACTIONS;

  if (!window.__ccoCliFitBound) {
    window.__ccoCliFitBound = true;
    let t = null;
    window.addEventListener("resize", () => {
      clearTimeout(t);
      t = setTimeout(() => {
        const st = state();
        if (st?.cliBodyHeight === "auto") call("fitCliBodyHeight");
      }, 80);
    });
  }

  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && typeof g("closeImageLightbox") === "function") {
      const lb = document.getElementById("img-lightbox");
      if (lb && !lb.hidden) {
        e.preventDefault();
        call("closeImageLightbox");
        return;
      }
    }
    if (e.target?.id === "chat-input" && e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      if (!state()?.chatBusy) call("sendChatMessage");
    }
  });

  document.addEventListener(
    "paste",
    (e) => {
      if (state()?.page !== "chat") return;
      const t = e.target;
      const inChat =
        t?.id === "chat-input" ||
        t?.closest?.("#page-chat") ||
        t?.closest?.(".chat-composer");
      if (!inChat) return;
      if (typeof g("handleChatPaste") === "function") {
        Promise.resolve(call("handleChatPaste", e)).catch((err) =>
          toast(String(err?.message || err))
        );
      }
    },
    true
  );

  document.addEventListener("dblclick", (e) => {
    const railItem = e.target?.closest?.(".plan-rail-item[data-plan-rail]");
    if (railItem) {
      e.preventDefault();
      const p = railItem.dataset.planRail;
      if (typeof g("openPlanRailItem") === "function") {
        Promise.resolve(call("openPlanRailItem", p)).catch((err) =>
          toast(String(err?.message || err))
        );
      } else {
        Promise.resolve(call("openPlanFullView", p)).catch((err) =>
          toast(String(err?.message || err))
        );
      }
      return;
    }
    const mgmtItem = e.target?.closest?.(".plans-mgmt-item[data-plans-mgmt]");
    if (mgmtItem) {
      e.preventDefault();
      const p = mgmtItem.dataset.plansMgmt;
      if (typeof g("openPlansMgmtItem") === "function") {
        Promise.resolve(call("openPlansMgmtItem", p)).catch((err) =>
          toast(String(err?.message || err))
        );
      } else {
        Promise.resolve(call("openPlanFullView", p)).catch((err) =>
          toast(String(err?.message || err))
        );
      }
    }
  });

  const sessionSel = document.getElementById("chat-session-select");
  if (sessionSel && !sessionSel.dataset.bound) {
    sessionSel.dataset.bound = "1";
    sessionSel.addEventListener("change", () => {
      const sid = sessionSel.value || "default";
      if (typeof g("switchChatSession") === "function") {
        Promise.resolve(call("switchChatSession", sid)).catch((err) =>
          toast(String(err?.message || err))
        );
      }
    });
  }

  const fileInput = document.getElementById("chat-file-input");
  if (fileInput && !fileInput.dataset.bound) {
    fileInput.dataset.bound = "1";
    fileInput.addEventListener("change", () => {
      if (typeof g("addChatAttachments") === "function") {
        Promise.resolve(call("addChatAttachments", fileInput.files)).catch(
          (err) => toast(String(err?.message || err))
        );
      }
    });
  }

  const dropZones = [
    document.querySelector(".chat-composer"),
    document.querySelector("#page-chat .chat-shell"),
    document.getElementById("chat-messages"),
  ].filter(Boolean);
  for (const zone of dropZones) {
    if (zone.dataset.dropBound) continue;
    zone.dataset.dropBound = "1";
    zone.addEventListener("dragover", (e) => {
      e.preventDefault();
      zone.classList.add("is-drop");
    });
    zone.addEventListener("dragleave", () => {
      zone.classList.remove("is-drop");
    });
    zone.addEventListener("drop", (e) => {
      e.preventDefault();
      zone.classList.remove("is-drop");
      if (
        e.dataTransfer?.files?.length &&
        typeof g("addChatAttachments") === "function"
      ) {
        Promise.resolve(call("addChatAttachments", e.dataTransfer.files)).catch(
          (err) => toast(String(err?.message || err))
        );
      }
    });
  }

  attachDocumentClick({ UI_ACTIONS, call, g, state, $, $$, toast });

  document.addEventListener("change", (e) => {
    const t = e.target;
    if (!t) return;
    if (t.id === "pp-provider") {
      t.dataset.touched = "1";
    }
    if (t.id === "pp-max-parallel" || t.id === "chooser-max-parallel") {
      call("commitSplitMaxParallel", t);
    }
    if (t.id === "pp-auto-start" || t.id === "pp-pause-confirm") {
      const st = state();
      if (!st) return;
      const on = !!t.checked;
      st.autoStartAfterPlan = on;
      const key =
        typeof g("PAUSE_CONFIRM_KEY") === "string"
          ? g("PAUSE_CONFIRM_KEY")
          : "cco.pauseConfirmAfterPlan";
      localStorage.setItem(key, on ? "0" : "1");
      toast(
        on
          ? "已开启：拆分后自动开始（跳过拆分台，有业务可选时仍会停住）"
          : "已关闭：拆分后停在拆分台，确认后再开始"
      );
    }
    if (
      t.id === "chooser-show-executed" ||
      t.id === "plan-rail-show-executed" ||
      t.id === "plans-mgmt-show-executed"
    ) {
      if (typeof g("setShowExecutedPlans") === "function") {
        call("setShowExecutedPlans", !!t.checked);
      } else {
        const st = state();
        if (st) st.showExecutedPlans = !!t.checked;
        try {
          localStorage.setItem("cco.showExecutedPlans", t.checked ? "1" : "0");
        } catch (_) {}
        if (state()?.planChooserOpen && typeof g("renderPlanChooser") === "function") {
          call("renderPlanChooser");
        }
        if (typeof g("renderPlanRail") === "function") call("renderPlanRail");
        if (
          state()?.page === "plans" &&
          typeof g("renderPlansMgmtPage") === "function"
        ) {
          call("renderPlansMgmtPage");
        }
      }
    }
  });

  document.addEventListener("focusin", (e) => {
    const t = e.target;
    if (t?.id === "chooser-max-parallel" || t?.id === "pp-max-parallel") {
      t.dataset.editing = "1";
      t.dataset.touched = "1";
    }
  });
  document.addEventListener("focusout", (e) => {
    const t = e.target;
    if (t?.id === "chooser-max-parallel" || t?.id === "pp-max-parallel") {
      call("commitSplitMaxParallel", t);
    }
  });
  document.addEventListener("input", (e) => {
    const t = e.target;
    if (t?.id === "plan-full-editor") {
      if (typeof g("onPlanFullEditorInput") === "function") {
        call("onPlanFullEditorInput");
      }
      return;
    }
    if (t?.id !== "chooser-max-parallel" && t?.id !== "pp-max-parallel") return;
    t.dataset.touched = "1";
    t.dataset.editing = "1";
    const typed = parseInt(t.value, 10);
    const hidden = $("#pp-max-parallel");
    if (Number.isFinite(typed) && typed > 0 && hidden && t !== hidden) {
      hidden.value = String(Math.max(1, Math.min(32, typed)));
    }
  });

  document.addEventListener(
    "click",
    (e) => {
      const chip = e.target?.closest?.("#cli-status-filters [data-cli-filter]");
      if (!chip) return;
      e.preventDefault();
      const st = state();
      if (!st) return;
      const f = chip.getAttribute("data-cli-filter") || "all";
      st.cliStatusFilter = f;
      st.filterFailedOnly = f === "fail";
      call("renderCliBoard", st.live?.tasks || []);
    },
    true
  );

  try {
    const st = state();
    call(
      "applyCliBodyHeight",
      st?.cliBodyHeight === "auto" ? "auto" : st?.cliBodyHeight || "auto"
    );
    call("bindCliHeightGrip");
  } catch (_) {}
}

/** 兼容旧名 */
export function wire() {
  try {
    if (typeof g("applyLogFontSize") === "function") {
      call("applyLogFontSize", state()?.logFontSize);
    }
  } catch (_) {}
  bindGlobalUI();
}
