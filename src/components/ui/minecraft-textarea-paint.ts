import { minecraftGlyphs } from "./minecraft-font";
import { minecraftInputAssets } from "./minecraft-input-assets";
import { minecraftTextareaAssets } from "./minecraft-textarea-assets";
import {
  getUnihexGlyph,
  isCJKCodepoint,
  loadUnihex,
  paintUnihex,
} from "./minecraft-unifont";
import { disposeUnihex } from "./minecraft-unifont-gpu";

const requiresCJKFont = function requiresCJKFont(text: string) {
  for (const character of text) {
    if (
      isCJKCodepoint(
        // SAFETY: String iteration supplies a nonempty character, so its first code point exists.
        // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- The intrinsic string iterator proves this specific codePointAt(0) result.
        character.codePointAt(0) as number
      )
    ) {
      return true;
    }
  }

  return false;
};

const supportsInputGlyphs = function supportsInputGlyphs(text: string) {
  for (const character of text) {
    if (
      !(
        /^[\u0020-\u007E\n]$/u.test(character) ||
        getUnihexGlyph(character) !== undefined
      )
    ) {
      return false;
    }
  }

  return true;
};

const asciiGlyphs: readonly ((typeof minecraftGlyphs)[number] | undefined)[] =
  minecraftGlyphs;

interface Line {
  begin: number;
  end: number;
}

const glyphAdvance = (char: string) =>
  asciiGlyphs[
    // SAFETY: Private glyph callers supply nonempty iteration characters or guarded in-bounds code units. Its first code point therefore exists.
    // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- The verified glyph-character producer proves this specific codePointAt(0) result.
    (char.codePointAt(0) as number) - 32
  ]?.[2] ??
  getUnihexGlyph(char)?.advance ??
  6;

const advance = (text: string) => {
  let measuredGlyphWidth = 0;

  for (const glyphCharacter of text) {
    measuredGlyphWidth += glyphAdvance(glyphCharacter);
  }

  return Math.ceil(measuredGlyphWidth);
};

const wrap = function wrap(value: string, width: number): Line[] {
  const lines: Line[] = [];
  let start = 0;

  for (const paragraph of value.split("\n")) {
    let begin = start;
    let remaining = paragraph;

    while (remaining.length && advance(remaining) > width) {
      let count = 0;
      let used = 0;
      let space = -1;

      while (count < remaining.length) {
        if (remaining[count] === " ") {
          space = count;
        }

        const char = String.fromCodePoint(
          // SAFETY: This loop calls codePointAt only with count below the current string length; count starts at zero and advances by each nonempty character length.
          // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- The existing in-bounds string traversal proves this exact codePointAt result.
          remaining.codePointAt(count) as number
        );

        const next = glyphAdvance(char);

        if (used + next > width && count > 0) {
          break;
        }

        used += next;
        count += char.length;
      }

      const length = space >= 0 ? space : Math.max(1, count);
      lines.push({ begin, end: begin + length });
      const consumed = length + Number(space >= 0);
      begin += consumed;
      remaining = remaining.slice(consumed);
    }

    lines.push({ begin, end: begin + remaining.length });
    start += paragraph.length + 1;
  }

  return lines;
};

let images:
  | Promise<{
      normal: HTMLImageElement;
      highlighted: HTMLImageElement;
      atlas: HTMLImageElement;
      scroller: HTMLImageElement;
      track: HTMLImageElement;
      alpha: Uint8ClampedArray;
    }>
  | undefined;

