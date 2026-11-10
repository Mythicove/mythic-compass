import { paintUnihexGPU } from "./minecraft-unifont-gpu";

// oxlint-disable-next-line typescript/consistent-type-definitions -- This public alias preserves implicit index-signature compatibility for glyph consumers.
export type UnihexGlyph = {
  code: number;
  rows: number[];
  bitWidth: number;
  left: number;
  width: number;
  advance: number;
};

let font: Map<number, UnihexGlyph> | undefined;
let pending: Promise<void> | undefined;

export const isCJKCodepoint = (code: number) =>
  (code >= 0x11_00 && code <= 0x11_ff) ||
  (code >= 0x2e_80 && code <= 0x9f_ff) ||
  (code >= 0xa9_60 && code <= 0xa9_7f) ||
  (code >= 0xac_00 && code <= 0xd7_ff) ||
  (code >= 0xf9_00 && code <= 0xfa_ff) ||
  (code >= 0xff_01 && code <= 0xff_ef) ||
  (code >= 0x1_af_f0 && code <= 0x1_b2_ff) ||
  (code >= 0x2_00_00 && code <= 0x3_23_af);

export const getUnihexGlyph = (char: string) => {
  const codepoint = char.codePointAt(0);

  return codepoint === undefined ? undefined : font?.get(codepoint);
};

// oxlint-disable-next-line typescript/promise-function-async -- Return the cached Unihex initialization Promise itself, including rejection; async would wrap it.
export const loadUnihex = function loadUnihex() {
  return (pending ??= (async () => {
    const { unifontZip, unifontOverrides } =
      await import("./minecraft-unifont-data");

    const bytes = Uint8Array.from(
      atob(unifontZip),
      (char) => char.codePointAt(0) ?? Number.NaN
    );

    const view = new DataView(bytes.buffer);
    let end = bytes.length - 22;

    while (end >= 0 && view.getUint32(end, true) !== 0x06_05_4b_50) {
      end -= 1;
    }

    if (end < 0) {
      throw new Error("Original Unihex ZIP directory missing");
    }

    let position = view.getUint32(end + 16, true);
    let contents = "";

    while (view.getUint32(position, true) === 0x02_01_4b_50) {
      const method = view.getUint16(position + 10, true);
      const length = view.getUint32(position + 20, true);

      const nameLength = view.getUint16(position + 28, true);
      const extra = view.getUint16(position + 30, true);
      const comment = view.getUint16(position + 32, true);

      const name = new TextDecoder().decode(
        bytes.subarray(position + 46, position + 46 + nameLength)
      );

      if (name.endsWith(".hex")) {
        if (method !== 8) {
          throw new Error("Unexpected Unihex ZIP compression");
        }

        const local = view.getUint32(position + 42, true);

        const start =
          local +
          30 +
          view.getUint16(local + 26, true) +
          view.getUint16(local + 28, true);

        // oxlint-disable-next-line eslint/no-await-in-loop -- ZIP members append in directory order, preserving duplicate-glyph precedence and the first decompression/next-header failure; parallel decoding changes that protocol.
        contents += await new Response(
          new Blob([bytes.slice(start, start + length)])
            .stream()
            .pipeThrough(new DecompressionStream("deflate-raw"))
        ).text();
      }

      position += 46 + nameLength + extra + comment;
    }

    font = new Map();

    for (const line of contents.split("\n")) {
      const [raw, hex] = line.trim().split(":");

      if (!hex) {
        continue;
      }

      const code = Number.parseInt(raw, 16);

      if (!isCJKCodepoint(code)) {
        continue;
      }

      const bitWidth = hex.length / 4;
      const rowLength = hex.length / 16;

      const rows = Array.from({ length: 16 }, (_, row) =>
        Number.parseInt(hex.slice(row * rowLength, (row + 1) * rowLength), 16)
      );

      let left = bitWidth;
      let right = -1;

      for (let x = 0; x < bitWidth; x += 1) {
        // oxlint-disable-next-line no-bitwise -- Locate occupied Unihex columns by testing their encoded bit across all glyph rows.
        if (rows.some((row) => (row >>> (bitWidth - 1 - x)) & 1)) {
          left = Math.min(left, x);
          right = Math.max(right, x);
        }
      }

      if (right < 0) {
        left = 0;
        right = bitWidth;
      }

      const fixed = unifontOverrides.find(
        (range) => code >= range[0] && code <= range[1]
      );

      if (fixed) {
        ({ 2: left, 3: right } = fixed);
      }

      const width = right - left + 1;
      font.set(code, {
        advance: width / 2 + 1,
        bitWidth,
        code,
        left,
        rows,
        width,
      });
    }
  })());
};

export const paintUnihex = function paintUnihex(
  context: CanvasRenderingContext2D,
  glyph: UnihexGlyph,
  x: number,
  y: number,
  scale: number
) {
  if (paintUnihexGPU(context, glyph, x, y, scale)) {
    return;
  }

  const left = x * scale;
  const top = y * scale;
  const width = (glyph.width / 2) * scale;
  const height = 8 * scale;

  for (
    let py = Math.floor(top + 0.5);
    py < Math.floor(top + height + 0.5);
    py += 1
  ) {
    const sy = Math.max(
      0,
      Math.min(
        15,
        Math.floor(0.01 + ((py + 0.5 - top) / height) * 15.98 - 1e-6)
      )
    );

    for (
      let px = Math.ceil(left - 0.5);
      px < Math.ceil(left + width - 0.5);
      px += 1
    ) {
      const sx =
        Math.floor(
          0.01 + ((px + 0.5 - left) / width) * (glyph.width - 0.02) - 1e-6
        ) + glyph.left;

      if (
        sx >= 0 &&
        sx < glyph.bitWidth &&
        // oxlint-disable-next-line no-bitwise -- Unihex rows store one bit per glyph column; extract the selected column bit.
        (glyph.rows[sy] >>> (glyph.bitWidth - 1 - sx)) & 1
      ) {
        context.fillRect(px, py, 1, 1);
      }
    }
  }
};
