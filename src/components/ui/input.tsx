"use client";

import { Input as InputPrimitive } from "@base-ui/react/input";
import * as React from "react";

import { attachMinecraftInput } from "./minecraft-input-paint";
import { minecraftClasses as cn, minecraftStyle } from "./minecraft-render";

import "./minecraft-unifont-metrics.css";
import "./minecraft-input.css";

interface InputStyle extends React.CSSProperties {
  "--mc-scale": number;
}

const Input = ({
  className,
  type = "text",
  scale = 2,
  style,
  ref,
  ...props
}: React.ComponentProps<"input"> & {
  scale?: 1 | 2 | 3;
}) => {
  const node = React.useRef<HTMLInputElement>(null);
  React.useImperativeHandle<typeof node.current, typeof node.current>(
    ref,
    () => node.current,
    []
  );
  React.useEffect(
    () =>
      node.current ? attachMinecraftInput(node.current, scale) : undefined,
    [scale]
  );

  return (
    <InputPrimitive
      ref={node}
      type={type}
      data-slot="input"
      spellCheck={false}
      autoCorrect="off"
      className={cn("mc-input", className)}
      style={minecraftStyle(style, (__style): InputStyle => ({
        "--mc-scale": scale,
        ...__style,
      }))}
      {...props}
    />
  );
};

export { Input };
