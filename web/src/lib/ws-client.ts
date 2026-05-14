"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { useAuthStore } from "@/stores/auth.store";

const WS_URL = process.env.NEXT_PUBLIC_WS_URL ?? "ws://localhost:8080/ws";

const BACKOFF = [1_000, 2_000, 4_000, 8_000, 16_000] as const;

export type WsStatus = "connecting" | "connected" | "disconnected";

interface SubscribeMsg {
  id: string;
  method: "subscribe";
  params: {
    channels: string[];
    afterSeq?: number;
  };
}

interface UnsubscribeMsg {
  id: string;
  method: "unsubscribe";
  params: {
    channels: string[];
  };
}

type OutboundMsg = SubscribeMsg | UnsubscribeMsg;
type ChannelSeq = Map<string, number>;

export interface UseWebSocketReturn {
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
  const requestIdRef = useRef(0);

  const nextRequestId = useCallback(() => {
    requestIdRef.current += 1;
    return `ws-${requestIdRef.current}`;
  }, []);

  const send = useCallback((msg: OutboundMsg) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(msg));
    }
  }, []);

  const sendSubscribe = useCallback(
    (channel: string, afterSeq?: number) => {
      const params: SubscribeMsg["params"] = { channels: [channel] };
      if (afterSeq !== undefined) params.afterSeq = afterSeq;
      send({ id: nextRequestId(), method: "subscribe", params });
    },
    [nextRequestId, send],
  );

  const sendUnsubscribe = useCallback(
    (channel: string) => {
      send({ id: nextRequestId(), method: "unsubscribe", params: { channels: [channel] } });
    },
    [nextRequestId, send],
  );

  const connect = useCallback(() => {
    if (wsRef.current?.readyState === WebSocket.OPEN) return;

    const url = accessToken ? `${WS_URL}?token=${encodeURIComponent(accessToken)}` : WS_URL;
    const ws = new WebSocket(url);
    wsRef.current = ws;
    setStatus("connecting");

    ws.onopen = () => {
      setStatus("connected");
      retryRef.current = 0;
      for (const channel of channelsRef.current) {
        const afterSeq = seqRef.current.get(channel);
        const params: SubscribeMsg["params"] = { channels: [channel] };
        if (afterSeq !== undefined) params.afterSeq = afterSeq;
        ws.send(JSON.stringify({ id: nextRequestId(), method: "subscribe", params }));
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

      const channel = typeof msg.channel === "string" ? msg.channel : null;
      if (!channel) return;
      const dispatchChannel = channel.endsWith(".snapshot")
        ? `${channel.slice(0, -".snapshot".length)}.diff`
        : channel;

      const seq = typeof msg.seq === "number" ? msg.seq : null;
      if (seq !== null) {
        const lastSeq = seqRef.current.get(dispatchChannel);
        if (lastSeq !== undefined && seq !== lastSeq + 1) {
          seqRef.current.delete(dispatchChannel);
          ws.send(
            JSON.stringify({
              id: nextRequestId(),
              method: "unsubscribe",
              params: { channels: [dispatchChannel] },
            } satisfies UnsubscribeMsg),
          );
          ws.send(
            JSON.stringify({
              id: nextRequestId(),
              method: "subscribe",
              params: { channels: [dispatchChannel], afterSeq: lastSeq },
            } satisfies SubscribeMsg),
          );
          return;
        }
        seqRef.current.set(dispatchChannel, seq);
      }

      const handlers = handlersRef.current.get(dispatchChannel);
      if (!handlers) return;
      for (const handler of handlers) {
        handler(msg.data ?? msg.payload ?? msg);
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
  }, [accessToken, nextRequestId]);

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
        sendSubscribe(channel);
      }
      handlersRef.current.get(channel)?.add(onMessage);

      return () => {
        const handlers = handlersRef.current.get(channel);
        if (!handlers) return;
        handlers.delete(onMessage);
        if (handlers.size === 0) {
          handlersRef.current.delete(channel);
          channelsRef.current.delete(channel);
          seqRef.current.delete(channel);
          sendUnsubscribe(channel);
        }
      };
    },
    [sendSubscribe, sendUnsubscribe],
  );

  return useMemo(() => ({ status, subscribe }), [status, subscribe]);
}
