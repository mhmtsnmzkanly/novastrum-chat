/**
 * Lightweight connection helper for Novastrum.
 * - HTTP helper with normalized envelope { ok, status, body }
 * - Command router for common API calls
 * - WebSocket client with auto-reconnect + heartbeat
 * - Tiny event bus (on/off/emit)
 */
const Connection = (() => {
  const listeners = new Map();
  const statusNodes = new Set();

  let ws = null;
  let heartbeatTimer = null;
  let reconnectTimer = null;
  let backoff = 1200;

  const HEARTBEAT_MS = 15000;
  const MAX_BACKOFF_MS = 30000;
  const STATUS_DEFAULT = { text: "WS bekleniyor…", state: "warn" };
  let lastStatus = STATUS_DEFAULT;

  const numericEventMap = new Map([
    [3, "receive_dm"],
    [7, "discussion_updated"],
  ]);

  // ---- helpers -------------------------------------------------------------
  function updateStatus(text, state) {
    lastStatus = { text: text || STATUS_DEFAULT.text, state };
    statusNodes.forEach((el) => {
      el.textContent = lastStatus.text;
      el.classList.remove("presence-ok", "presence-warn", "presence-error");
      if (state) el.classList.add(`presence-${state}`);
    });
  }

  function attachStatus(targetId) {
    const node = typeof targetId === "string" ? document.getElementById(targetId) : targetId;
    if (!node) return;
    statusNodes.add(node);
    updateStatus(lastStatus.text, lastStatus.state);
  }

  function buildWsUrl() {
    const scheme = location.protocol === "https:" ? "wss" : "ws";
    return `${scheme}://${location.host}/ws`;
  }

  function emit(event, payload) {
    const queue = listeners.get(event);
    if (!queue || !queue.length) return;
    queue.slice().forEach((fn) => {
      try { fn(payload); } catch (err) { console.error("Connection listener error", event, err); }
    });
  }

  function on(event, handler) {
    if (!event || typeof handler !== "function") return;
    if (!listeners.has(event)) listeners.set(event, []);
    listeners.get(event).push(handler);
  }

  function off(event, handler) {
    const queue = listeners.get(event);
    if (!queue) return;
    listeners.set(event, queue.filter((fn) => fn !== handler));
  }

  function mapEventName(raw) {
    if (typeof raw === "number") return numericEventMap.get(raw) || `event_${raw}`;
    if (typeof raw === "string") return raw;
    return null;
  }

  // ---- websocket lifecycle -----------------------------------------------
  function sendHeartbeat() {
    if (ws && ws.readyState === WebSocket.OPEN) {
      try { ws.send(JSON.stringify({ type: "ping", at: Date.now() })); } catch (_) { /* ignore */ }
    }
  }

  function scheduleReconnect() {
    if (reconnectTimer) return;
    const delay = Math.min(MAX_BACKOFF_MS, backoff);
    reconnectTimer = window.setTimeout(() => {
      reconnectTimer = null;
      connect();
    }, delay);
    backoff = Math.min(MAX_BACKOFF_MS, backoff * 1.5);
  }

  function clearTimers() {
    if (heartbeatTimer) { clearInterval(heartbeatTimer); heartbeatTimer = null; }
    if (reconnectTimer) { clearTimeout(reconnectTimer); reconnectTimer = null; }
  }

  function resetSocket() {
    clearTimers();
    if (ws) {
      ws.removeEventListener("open", handleOpen);
      ws.removeEventListener("message", handleMessage);
      ws.removeEventListener("error", handleError);
      ws.removeEventListener("close", handleClose);
      try { ws.close(); } catch (_) { /* ignore */ }
      ws = null;
    }
  }

  function handleOpen() {
    updateStatus("WebSocket bağlı", "ok");
    backoff = 1200;
    sendHeartbeat();
    heartbeatTimer = window.setInterval(sendHeartbeat, HEARTBEAT_MS);
  }

  function handleMessage(evt) {
    let parsed;
    try { parsed = JSON.parse(evt.data); } catch (err) { console.warn("WS parse error", err); return; }
    const eventName = mapEventName(parsed.type);
    if (eventName) emit(eventName, parsed.payload);
  }

  function handleError() {
    updateStatus("WebSocket hatası", "error");
  }

  function handleClose() {
    updateStatus("Bağlantı koptu, yeniden bağlanıyor…", "warn");
    resetSocket();
    scheduleReconnect();
  }

  function connect() {
    if (ws && ws.readyState !== WebSocket.CLOSED) return;
    resetSocket();
    updateStatus("WebSocket bağlanıyor…", "warn");
    ws = new WebSocket(buildWsUrl());
    ws.addEventListener("open", handleOpen);
    ws.addEventListener("message", handleMessage);
    ws.addEventListener("error", handleError);
    ws.addEventListener("close", handleClose);
  }

  function isConnected() {
    return Boolean(ws && ws.readyState === WebSocket.OPEN);
  }

  // ---- HTTP + command routing --------------------------------------------
  async function requestJson(path, method = "POST", payload) {
    try {
      const response = await fetch(path, {
        method,
        credentials: "same-origin",
        headers: { "Content-Type": "application/json" },
        body: payload ? JSON.stringify(payload) : undefined,
      });
      const body = await response.json().catch(() => ({ ok: false, message: "invalid json" }));
      return { ok: response.ok, status: response.status, body };
    } catch (_err) {
      return { ok: false, status: 0, body: { ok: false, message: "network error" } };
    }
  }

  function fail(message) {
    return { ok: false, status: 0, body: { ok: false, message } };
  }

  const routes = {
    discussion_reply: { path: "/api/discussion/post", method: "POST", req: ["user_id", "body"] },
    discussion_vote: { path: "/api/discussion/vote", method: "POST", req: ["user_id", "post_id", "value"] },
    send_dm: { path: "/api/chats/message", method: "POST", req: ["user_id", "chat_id", "body"] },
    mute_chat: { path: "/api/community/mute", method: "POST", req: ["user_id", "chat_id"] },
    edit_message: { path: "/api/chats/message/edit", method: "POST", req: ["user_id", "message_id", "chat_type", "new_body"] },
    delete_message: { path: "/api/chats/message/delete", method: "POST", req: ["user_id", "message_id", "chat_type"] },
    group_rename: { path: "/api/chats/group/rename", method: "POST", req: ["user_id", "chat_id", "name"] },
    group_add_member: { path: "/api/chats/group/member/add", method: "POST", req: ["user_id", "chat_id", "target_user_id"] },
    group_remove_member: { path: "/api/chats/group/member/remove", method: "POST", req: ["user_id", "chat_id", "target_user_id"] },
    group_delete: { path: "/api/chats/group/delete", method: "POST", req: ["user_id", "chat_id"] },
  };

  function prepare(command, payload) {
    const data = { ...(payload || {}) };
    if (command === "discussion_reply") {
      data.thread_id = data.thread_id ?? null;
      data.parent_id = data.parent_id ?? null;
      data.title = data.title ?? "";
    }
    if (command === "send_dm") {
      data.chat_type = data.chat_type || "Community";
      data.body = data.body ?? "";
    }
    return data;
  }

  async function sendCommand(name, payload) {
    const route = routes[name];
    if (!route) return fail(`unsupported command: ${name}`);
    const data = prepare(name, payload);
    for (const key of route.req || []) {
      if (data[key] === undefined || data[key] === null) return fail(`missing ${key}`);
    }
    return requestJson(route.path, route.method, data);
  }

  // Convenience wrappers
  const api = {};
  Object.keys(routes).forEach((key) => { api[key] = (payload) => sendCommand(key, payload); });
  api.join_community = () => fail("join_community not implemented");

  return {
    connect,
    isConnected,
    on,
    off,
    attachStatus,
    ...api,
  };
})();

window.Connection = Connection;
