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

/** Set by Disconnect so a reload stays disconnected until the user connects again. */
const STAY_DISCONNECTED = "qday.stayDisconnected";

function stayDisconnected(value?: boolean): boolean {
  try {
    if (value === true) localStorage.setItem(STAY_DISCONNECTED, "1");
    if (value === false) localStorage.removeItem(STAY_DISCONNECTED);
    return localStorage.getItem(STAY_DISCONNECTED) === "1";
  } catch {
    return false; // Storage blocked: forgetting the port is enough on its own.
  }
}

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
        // Everything on screen that needs the device goes with it, open dialogs included.
        console.warn("[qday] the device connection closed");
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
    stayDisconnected(false);
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
    stayDisconnected(true);
    await device.current?.forget();
    device.current = null;
    setState({ kind: "idle" });
  }, []);

  // Reconnect quietly on reload if the user already allowed one device and didn't disconnect it.
  useEffect(() => {
    if (MOCK_STATUS || !QDayDevice.isSupported() || stayDisconnected()) return;
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

  /** Has the device review and sign an unsigned transaction, then re-reads its key state. */
  const signFrame = useCallback(async (raw: Uint8Array) => {
    const current = device.current;
    if (!current) throw new DeviceError("Connect your QDay first");
    const signature = await current.signFrame(raw);
    setState({ kind: "connected", status: await current.status() });
    return signature;
  }, []);

  return { state, log, connect, refresh, disconnect, signFrame };
}
