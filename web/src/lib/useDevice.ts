import { DeviceError, type DeviceStatus, QDayDevice } from "@qday/device";
import { useCallback, useEffect, useRef, useState } from "react";
import { MOCK } from "./wallet";

export type ConnectionState =
  | { kind: "unsupported" }
  | { kind: "idle" }
  | { kind: "connecting" }
  | { kind: "connected"; status: DeviceStatus }
  | { kind: "error"; message: string };

const LOG_LIMIT = 200;

/** Dev only: `?mock` shows a sample device so the layout can be reviewed without hardware. */
const MOCK_STATUS: DeviceStatus | null =
  MOCK
    ? {
        scheme: "xmss-keccak",
        root: `0xbfefd6eccd30${"7a".repeat(24)}b7f3`,
        pubSeed: `0x5c1e${"3d".repeat(28)}9a20`,
        nextIndex: 961,
        tree: 0,
        next: null,
      }
    : null;

/** Owns the one device connection and its latest status. */
export function useDevice() {
  const [state, setState] = useState<ConnectionState>(() => {
    if (MOCK_STATUS) return { kind: "connected", status: MOCK_STATUS };
    return QDayDevice.isSupported() ? { kind: "idle" } : { kind: "unsupported" };
  });
  const [log, setLog] = useState<string[]>([]);
  const device = useRef<QDayDevice | null>(null);

  const attach = useCallback(async (next: QDayDevice) => {
    if (device.current === next) {
      setState({ kind: "connected", status: await next.status() });
      return;
    }
    device.current = next;
    next.onLog((line) => setLog((lines) => [...lines.slice(-(LOG_LIMIT - 1)), line]));
    next.onClose(() => {
      if (device.current === next) {
        device.current = null;
        setState({ kind: "idle" });
      }
    });
    setState({ kind: "connected", status: await next.status() });
  }, []);

  const fail = useCallback((error: unknown) => {
    if (error instanceof DOMException && error.name === "NotFoundError") {
      setState({ kind: "idle" }); // The user closed the picker.
      return;
    }
    const message = error instanceof DeviceError || error instanceof Error ? error.message : String(error);
    setState({ kind: "error", message });
  }, []);

  const connect = useCallback(async () => {
    setState({ kind: "connecting" });
    try {
      await attach(await QDayDevice.request());
    } catch (error) {
      await device.current?.close();
      fail(error);
    }
  }, [attach, fail]);

  const refresh = useCallback(async () => {
    const current = device.current;
    if (!current) return;
    try {
      setState({ kind: "connected", status: await current.status() });
    } catch (error) {
      fail(error);
    }
  }, [fail]);

  const disconnect = useCallback(async () => {
    await device.current?.close();
    device.current = null;
    setState({ kind: "idle" });
  }, []);

  // Reconnect quietly on reload if the user already allowed one device.
  useEffect(() => {
    if (MOCK_STATUS || !QDayDevice.isSupported()) return;
    let cancelled = false;
    void QDayDevice.reconnect()
      .then(async (found) => {
        // A cancelled run leaves the shared connection open for the run that replaced it.
        if (!found || cancelled) return;
        setState({ kind: "connecting" });
        await attach(found);
      })
      .catch((error: unknown) => {
        if (!cancelled) fail(error);
      });
    return () => {
      cancelled = true;
    };
  }, [attach, fail]);

  return { state, log, connect, refresh, disconnect };
}