// oxlint-disable-next-line typescript/promise-function-async -- Return the cached image Promise itself, including rejection; async would wrap its identity.
const loadImages = function loadImages() {
  return (images ??= Promise.all(
    [
      minecraftInputAssets.normal,
      minecraftInputAssets.highlighted,
      minecraftInputAssets.atlas,
      minecraftTextareaAssets.scroller,
      minecraftTextareaAssets.scroller_background,
    ].map(
      // oxlint-disable-next-line typescript/promise-function-async -- C15b: return this synchronous image-load/error adapter Promise directly; async would add adoption jobs before Promise.all readiness.
      (src) =>
        // oxlint-disable-next-line promise/avoid-new -- Preserve the cached texture-load event bridge and raw load-error event.
        new Promise<HTMLImageElement>((resolve, reject) => {
          const image = new Image();
          image.addEventListener("load", () => {
            resolve(image);
          });

          image.addEventListener("error", reject);
          image.src = src;
        })
    )
  ).then(([normal, highlighted, atlas, scroller, track]) => {
    const canvas = document.createElement("canvas");
    canvas.height = 128;
    canvas.width = 128;
    const context = canvas.getContext("2d");

    if (!context) {
      throw new Error(
        "Minecraft textarea paint 2D canvas context could not be created"
      );
    }

    context.drawImage(atlas, 0, 0);

    return {
      alpha: context.getImageData(0, 0, 128, 128).data,
      atlas,
      highlighted,
      normal,
      scroller,
      track,
    };
  }));
};

const sprite = function sprite(
  context: CanvasRenderingContext2D,
  image: HTMLImageElement,
  dx: number,
  dy: number,
  width: number,
  height: number,
  scale: number
) {
  const widths = [1, image.width - 2, 1];
  const heights = [1, image.height - 2, 1];
  const dw = [scale, width - 2 * scale, scale];
  const dh = [scale, height - 2 * scale, scale];

  for (
    let row = 0, sy = 0, y = dy;
    row < 3;
    sy += heights[row], y += dh[row], row += 1
  ) {
    for (
      let col = 0, sx = 0, x = dx;
      col < 3;
      sx += widths[col], x += dw[col], col += 1
    ) {
      for (let oy = 0; oy < dh[row]; oy += heights[row] * scale) {
        for (let ox = 0; ox < dw[col]; ox += widths[col] * scale) {
          const w = Math.min(widths[col] * scale, dw[col] - ox);
          const h = Math.min(heights[row] * scale, dh[row] - oy);

          context.drawImage(
            image,
            sx,
            sy,
            w / scale,
            h / scale,
            x + ox,
            y + oy,
            w,
            h
          );
        }
      }
    }
  }
};

const text = function text(
  context: CanvasRenderingContext2D,
  alpha: Uint8ClampedArray,
  value: string,
  x: number,
  y: number,
  ink: number,
  scale: number
) {
  for (const shadow of [true, false]) {
    const color = shadow ? Math.floor(ink / 4) : ink;
    context.fillStyle = `rgb(${color},${color},${color})`;
    let offset = x;

    for (const char of value) {
      const glyph =
        asciiGlyphs[
          // SAFETY: String iteration supplies a nonempty character, so its first code point exists.
          // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- The intrinsic string iterator proves this specific codePointAt(0) result.
          (char.codePointAt(0) as number) - 32
        ];

      if (!glyph) {
        const unihex = getUnihexGlyph(char);

        if (unihex) {
          paintUnihex(
            context,
            unihex,
            offset + (shadow ? 0.5 : 0),
            y + (shadow ? 0.5 : 0),
            scale
          );
        }

        offset += glyphAdvance(char);
        continue;
      }

      if (char !== " ") {
        for (let py = 0; py < 8; py += 1) {
          for (let px = 0; px < 8; px += 1) {
            if (alpha[((glyph[1] + py) * 128 + glyph[0] + px) * 4 + 3]) {
              context.fillRect(
                (offset + px + Number(shadow)) * scale,
                (y + py + Number(shadow)) * scale,
                scale,
                scale
              );
            }
          }
        }
      }

      offset += glyph[2];
    }
  }
};

