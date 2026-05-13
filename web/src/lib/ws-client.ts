"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import { useAuthStore } from "@/stores/auth.store";

const WS_URL = process.env.NEXT_PUBLIC_WS_URL ?? "ws://localhost:8080/ws";

const BACKOFF = [1_000, 2_000, 4_000, 8_000, 16_000] as const;

export type WsStatus = "connecting" | "connected" | "disconnected";

interface SubscribeMsg {
  type: "subscribe";
  channel: string;
}

interface UnsubscribeMsg {
  type: "unsubscribe";
  channel: string;
}

type OutboundMsg = SubscribeMsg | UnsubscribeMsg;
type ChannelSeq = Map<string, number>;

interface UseWebSocketReturn {
  status: WsStatus;
  subscribe: (channel: string, onMessage: (payload: unknown) => void) => () => void;
}

export function useWebSocket(): UseWebSocketReturn {
  const accessToken = useAuthStore((s) => s.accessToken);
  const [status, setStatus] = useState<WsStatus>("disconnected");

  const wsRef = useRef<WebSocket | null>(null);
  const retryRef = useRef(0);
  const retryTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const handlersRef = useRef<Map<string, Set<(payload: unknown) => void>>>(new Map());
  const seqRef = useRef<ChannelSeq>(new Map());
  const channelsRef = useRef<Set<string>>(new Set());

  const send = useCallback((msg: OutboundMsg) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(msg));
    }
  }, []);

  const connect = useCallback(() => {
    if (wsRef.current?.readyState === WebSocket.OPEN) return;

    const url = accessToken ? `${WS_URL}?token=${encodeURIComponent(accessToken)}` : WS_URL;
    const ws = new WebSocket(url);
    wsRef.current = ws;
    setStatus("connecting");

    ws.onopen = () => {
      setStatus("connected");
      retryRef.current = 0;
      // Re-subscribe all channels after reconnect.
      for (const channel of channelsRef.current) {
        ws.send(JSON.stringify({ type: "subscribe", channel } satisfies SubscribeMsg));
      }
    };

    ws.onmessage = (event: MessageEvent<string>) => {
      let parsed: unknown;
      try {
        parsed = JSON.parse(event.data) as unknown;
      } catch {
        return;
      }
      if (typeof parsed !== "object" || parsed === null) return;
      const msg = parsed as Record<string, unknown>;

      // Heartbeat — no dispatch needed.
      if (msg.type === "heartbeat") return;

      const channel = typeof msg.channel === "string" ? msg.channel : null;
      if (!channel) return;

      // Gap detection: if seq is non-zero and out of order, re-subscribe to force re-snapshot.
      const seq = typeof msg.seq === "number" ? msg.seq : null;
      if (seq !== null) {
        const lastSeq = seqRef.current.get(channel);
        if (lastSeq !== undefined && seq !== lastSeq + 1) {
          // Gap detected — drop cached seq, re-subscribe to trigger snapshot.
          seqRef.current.delete(channel);
          ws.send(JSON.stringify({ type: "unsubscribe", channel } satisfies UnsubscribeMsg));
          ws.send(JSON.stringify({ type: "subscribe", channel } satisfies SubscribeMsg));
          return;
        }
        seqRef.current.set(channel, seq);
      }

      const handlers = handlersRef.current.get(channel);
      if (!handlers) return;
      for (const handler of handlers) {
        handler(msg.payload ?? msg);
      }
    };

    ws.onclose = () => {
      setStatus("disconnected");
      wsRef.current = null;
      const delay = BACKOFF[Math.min(retryRef.current, BACKOFF.length - 1)] ?? 16_000;
      retryRef.current += 1;
      retryTimerRef.current = setTimeout(connect, delay);
    };

    ws.onerror = () => {
      ws.close();
    };
  }, [accessToken]);

  // Connect on mount; reconnect when token changes.
  useEffect(() => {
    connect();
    return () => {
      if (retryTimerRef.current) clearTimeout(retryTimerRef.current);
      wsRef.current?.close();
      wsRef.current = null;
    };
  }, [connect]);

  const subscribe = useCallback(
    (channel: string, onMessage: (payload: unknown) => void): (() => void) => {
      if (!handlersRef.current.has(channel)) {
        handlersRef.current.set(channel, new Set());
        channelsRef.current.add(channel);
        send({ type: "subscribe", channel });
      }
      // Entry was just created above if missing — guaranteed to exist.
      handlersRef.current.get(channel)?.add(onMessage);

      return () => {
        const handlers = handlersRef.current.get(channel);
        if (!handlers) return;
        handlers.delete(onMessage);
        if (handlers.size === 0) {
          handlersRef.current.delete(channel);
          channelsRef.current.delete(channel);
          seqRef.current.delete(channel);
          send({ type: "unsubscribe", channel });
        }
      };
    },
    [send],
  );

  return { status, subscribe };
}
