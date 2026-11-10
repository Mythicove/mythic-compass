import type { UnihexGlyph } from "./minecraft-unifont";
import { nativeUnihexAtlas } from "./minecraft-unifont-atlas";

const fallbackGlyphAtlas: Partial<typeof nativeUnihexAtlas> = nativeUnihexAtlas;

interface Painter {
  canvas: HTMLCanvasElement;
  gl: WebGLRenderingContext;
  buffer: WebGLBuffer;
  texture: WebGLTexture;
  program: WebGLProgram;
}

// Copy each glyph synchronously into its own 2D target, then reuse one offscreen
// painter. A target per label otherwise exhausts Chromium's active GL budget.
let sharedPainter: Painter | null | undefined;
const targets = new WeakSet<CanvasRenderingContext2D>();
let targetCount = 0;
const origins = new WeakMap<CanvasRenderingContext2D, DOMRect>();

export type UnihexAtlas = Record<
  string,
  { u0: number; u1: number; v0: number; v1: number }
>;

const atlases = new WeakMap<CanvasRenderingContext2D, UnihexAtlas>();

export const setUnihexAtlas = function setUnihexAtlas(
  context: CanvasRenderingContext2D,
  atlas?: UnihexAtlas
) {
  if (atlas) {
    atlases.set(context, atlas);
  } else {
    atlases.delete(context);
  }
};

export const positionUnihex = function positionUnihex(
  context: CanvasRenderingContext2D,
  bounds: DOMRect
) {
  origins.set(context, bounds);
};

const releasePainter = function releasePainter(
  painter: Painter | null | undefined
) {
  if (!painter) {
    return;
  }

  const { gl, buffer, texture, program } = painter;
  gl.deleteBuffer(buffer);
  gl.deleteTexture(texture);
  gl.deleteProgram(program);
  gl.getExtension("WEBGL_lose_context")?.loseContext();
};

export const disposeUnihex = function disposeUnihex(
  context: CanvasRenderingContext2D
) {
  origins.delete(context);
  atlases.delete(context);

  if (!targets.delete(context)) {
    return;
  }

  targetCount -= 1;

  if (targetCount === 0) {
    releasePainter(sharedPainter);
    sharedPainter = undefined;
  }
};

const create = function create(): Painter | null {
  const canvas = document.createElement("canvas");

  const gl = canvas.getContext("webgl", {
    alpha: true,
    antialias: false,
    premultipliedAlpha: false,
    preserveDrawingBuffer: true,
  });

  if (!gl) {
    return null;
  }

  try {
    const compile = (kind: number, source: string) => {
      const shader = gl.createShader(kind);

      if (!shader) {
        throw new Error(
          "Minecraft unifont gpu WebGL shader could not be created"
        );
      }

      gl.shaderSource(shader, source);
      gl.compileShader(shader);

      if (gl.getShaderParameter(shader, gl.COMPILE_STATUS) !== true) {
        throw new Error(
          gl.getShaderInfoLog(shader) ?? "Native font shader failed"
        );
      }

      return shader;
    };

    const vertex = compile(
      gl.VERTEX_SHADER,
      "attribute vec2 position; attribute vec2 uv; uniform vec2 projection; varying vec2 coord; void main(){gl_Position=vec4(position*projection+vec2(-1.,1.),0.,1.);coord=uv;}"
    );

    const fragment = compile(
      gl.FRAGMENT_SHADER,
      "precision highp float; varying vec2 coord; uniform sampler2D atlas; uniform float ink; void main(){float a=texture2D(atlas,coord).a;if(a<.1)discard;gl_FragColor=vec4(vec3(ink),a);}"
    );

    const program = gl.createProgram();
    gl.attachShader(program, vertex);
    gl.attachShader(program, fragment);
    gl.linkProgram(program);
    gl.useProgram(program);
    gl.deleteShader(vertex);
    gl.deleteShader(fragment);
    const texture = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, texture);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texImage2D(
      gl.TEXTURE_2D,
      0,
      gl.RGBA,
      256,
      256,
      0,
      gl.RGBA,
      gl.UNSIGNED_BYTE,
      null
    );
    const buffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);

    for (const [name, offset] of [
      ["position", 0],
      ["uv", 8],
    ] as const) {
      const location = gl.getAttribLocation(program, name);
      gl.enableVertexAttribArray(location);
      gl.vertexAttribPointer(location, 2, gl.FLOAT, false, 16, offset);
    }

    gl.disable(gl.DITHER);

    return { buffer, canvas, gl, program, texture };
  } catch (error) {
    const lost = gl.isContextLost();
    gl.getExtension("WEBGL_lose_context")?.loseContext();

    if (lost) {
      return null;
    }

    throw error;
  }
};

