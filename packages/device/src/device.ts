import { LineBuffer } from "./lines";
import {
  type ErrorCode,
  type NextKey,
  type Request,
  type Response,
  isProtocolLine,
  parseResponse,
  requestLine,
} from "./protocol";

const BAUD = 115_200;
/** Opening the port can reset the board, so the first request gets this many tries. */
const STATUS_TRIES = 5;
const STATUS_WAIT_MS = 1_000;
/** Signing waits for a person, and tree building takes minutes. */
export const LONG_WAIT_MS = 300_000;

/** The device's keys, from a PUBKEY response. */
export interface DeviceStatus {
  /** Signature scheme the keys belong to. The current firmware only has one. */
  scheme: "xmss-keccak";
  root: `0x${string}`;
  pubSeed: `0x${string}`;
  nextIndex: number;
  tree: number;
  next: NextKey | null;
}

export class DeviceError extends Error {
  constructor(
    message: string,
    readonly code?: ErrorCode,
  ) {
    super(message);
    this.name = "DeviceError";
  }
}

type Pending = { resolve: (response: Response | null) => void; timer: ReturnType<typeof setTimeout> };

/** One connection per port: concurrent opens (a click during a reconnect, React running effects twice) share it. */
const connections = new Map<SerialPort, Promise<QDayDevice>>();

/** A QDay device on a Web Serial port. One request at a time, like the protocol. */
export class QDayDevice {
  private readonly lines = new LineBuffer();
  private readonly encoder = new TextEncoder();
  private pending: Pending | null = null;
  private busy: Promise<unknown> = Promise.resolve();
  private reader: ReadableStreamDefaultReader<Uint8Array> | null = null;
  /** The read loop. It holds the readable stream's lock until it ends. */
  private reading: Promise<void> = Promise.resolve();
  private closed = false;
  private readonly logListeners = new Set<(line: string) => void>();
  private readonly closeListeners = new Set<() => void>();

  private constructor(readonly port: SerialPort) {}

  /** True when this browser can talk to a device (desktop Chromium, secure context). */
  static isSupported(): boolean {
    return typeof navigator !== "undefined" && "serial" in navigator;
  }

  /** Asks the user to pick the device, then opens it. Must run from a click. */
  static async request(): Promise<QDayDevice> {
    if (!QDayDevice.isSupported()) throw new DeviceError("This browser can't reach USB devices. Use Chrome, Edge or Brave on desktop.");
    const port = await navigator.serial.requestPort();
    return QDayDevice.open(port);
  }

  /** Reopens a port the user already allowed, if there is exactly one. */
  static async reconnect(): Promise<QDayDevice | null> {
    if (!QDayDevice.isSupported()) return null;
    const ports = await navigator.serial.getPorts();
    return ports.length === 1 && ports[0] ? QDayDevice.open(ports[0]) : null;
  }

  static open(port: SerialPort): Promise<QDayDevice> {
    const existing = connections.get(port);
    if (existing) return existing;
    const opening = (async () => {
      await port.open({ baudRate: BAUD });
      const device = new QDayDevice(port);
      device.reading = device.readLoop();
      return device;
    })();
    connections.set(port, opening);
    opening.catch(() => connections.delete(port));
    return opening;
  }

  /** Lines the device prints that aren't protocol responses. */
  onLog(listener: (line: string) => void): () => void {
    this.logListeners.add(listener);
    return () => this.logListeners.delete(listener);
  }

  /** Called once when the port goes away, for example when the cable is pulled. */
  onClose(listener: () => void): () => void {
    this.closeListeners.add(listener);
    return () => this.closeListeners.delete(listener);
  }

  /** Asks for the keys, retrying because opening the port can reset the board. */
  async status(): Promise<DeviceStatus> {
    for (let attempt = 0; attempt < STATUS_TRIES; attempt += 1) {
      const response = await this.exchange({ kind: "pubkey" }, STATUS_WAIT_MS);
      if (response === null) continue;
      if (response.kind === "err") throw new DeviceError(`The device said ${response.code}`, response.code);
      if (response.kind !== "pubkey") throw new DeviceError("The device gave the wrong response");
      return {
        scheme: "xmss-keccak",
        root: response.root,
        pubSeed: response.pubSeed,
        nextIndex: response.nextIndex,
        tree: response.tree,
        next: response.next,
      };
    }
    throw new DeviceError(`The device didn't answer after ${STATUS_TRIES} tries`);
  }

  /**
   * Asks the device to review and sign an unsigned frame transaction. Waits for the person to approve,
   * up to LONG_WAIT_MS. Returns the 2,500-byte signature as 0x hex.
   */
  async signFrame(raw: Uint8Array): Promise<`0x${string}`> {
    const response = await this.request({ kind: "signFrame", raw }, LONG_WAIT_MS);
    if (response.kind !== "sig") throw new DeviceError("The device gave the wrong response");
    return response.signature;
  }

  /** Sends one request and waits for its response, failing on a device error. */
  async request(request: Request, timeoutMs = LONG_WAIT_MS): Promise<Response> {
    const response = await this.exchange(request, timeoutMs);
    if (response === null) throw new DeviceError("The device didn't answer");
    if (response.kind === "err") throw new DeviceError(`The device said ${response.code}`, response.code);
    return response;
  }

  async close(): Promise<void> {
    if (this.closed) return;
    this.closed = true;
    connections.delete(this.port);
    this.settle(null);
    try {
      await this.reader?.cancel();
    } catch {
      // The port may already be gone.
    }
    // A port with a locked stream refuses to close and stays open, so let the read loop let go first.
    await this.reading;
    try {
      await this.port.close();
    } catch (error) {
      console.warn("[qday] closing the port failed", error);
    }
    this.closeListeners.forEach((listener) => listener());
  }

  /** Queues requests so only one is ever on the wire. */
  private exchange(request: Request, timeoutMs: number): Promise<Response | null> {
    const run = async (): Promise<Response | null> => {
      if (this.closed) throw new DeviceError("The device is disconnected");
      const answer = new Promise<Response | null>((resolve) => {
        const timer = setTimeout(() => this.settle(null), timeoutMs);
        this.pending = { resolve, timer };
      });
      await this.write(requestLine(request));
      return answer;
    };
    const next = this.busy.then(run, run);
    this.busy = next.catch(() => undefined);
    return next;
  }

  private async write(text: string): Promise<void> {
    const writable = this.port.writable;
    if (!writable) throw new DeviceError("The device is disconnected");
    const writer = writable.getWriter();
    try {
      await writer.write(this.encoder.encode(text));
    } finally {
      writer.releaseLock();
    }
  }

  private settle(response: Response | null): void {
    const pending = this.pending;
    if (!pending) return;
    this.pending = null;
    clearTimeout(pending.timer);
    pending.resolve(response);
  }

  private async readLoop(): Promise<void> {
    try {
      while (!this.closed && this.port.readable) {
        this.reader = this.port.readable.getReader();
        try {
          for (;;) {
            const { value, done } = await this.reader.read();
            if (done) break;
            for (const line of this.lines.push(value)) {
              if (line.kind === "bad") continue;
              this.handleLine(line.text.replace(/\r$/, ""));
            }
          }
        } finally {
          this.reader.releaseLock();
          this.reader = null;
        }
      }
    } catch {
      // A read error means the device went away.
    }
    void this.close();
  }

  private handleLine(text: string): void {
    if (!isProtocolLine(text)) {
      this.logListeners.forEach((listener) => listener(text));
      return;
    }
    // Late answers to a timed-out request have nobody waiting, so they're dropped.
    this.settle(parseResponse(text));
  }
}
