/**
 * [INPUT]: chatApi · 展示状态
 * [OUTPUT]: 会话列表 / 发送意图；View 绑 DOM
 * [POS]: A2-4 ChatViewModel；禁止 confirm/start_run
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import { createStore } from "../../shared/store.js";
import * as chatApi from "./chatApi.js";

/**
 * @param {{ projectPath?: string|null }} [opts]
 */
export function createChatViewModel(opts = {}) {
  const store = createStore({
    projectPath: opts.projectPath || null,
    /** @type {Array<{session_id?: string, title?: string|null, message_count?: number}>} */
    sessions: [],
    sessionsLoading: false,
    /** last list error (人话) */
    sessionsError: null,
    busy: false,
    lastSendError: null,
  });

  return {
    store,
    getSnapshot: () => store.get(),
    subscribe: (fn) => store.subscribe(fn),

    setProject(path) {
      store.set({ ...store.get(), projectPath: path || null });
    },

    /** A2-4 最小可交付：会话列表经 gateway. */
    async loadSessions() {
      const s = store.get();
      if (!s.projectPath) {
        store.set({
          ...s,
          sessions: [{ session_id: "default", title: null, message_count: 0 }],
          sessionsLoading: false,
          sessionsError: null,
        });
        return store.get().sessions;
      }
      store.set({ ...s, sessionsLoading: true, sessionsError: null });
      try {
        const list = await chatApi.listSessions(s.projectPath);
        const sessions = Array.isArray(list) && list.length
          ? list
          : [{ session_id: "default", title: null, message_count: 0 }];
        store.set({
          ...store.get(),
          sessions,
          sessionsLoading: false,
          sessionsError: null,
        });
        return sessions;
      } catch (e) {
        const msg = e?.message || String(e);
        store.set({
          ...store.get(),
          sessionsLoading: false,
          sessionsError: msg,
          sessions: store.get().sessions.length
            ? store.get().sessions
            : [{ session_id: "default", title: null, message_count: 0 }],
        });
        throw e;
      }
    },

    /**
     * A2-4 最小可交付：发消息经 gateway（不旁路 confirm）.
     * @param {{ message: string, sessionId?: string, attachments?: unknown[]|null }} input
     */
    async send(input) {
      const s = store.get();
      if (!s.projectPath) {
        throw new Error("请先选择项目");
      }
      const text = String(input?.message || "").trim();
      if (!text && !(input?.attachments && input.attachments.length)) {
        throw new Error("请输入内容或添加附件");
      }
      store.set({ ...s, busy: true, lastSendError: null });
      try {
        const resp = await chatApi.sendMessage({
          project: s.projectPath,
          message: text || "（见附件）",
          sessionId: input.sessionId || "default",
          attachments: input.attachments ?? null,
        });
        store.set({ ...store.get(), busy: false, lastSendError: null });
        return resp;
      } catch (e) {
        const msg = e?.message || String(e);
        store.set({ ...store.get(), busy: false, lastSendError: msg });
        throw e;
      }
    },

    /**
     * Save plan markdown — 分配入口的落盘半步；不触发开跑.
     * @param {Record<string, unknown>} args
     */
    async savePlan(args) {
      return chatApi.savePlan(args);
    },
  };
}

export default createChatViewModel;