export const paintUnihexGPU = function paintUnihexGPU(
  context: CanvasRenderingContext2D,
  glyph: UnihexGlyph,
  x: number,
  y: number,
  scale: number
) {
  if (!targets.has(context)) {
    targets.add(context);
    targetCount += 1;
  }

  let painter = sharedPainter;

  if (painter?.gl.isContextLost() === true) {
    releasePainter(painter);
    painter = undefined;
    sharedPainter = painter;
  }

  if (painter === undefined) {
    painter = create();
    sharedPainter = painter;
  }

  if (!painter) {
    return false;
  }

  const { gl, canvas, program } = painter;
  const bounds = origins.get(context);

  const originX = Math.round(bounds?.x ?? 0);
  const originY = Math.round(bounds?.y ?? 0);

  if (canvas.width !== innerWidth) {
    canvas.width = innerWidth;
  }

  if (canvas.height !== innerHeight) {
    canvas.height = innerHeight;
  }

  gl.viewport(0, 0, canvas.width, canvas.height);
  gl.uniform2f(
    gl.getUniformLocation(program, "projection"),
    2 / canvas.width,
    -2 / canvas.height
  );
  // oxlint-disable-next-line typescript/no-base-to-string -- The existing Canvas context API accepts string, CanvasGradient and CanvasPattern; retain its exact String coercion and downstream color-parse fallback without narrowing the context or changing custom coercion behavior.
  const fill = String(context.fillStyle);

  const ink = fill.startsWith("#")
    ? Number.parseInt(fill.slice(1, 3), 16)
    : (fill.match(/[\d.]+/gu)?.map(Number)?.[0] ?? 224);

  gl.uniform1f(gl.getUniformLocation(program, "ink"), ink / 255);
  gl.clearColor(0, 0, 0, 0);
  gl.clear(gl.COLOR_BUFFER_BIT);
  const rgba = new Uint8Array(glyph.width * 16 * 4);

  for (let row = 0; row < 16; row += 1) {
    for (let col = 0; col < glyph.width; col += 1) {
      const on =
        // oxlint-disable-next-line no-bitwise -- Unihex rows store one bit per glyph column; extract the selected column bit.
        (glyph.rows[row] >>> (glyph.bitWidth - 1 - col - glyph.left)) & 1;

      rgba.set([255, 255, 255, on ? 255 : 0], (row * glyph.width + col) * 4);
    }
  }

  const submitGlyphQuad = () => {
    const observed = (atlases.get(context) ?? fallbackGlyphAtlas)[
      glyph.code.toString(16)
    ];

    const atlasX = observed ? Math.round(observed.u0 * 256 - 0.01) : 0;
    const atlasY = observed ? Math.round(observed.v0 * 256 - 0.01) : 0;

    gl.texSubImage2D(
      gl.TEXTURE_2D,
      0,
      atlasX,
      atlasY,
      glyph.width,
      16,
      gl.RGBA,
      gl.UNSIGNED_BYTE,
      rgba
    );

    const left = originX + x * scale;
    const top = originY + y * scale;
    const right = left + (glyph.width / 2) * scale;
    const bottom = top + 8 * scale;

    const u0 = observed?.u0 ?? Math.fround(0.01) / 256;

    const u1 =
      observed?.u1 ?? Math.fround(glyph.width - Math.fround(0.01)) / 256;

    const v0 = observed?.v0 ?? u0;
    const v1 = observed?.v1 ?? Math.fround(16 - Math.fround(0.01)) / 256;

    const a = [left, top, u0, v0];
    const b = [left, bottom, u0, v1];
    const c = [right, bottom, u1, v1];
    const d = [right, top, u1, v0];

    gl.bufferData(
      gl.ARRAY_BUFFER,
      new Float32Array([...a, ...b, ...c, ...a, ...c, ...d]),
      gl.STREAM_DRAW
    );
    gl.drawArrays(gl.TRIANGLES, 0, 6);
  };

  submitGlyphQuad();

  if (gl.isContextLost()) {
    sharedPainter = undefined;

    return false;
  }

  context.drawImage(
    canvas,
    originX,
    originY,
    context.canvas.width,
    context.canvas.height,
    0,
    0,
    context.canvas.width,
    context.canvas.height
  );

  return true;
};