export const attachMinecraftTextarea = function attachMinecraftTextarea(
  node: HTMLTextAreaElement,
  initialScale: number
) {
  const canvas = document.createElement("canvas");
  const context = canvas.getContext("2d", { willReadFrequently: true });

  if (!context) {
    throw new Error(
      "Minecraft textarea paint 2D canvas context could not be created"
    );
  }

  let cancelled = false;
  let frame = 0;
  let loaded: Awaited<ReturnType<typeof loadImages>> | undefined;
  let signature = "";
  let focusedAt = Date.now();

  let loadingUnihex = false;

  let geometry = {
    height: 0,
    maximum: 0,
    pane: 0,
    scale: initialScale,
    thumb: 32,
  };

  let drag: { y: number; scroll: number } | undefined;

  const paint = () => {
    if (!loaded || cancelled) {
      return;
    }

    const style = getComputedStyle(node);
    const scale = Number(style.getPropertyValue("--mc-scale")) || initialScale;

    const width = node.clientWidth;
    const height = node.clientHeight;
    const { value } = node;
    const focused = document.activeElement === node;

    if (!width || !height) {
      return;
    }

    const bounds = node.getBoundingClientRect();
    const inputGlyphText = value + node.placeholder;

    if (!loadingUnihex && requiresCJKFont(inputGlyphText)) {
      loadingUnihex = true;
      const fontLoading = loadUnihex();

      const repaintAfterFontLoading = async () => {
        await fontLoading;

        signature = "";
        paint();
      };

      repaintAfterFontLoading().catch(() => {
        /* empty */
      });
    }

    let pane = width / scale;
    let lines = wrap(value, Math.max(1, pane - 8));

    const scrollable = lines.length * 9 + 8 > height / scale;

    if (scrollable) {
      pane -= 6;
      lines = wrap(value, Math.max(1, pane - 8));
    }

    node.dataset.mcScrollbar = String(scrollable);

    const supported =
      supportsInputGlyphs(inputGlyphText) && style.direction !== "rtl";

    node.dataset.mcBitmap = String(supported);

    const cursor =
      node.selectionDirection === "backward"
        ? node.selectionStart
        : node.selectionEnd;

    const anchor =
      node.selectionDirection === "backward"
        ? node.selectionEnd
        : node.selectionStart;

    const blink =
      focused && Math.floor((Date.now() - focusedAt) / 300) % 2 === 0;

    const maximum = Math.max(0, lines.length * 9 + 8 - height / scale);
    const scroll = Math.max(0, Math.min(maximum, node.scrollTop / scale));

    const thumb = Math.max(
      32,
      Math.min(
        height / scale - 8,
        Math.floor((height / scale) ** 2 / (lines.length * 9 + 8))
      )
    );

    geometry = { height: height / scale, maximum, pane, scale, thumb };

    const next = JSON.stringify([
      width,
      height,
      scale,
      value,
      cursor,
      anchor,
      focused,
      blink,
      scroll,
      node.disabled,
      node.readOnly,
      node.placeholder,
      supported,
      bounds.x,
      bounds.y,
      innerWidth,
      innerHeight,
    ]);

    if (next === signature) {
      return;
    }

    signature = next;
    canvas.width = width;
    canvas.height = height;
    context.imageSmoothingEnabled = false;
    sprite(
      context,
      focused && !node.disabled ? loaded.highlighted : loaded.normal,
      0,
      0,
      pane * scale,
      height,
      scale
    );

    if (supported) {
      const paintEditableText = (
        frameImages: Awaited<ReturnType<typeof loadImages>>
      ) => {
        context.save();
        context.beginPath();
        context.rect(scale, scale, (pane - 2) * scale, height - 2 * scale);
        context.clip();

        if (!value && !focused && node.placeholder) {
          context.globalAlpha = 0.8;

          for (const [row, line] of wrap(
            node.placeholder,
            pane - 8
          ).entries()) {
            text(
              context,
              frameImages.alpha,
              node.placeholder.slice(line.begin, line.end),
              4,
              4 + row * 9 - scroll,
              224,
              scale
            );
          }

          context.globalAlpha = 1;
        } else {
          const row = Math.max(
            0,
            lines.findIndex(
              (line) => cursor >= line.begin && cursor <= line.end
            )
          );

          const line = lines[row];

          const x = 4 + advance(value.slice(line.begin, cursor));
          const y = 4 + row * 9 - scroll;

          for (const [lineIndex, current] of lines.entries()) {
            if (blink && cursor < value.length && lineIndex === row) {
              text(
                context,
                frameImages.alpha,
                value.slice(current.begin, cursor),
                4,
                y,
                224,
                scale
              );
              text(
                context,
                frameImages.alpha,
                value.slice(cursor, current.end),
                x,
                y,
                224,
                scale
              );
              context.fillStyle = "#d0d0d0";
              context.fillRect(x * scale, (y - 1) * scale, scale, 11 * scale);
            } else {
              text(
                context,
                frameImages.alpha,
                value.slice(current.begin, current.end),
                4,
                4 + lineIndex * 9 - scroll,
                224,
                scale
              );
            }
          }

          if (blink && cursor >= value.length) {
            text(context, frameImages.alpha, "_", x, y, 208, scale);
          }

          if (cursor !== anchor) {
            for (const [selectionRowIndex, selectionLine] of lines.entries()) {
              const begin = Math.max(
                selectionLine.begin,
                Math.min(cursor, anchor)
              );

              const end = Math.min(selectionLine.end, Math.max(cursor, anchor));

              if (end < begin) {
                continue;
              }

              const left = 4 + advance(value.slice(selectionLine.begin, begin));

              const right =
                Math.max(cursor, anchor) > selectionLine.end
                  ? pane
                  : 4 + advance(value.slice(selectionLine.begin, end));

              const top = 4 + selectionRowIndex * 9 - scroll;

              const x0 = Math.max(1, left);
              const x1 = Math.min(pane - 1, right);
              const y0 = Math.max(1, top);
              const y1 = Math.min(height / scale - 1, top + 9);

              if (x1 > x0 && y1 > y0) {
                const pixels = context.getImageData(
                  x0 * scale,
                  y0 * scale,
                  (x1 - x0) * scale,
                  (y1 - y0) * scale
                );

                for (let i = 0; i < pixels.data.length; i += 4) {
                  pixels.data[i] = 255 - pixels.data[i];
                  pixels.data[i + 1] = 255 - pixels.data[i + 1];
                  pixels.data[i + 2] = 255;
                }

                context.putImageData(pixels, x0 * scale, y0 * scale);
              }
            }
          }
        }

        context.restore();
      };

      paintEditableText(loaded);
    }

    if (scrollable) {
      sprite(context, loaded.track, pane * scale, 0, 6 * scale, height, scale);
      const y = Math.floor((scroll * (height / scale - thumb)) / maximum);
      sprite(
        context,
        loaded.scroller,
        pane * scale,
        y * scale,
        6 * scale,
        thumb * scale,
        scale
      );
    }

    node.style.setProperty(
      "--mc-textarea-image",
      `url("${canvas.toDataURL()}")`
    );
    node.dataset.mcReady = "true";
  };

  const tick = () => {
    paint();

    if (!cancelled) {
      frame = requestAnimationFrame(tick);
    }
  };

  const focus = () => {
    focusedAt = Date.now();
    signature = "";
    paint();
  };

  const down = (event: PointerEvent) => {
    if (
      event.defaultPrevented ||
      node.disabled ||
      event.button !== 0 ||
      !geometry.maximum
    ) {
      return;
    }

    if (
      event.clientX - node.getBoundingClientRect().left <
      geometry.pane * geometry.scale
    ) {
      return;
    }

    event.preventDefault();
    drag = { scroll: node.scrollTop, y: event.clientY };
    node.setPointerCapture(event.pointerId);
  };

  const move = (event: PointerEvent) => {
    if (drag) {
      node.scrollTop =
        drag.scroll +
        ((event.clientY - drag.y) * geometry.maximum) /
          (geometry.height - geometry.thumb);
      paint();
    }
  };

  const up = () => {
    drag = undefined;
  };

  node.addEventListener("focus", focus);
  node.addEventListener("pointerdown", down);
  node.addEventListener("pointermove", move);
  node.addEventListener("pointerup", up);
  node.addEventListener("pointercancel", up);
  const imagesLoading = loadImages();

  const startPaintingAfterImages = async () => {
    const result = await imagesLoading;

    if (!cancelled) {
      loaded = result;
      tick();
    }
  };

  startPaintingAfterImages().catch(() => {
    node.dataset.mcBitmap = "false";
  });

  return () => {
    cancelled = true;
    cancelAnimationFrame(frame);
    disposeUnihex(context);
    node.removeEventListener("focus", focus);
    node.removeEventListener("pointerdown", down);
    node.removeEventListener("pointermove", move);
    node.removeEventListener("pointerup", up);
    node.removeEventListener("pointercancel", up);
  };
};
