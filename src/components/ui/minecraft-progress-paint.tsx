"use client";

import * as React from "react";

import { minecraftProgressRgba } from "./minecraft-progress-assets";

type Sprite = keyof typeof minecraftProgressRgba;

interface Painter {
  canvas: HTMLCanvasElement;
  gl: WebGLRenderingContext;
  texture: WebGLTexture;
}

let shared: Painter | null | undefined;
const decoded = new Map<Sprite, Uint8Array>();

const create = function create(): Painter | null {
  const canvas = document.createElement("canvas");
  canvas.width = 182;
  canvas.height = 5;

  const gl = canvas.getContext("webgl", {
    alpha: true,
    antialias: false,
    premultipliedAlpha: false,
    preserveDrawingBuffer: true,
  });

  if (!gl) {
    return null;
  }

  const shader = (kind: number, source: string) => {
    const result = gl.createShader(kind);

    if (!result) {
      throw new Error(
        "Minecraft progress paint WebGL shader could not be created"
      );
    }

    gl.shaderSource(result, source);
    gl.compileShader(result);

    if (gl.getShaderParameter(result, gl.COMPILE_STATUS) !== true) {
      throw new Error(gl.getShaderInfoLog(result) ?? "Progress shader failed");
    }

    return result;
  };

  const vertex = shader(
    gl.VERTEX_SHADER,
    "attribute vec2 position;attribute vec2 uv;varying vec2 coord;void main(){gl_Position=vec4(position,0.,1.);coord=uv;}"
  );

  const fragment = shader(
    gl.FRAGMENT_SHADER,
    "precision highp float;varying vec2 coord;uniform sampler2D atlas;void main(){gl_FragColor=texture2D(atlas,coord);}"
  );

  const program = gl.createProgram();
  gl.attachShader(program, vertex);
  gl.attachShader(program, fragment);
  gl.linkProgram(program);
  gl.useProgram(program);
  gl.deleteShader(vertex);
  gl.deleteShader(fragment);
  gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
  gl.bufferData(
    gl.ARRAY_BUFFER,
    new Float32Array([
      -1, 1, 0, 0, -1, -1, 0, 1, 1, -1, 1, 1, -1, 1, 0, 0, 1, -1, 1, 1, 1, 1, 1,
      0,
    ]),
    gl.STATIC_DRAW
  );

  for (const [name, offset] of [
    ["position", 0],
    ["uv", 8],
  ] as const) {
    const location = gl.getAttribLocation(program, name);
    gl.enableVertexAttribArray(location);
    gl.vertexAttribPointer(location, 2, gl.FLOAT, false, 16, offset);
  }

  const texture = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, texture);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  gl.disable(gl.DITHER);
  gl.enable(gl.BLEND);
  gl.blendFuncSeparate(
    gl.SRC_ALPHA,
    gl.ONE_MINUS_SRC_ALPHA,
    gl.ONE,
    gl.ONE_MINUS_SRC_ALPHA
  );
  gl.viewport(0, 0, 182, 5);

  return { canvas, gl, texture };
};

const paint = function paint(
  canvas: HTMLCanvasElement,
  background: Sprite,
  foreground: Sprite,
  notchBackground: Sprite,
  notchForeground: Sprite,
  filled: number
) {
  if (shared?.gl.isContextLost() ?? false) {
    shared = undefined;
  }

  if (shared === undefined) {
    shared = create();
  }

  if (!shared) {
    return false;
  }

  const { gl } = shared;
  gl.disable(gl.SCISSOR_TEST);
  gl.clearColor(0, 0, 0, 0);
  gl.clear(gl.COLOR_BUFFER_BIT);

  const draw = (name: Sprite) => {
    let pixels = decoded.get(name);

    if (!pixels) {
      pixels = Uint8Array.from(
        atob(minecraftProgressRgba[name]),
        (char) => char.codePointAt(0) ?? Number.NaN
      );
      decoded.set(name, pixels);
    }

    gl.texImage2D(
      gl.TEXTURE_2D,
      0,
      gl.RGBA,
      182,
      5,
      0,
      gl.RGBA,
      gl.UNSIGNED_BYTE,
      pixels
    );
    gl.drawArrays(gl.TRIANGLES, 0, 6);
  };

  draw(background);
  draw(notchBackground);

  if (filled > 0) {
    gl.enable(gl.SCISSOR_TEST);
    gl.scissor(0, 0, filled, 5);
    draw(foreground);
    draw(notchForeground);
  }

  const context = canvas.getContext("2d");

  if (!context) {
    throw new Error(
      "Minecraft progress paint 2D canvas context could not be created"
    );
  }

  context.clearRect(0, 0, 182, 5);
  context.imageSmoothingEnabled = false;
  context.drawImage(shared.canvas, 0, 0);

  return true;
};

export const MinecraftProgressPaint = ({
  background,
  foreground,
  notchBackground,
  notchForeground,
  filled,
}: {
  background: Sprite;
  foreground: Sprite;
  notchBackground: Sprite;
  notchForeground: Sprite;
  filled: number;
}) => {
  const canvas = React.useRef<HTMLCanvasElement>(null);
  const [ready, setReady] = React.useState(false);
  React.useLayoutEffect(() => {
    setReady(
      paint(
        // SAFETY: React attaches this always-rendered <canvas> before its synchronous layout effect setup reads the owned canvas ref.
        // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- React commit ordering and this intrinsic ref attachment prove the exact HTMLCanvasElement result.
        canvas.current as HTMLCanvasElement,
        background,
        foreground,
        notchBackground,
        notchForeground,
        filled
      )
    );
  }, [background, foreground, notchBackground, notchForeground, filled]);

  return (
    <canvas
      ref={canvas}
      width={182}
      height={5}
      aria-hidden="true"
      className="mc-progress-paint"
      data-mc-progress-painted={ready}
    />
  );
};
