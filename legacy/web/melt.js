const Melt = (() => {
  const wsListeners = new Map();
  let wsSocket = null;
  let pingHandle = null;
  let reconnectHandle = null;
  let backoff = 1200;

  const publicApi = {
    apiBase: "",

    async api(path, method = "POST", payload) {
      try {
        const response = await fetch(`${publicApi.apiBase}${path}`, {
          method,
          credentials: "same-origin",
          headers: {
            "Content-Type": "application/json",
          },
          body: payload ? JSON.stringify(payload) : undefined,
        });

        const body = await response.json().catch(() => ({ ok: false, message: "invalid json response" }));

        return {
          ok: response.ok,
          status: response.status,
          body,
        };
      } catch (_error) {
        return {
          ok: false,
          status: 0,
          body: { ok: false, message: "network error" },
        };
      }
    },

    async uploadFile(file, metadata = {}) {
      const form = new FormData();
      if (metadata.user_id) {
        form.append("user_id", metadata.user_id);
      }
      if (metadata.file_id) {
        form.append("file_id", metadata.file_id);
      }
      form.append("file", file);

      try {
        const response = await fetch(`${this.apiBase}/api/files/upload`, {
          method: "POST",
          credentials: "same-origin",
          body: form,
        });

        const body = await response
          .json()
          .catch(() => ({ ok: false, message: "invalid json response" }));

        return { ok: response.ok, status: response.status, body };
      } catch (_error) {
        return {
          ok: false,
          status: 0,
          body: { ok: false, message: "network error" },
        };
      }
    },

    getSessionId() {
      return "";
    },

    setSessionId(_sessionId) {
      return;
    },

    clearSession() {
      return;
    },

    showNotice(targetId, message, type) {
      const target = document.getElementById(targetId);
      if (!target) return;
      target.className = `notice ${type || "success"}`;
      target.textContent = message;
      target.classList.remove("hidden");
    },

    hideNotice(targetId) {
      const target = document.getElementById(targetId);
      if (!target) return;
      target.classList.add("hidden");
    },

    toast(message) {
      const existing = document.querySelector(".toast");
      if (existing) {
        existing.remove();
      }

      const node = document.createElement("div");
      node.className = "toast";
      node.textContent = message;
      document.body.appendChild(node);

      window.setTimeout(() => {
        node.remove();
      }, 2600);
    },

    highlightMentions(text) {
      if (!text) return "";
      return text.replace(/(^|\s)@([a-z0-9_]{3,16})/gi, (all, prefix, username) => {
        return `${prefix}<span class="mention">@${username}</span>`;
      });
    },

    tabSwitch(groupSelector, buttonSelector, panelAttr) {
      const group = document.querySelector(groupSelector);
      if (!group) return;

      group.addEventListener("click", (event) => {
        const button = event.target.closest(buttonSelector);
        if (!button) return;

        const target = button.dataset.target;

        document.querySelectorAll(`${groupSelector} ${buttonSelector}`).forEach((btn) => {
          btn.classList.toggle("active", btn === button);
        });

        document.querySelectorAll(`[${panelAttr}]`).forEach((panel) => {
          panel.classList.toggle("hidden", panel.getAttribute(panelAttr) !== target);
        });
      });
    },

    onWebSocketEvent(type, handler) {
      if (!type || typeof handler !== "function") return;
      wsListeners.set(type, handler);
    },

    _dispatchWsEvent(event) {
      if (!event || !event.type) return;
      const handler = wsListeners.get(event.type);
      if (handler) {
        handler(event.payload);
      }
    },

    startPresenceSocket(elementId) {
      const statusNode = document.getElementById(elementId);
      if (!statusNode) return;

      const buildStatus = (text, state) => {
        if (!statusNode) return;
        statusNode.textContent = text;
        statusNode.classList.remove("presence-ok", "presence-warn", "presence-error");
        if (state) {
          statusNode.classList.add(`presence-${state}`);
        }
      };

      const clearTimers = () => {
        if (pingHandle) {
          clearInterval(pingHandle);
          pingHandle = null;
        }
        if (reconnectHandle) {
          clearTimeout(reconnectHandle);
          reconnectHandle = null;
        }
      };

      const sendPing = () => {
        if (wsSocket && wsSocket.readyState === WebSocket.OPEN) {
          wsSocket.send(JSON.stringify({ type: "ping", at: Date.now() }));
        }
      };

      const scheduleReconnect = () => {
        clearTimers();
        reconnectHandle = window.setTimeout(() => {
          connect();
        }, backoff);
        backoff = Math.min(30000, backoff * 1.5);
      };

      const connect = () => {
        clearTimers();
        const scheme = location.protocol === "https:" ? "wss" : "ws";
        wsSocket = new WebSocket(`${scheme}://${location.host}/ws`);
        buildStatus("WS bağlanıyor...", "warn");

        wsSocket.addEventListener("open", () => {
          buildStatus("WebSocket bağlantısı kuruldu", "ok");
          backoff = 1200;
          sendPing();
          pingHandle = window.setInterval(sendPing, 15000);
        });

        wsSocket.addEventListener("message", (event) => {
          try {
            const payload = JSON.parse(event.data);
            publicApi._dispatchWsEvent(payload);
          } catch (_error) {
            console.warn("ws payload parse failed");
          }
          sendPing();
        });

        wsSocket.addEventListener("error", () => {
          buildStatus("WebSocket hattında hata", "error");
        });

        wsSocket.addEventListener("close", () => {
          buildStatus("Bağlantı kesildi, yeniden deneniyor...", "warn");
          scheduleReconnect();
        });
      };

      connect();
    },
  };

  return publicApi;
})();

window.Melt = Melt;
