"use client";

import * as React from "react";

import { minecraftGlyphs } from "./minecraft-font";
import { minecraftInputAssets } from "./minecraft-input-assets";
import { useCanvasPaintConnection } from "./minecraft-render";
import { getUnihexGlyph, loadUnihex, paintUnihex } from "./minecraft-unifont";
import type { UnihexGlyph } from "./minecraft-unifont";
import {
  positionUnihex,
  disposeUnihex,
  setUnihexAtlas,
} from "./minecraft-unifont-gpu";
import type { UnihexAtlas } from "./minecraft-unifont-gpu";

import "./minecraft-unicode.css";

const hasSupportedLineGlyphs = function hasSupportedLineGlyphs(text: string) {
  for (const character of text) {
    if (
      !(
        /^[\u0020-\u007E]$/u.test(character) ||
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

interface UnicodeLabelStyle extends React.CSSProperties {
  "--mc-unicode-width": string;
}

let alpha: Promise<Uint8ClampedArray> | undefined;

// oxlint-disable-next-line typescript/promise-function-async -- Return the cached image Promise itself, including rejection; async would wrap its identity.
const loadAlpha = function loadAlpha() {
  // oxlint-disable-next-line promise/avoid-new -- Preserve the cached Image load/error bridge and its onload alpha-extraction phase.
  return (alpha ??= new Promise((resolve, reject) => {
    const image = new Image();
    image.addEventListener("load", () => {
      const canvas = document.createElement("canvas");
      canvas.height = 128;
      canvas.width = 128;
      const context = canvas.getContext("2d");

      if (!context) {
        reject(
          new Error(
            "Minecraft unicode label 2D canvas context could not be created"
          )
        );

        return;
      }

      context.drawImage(image, 0, 0);
      resolve(context.getImageData(0, 0, 128, 128).data);
    });

    image.addEventListener("error", reject);
    image.src = minecraftInputAssets.atlas;
  }));
};

const advance = (char: string) =>
  asciiGlyphs[
    // SAFETY: Private glyph callers supply nonempty iteration characters or guarded in-bounds code units. Its first code point therefore exists.
    // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- The verified glyph-character producer proves this specific codePointAt(0) result.
    (char.codePointAt(0) as number) - 32
  ]?.[2] ??
  getUnihexGlyph(char)?.advance ??
  0;

export const MinecraftUnicodeLabel = ({
  text,
  atlas,
}: {
  text: string;
  atlas?: UnihexAtlas;
}) => {
  const root = React.useRef<HTMLSpanElement>(null);
  const canvas = React.useRef<HTMLCanvasElement>(null);
  const { attach, connection } = useCanvasPaintConnection(canvas);

  const [width, setWidth] = React.useState<number>();

  const paintInput = React.useMemo(
    () => ({ atlas, connection, text }),
    [atlas, text, connection]
  );

  const [completedInput, setCompletedInput] =
    React.useState<typeof paintInput>();

  const bitmap = completedInput === paintInput;

  React.useLayoutEffect((): (() => void) | undefined => {
    if (connection === null) {
      return undefined;
    }

    // SAFETY: React attaches this private, unkeyed <span> before effect setup. The ref has one intrinsic attachment and no other writer; existing cleanup guards or cancels every deferred use.
    // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- The owned intrinsic commit and existing effect lifetime prove this exact HTMLSpanElement capture.
    const node = root.current as HTMLSpanElement;

    let disposed = false;
    let frame = 0;
    let signature = "";

    const context =
      // SAFETY: React attaches this always-rendered <canvas> before its synchronous layout effect setup reads the owned canvas ref.
      // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- React commit ordering and this intrinsic ref attachment prove the exact HTMLCanvasElement result.
      (canvas.current as HTMLCanvasElement).getContext("2d");

    if (!context) {
      throw new Error(
        "Minecraft unicode label 2D canvas context could not be created"
      );
    }

    setUnihexAtlas(context, atlas);
    const glyphResources = Promise.all([loadUnihex(), loadAlpha()]);

    const paintAfterGlyphResources = async () => {
      const [, pixels] = await glyphResources;

      if (disposed) {
        return;
      }

      if (!hasSupportedLineGlyphs(text)) {
        setWidth(undefined);

        return;
      }

      let accumulatedGlyphAdvance = 0;

      for (const advanceCharacter of text) {
        accumulatedGlyphAdvance += advance(advanceCharacter);
      }

      const logicalWidth = Math.ceil(accumulatedGlyphAdvance);

      setWidth(logicalWidth);

      const tick = () => {
        if (disposed) {
          return;
        }

        const style = getComputedStyle(node);
        const scale = Number(style.getPropertyValue("--mc-scale")) || 2;

        const bounds = node.getBoundingClientRect();
        const ink = Number(style.color.match(/[\d.]+/gu)?.[0] ?? 255);

        const next = JSON.stringify([
          scale,
          bounds.x,
          bounds.y,
          bounds.width,
          ink,
          innerWidth,
          innerHeight,
        ]);

        if (
          Math.abs(bounds.width - logicalWidth * scale) < 0.1 &&
          next !== signature
        ) {
          signature = next;
          context.canvas.width = Math.max(1, logicalWidth * scale);
          context.canvas.height = 9 * scale;
          context.imageSmoothingEnabled = false;
          positionUnihex(context, bounds);

          for (const shadow of [true, false]) {
            let x = 0;
            const color = shadow ? Math.floor(ink / 4) : ink;
            context.fillStyle = `rgb(${color},${color},${color})`;

            for (const char of text) {
              const glyph =
                asciiGlyphs[
                  // SAFETY: String iteration supplies a nonempty character, so its first code point exists.
                  // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- The intrinsic string iterator proves this specific codePointAt(0) result.
                  (char.codePointAt(0) as number) - 32
                ];

              if (glyph) {
                if (char !== " ") {
                  for (let y = 0; y < 8; y += 1) {
                    for (let px = 0; px < 8; px += 1) {
                      if (
                        pixels[((glyph[1] + y) * 128 + glyph[0] + px) * 4 + 3]
                      ) {
                        context.fillRect(
                          (x + px + Number(shadow)) * scale,
                          (y + Number(shadow)) * scale,
                          scale,
                          scale
                        );
                      }
                    }
                  }
                }
              } else {
                // SAFETY: completed loadUnihex and hasSupportedLineGlyphs established this character's glyph; the private font map is never cleared or replaced afterward, and this branch excludes every populated ASCII glyph.
                // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- The same immutable text and initialized glyph-map membership prove this exact UnihexGlyph read.
                const unicodeGlyph = getUnihexGlyph(char) as UnihexGlyph;
                paintUnihex(
                  context,
                  unicodeGlyph,
                  x + (shadow ? 0.5 : 0),
                  shadow ? 0.5 : 0,
                  scale
                );
              }

              x += advance(char);
            }
          }

          setCompletedInput(paintInput);
        }

        frame = requestAnimationFrame(tick);
      };

      frame = requestAnimationFrame(tick);
    };

    paintAfterGlyphResources().catch(() => {
      if (!disposed) {
        setCompletedInput(undefined);
      }
    });

    return () => {
      disposed = true;
      cancelAnimationFrame(frame);
      disposeUnihex(context);
    };
  }, [text, atlas, paintInput, connection]);

  const labelStyle: UnicodeLabelStyle = {
    "--mc-unicode-width":
      width === undefined ? "auto" : `calc(${width} * var(--mc-pixel))`,
  };

  return (
    <span
      ref={root}
      data-mc-label-ready={bitmap}
      className="mc-unicode-label"
      style={labelStyle}
    >
      <canvas
        ref={attach}
        aria-hidden="true"
        className="mc-unicode-label-canvas"
      />
      <span className={bitmap ? "mc-text-accessible" : undefined}>{text}</span>
    </span>
  );
};
