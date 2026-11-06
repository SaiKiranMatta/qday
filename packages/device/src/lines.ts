import { MAX_LINE } from "./protocol";

/** What a finished line was. */
export type Line = { kind: "text"; text: string } | { kind: "bad" };

/** Collects bytes into lines, refusing any that are too long or aren't UTF-8. */
export class LineBuffer {
  private bytes: number[] = [];
  private overflowed = false;
  private readonly decoder = new TextDecoder("utf-8", { fatal: true });

  /** Adds a chunk and returns every line it finished. */
  push(chunk: Uint8Array): Line[] {
    const lines: Line[] = [];
    for (const byte of chunk) {
      if (byte === 0x0a) {
        lines.push(this.finish());
        continue;
      }
      if (this.bytes.length + 1 < MAX_LINE) this.bytes.push(byte);
      else this.overflowed = true;
    }
    return lines;
  }

  private finish(): Line {
    const overflowed = this.overflowed;
    const bytes = Uint8Array.from(this.bytes);
    this.bytes = [];
    this.overflowed = false;
    if (overflowed) return { kind: "bad" };
    try {
      return { kind: "text", text: this.decoder.decode(bytes) };
    } catch {
      return { kind: "bad" };
    }
  }
}
