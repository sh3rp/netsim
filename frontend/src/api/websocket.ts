import type { TickUpdate } from "../types/models.ts";

type TickHandler = (update: TickUpdate) => void;

let ws: WebSocket | null = null;
let handlers: TickHandler[] = [];
let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

export function connect() {
  if (ws && ws.readyState === WebSocket.OPEN) return;

  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  const url = `${protocol}//${window.location.host}/ws`;
  ws = new WebSocket(url);

  ws.onmessage = (event) => {
    try {
      const update: TickUpdate = JSON.parse(event.data);
      handlers.forEach((h) => h(update));
    } catch {
      // ignore parse errors
    }
  };

  ws.onclose = () => {
    ws = null;
    // Auto-reconnect after 2s
    if (!reconnectTimer) {
      reconnectTimer = setTimeout(() => {
        reconnectTimer = null;
        connect();
      }, 2000);
    }
  };

  ws.onerror = () => {
    ws?.close();
  };
}

export function onTick(handler: TickHandler): () => void {
  handlers.push(handler);
  return () => {
    handlers = handlers.filter((h) => h !== handler);
  };
}

export function disconnect() {
  if (reconnectTimer) {
    clearTimeout(reconnectTimer);
    reconnectTimer = null;
  }
  ws?.close();
  ws = null;
}
