import { DeviceError, type DeviceStatus, QDayDevice, type StealthSpend } from "@qday/device";
import type { DemoRequest, DemoSigner } from "@qday/frame";
import { useCallback, useEffect, useRef, useState } from "react";
import { openDemo } from "./demo";
import { MOCK } from "./wallet";

export type ConnectionState =
  | { kind: "unsupported" }
  | { kind: "idle" }
  | { kind: "connecting" }
  | { kind: "connected"; status: DeviceStatus; demo: boolean }
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

const disconnected = (): ConnectionState => (QDayDevice.isSupported() ? { kind: "idle" } : { kind: "unsupported" });

/** A demo wallet's request, waiting for the person to decide. */
export interface PendingApproval {
  request: DemoRequest;
  decide: (approved: boolean) => void;
}

/** Owns the one device connection, or the demo wallet standing in for it, and its latest status. */
export function useDevice() {
  const [state, setState] = useState<ConnectionState>(() => {
    if (MOCK_STATUS) return { kind: "connected", status: MOCK_STATUS, demo: false };
    return disconnected();
  });
  const [log, setLog] = useState<string[]>([]);
  const device = useRef<QDayDevice | null>(null);
  const demo = useRef<DemoSigner | null>(null);
  const [approval, setApproval] = useState<PendingApproval | null>(null);

  const attach = useCallback(async (next: QDayDevice) => {
    if (device.current === next) {
      setState({ kind: "connected", status: await next.status(), demo: false });
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
    setState({ kind: "connected", status: await next.status(), demo: false });
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

  /** Opens the demo wallet in this browser, or the one `seed` belongs to. */
  const startDemo = useCallback(
    async (seed?: `0x${string}`) => {
      setState({ kind: "connecting" });
      const approve = (request: DemoRequest) =>
        new Promise<boolean>((resolve) => {
          setApproval({
            request,
            decide: (approved) => {
              setApproval(null);
              resolve(approved);
            },
          });
        });
      try {
        const signer = await openDemo(approve, seed);
        demo.current = signer;
        setState({ kind: "connected", status: await signer.status(), demo: true });
      } catch (error) {
        demo.current = null;
        fail(error);
      }
    },
    [fail],
  );

  const refresh = useCallback(async () => {
    const current = demo.current ?? device.current;
    if (!current) return;
    try {
      setState({ kind: "connected", status: await current.status(), demo: demo.current !== null });
    } catch (error) {
      fail(error);
    }
  }, [fail]);

  const disconnect = useCallback(async () => {
    if (demo.current) {
      demo.current.keys.free();
      demo.current = null;
      setState(disconnected());
      return;
    }
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
    const current = demo.current ?? device.current;
    if (!current) throw new DeviceError("Connect your QDay first");
    const signature = await current.signFrame(raw);
    // Hand the signature back at once and refresh the key state alongside, not before, the submission.
    void current
      .status()
      .then((status) => setState({ kind: "connected", status, demo: demo.current !== null }))
      .catch((error: unknown) => console.warn("[qday] couldn't refresh the device after signing", error));
    return signature;
  }, []);

  const connected = useCallback(() => {
    const current = demo.current ?? device.current;
    if (!current) throw new DeviceError("Connect your QDay first");
    return current;
  }, []);
  const stealthAddress = useCallback(() => connected().stealthAddress(), [connected]);
  const viewKey = useCallback(() => connected().viewKey(), [connected]);
  const spendStealth = useCallback((spend: StealthSpend) => connected().spendStealth(spend), [connected]);

  const exportDemoSeed = useCallback(() => demo.current?.exportSeed() ?? null, []);
  /** Deletes the demo wallet from this browser. */
  const forgetDemo = useCallback(() => {
    demo.current?.forget();
    demo.current = null;
    setState(disconnected());
  }, []);

  return {
    state,
    log,
    connect,
    startDemo,
    approval,
    exportDemoSeed,
    forgetDemo,
    refresh,
    disconnect,
    signFrame,
    stealthAddress,
    viewKey,
    spendStealth,
  };
}
