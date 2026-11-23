"use client";

import * as React from "react";

import { minecraftClasses as cn, minecraftStyle } from "./minecraft-render";
import { attachMinecraftTextarea } from "./minecraft-textarea-paint";

import "./minecraft-unifont-metrics.css";
import "./minecraft-textarea.css";

interface TextareaStyle extends React.CSSProperties {
  "--mc-scale": number;
}

const Textarea = ({
  className,
  scale = 2,
  style,
  ref,
  ...props
}: React.ComponentProps<"textarea"> & {
  scale?: 1 | 2 | 3;
}) => {
  const node = React.useRef<HTMLTextAreaElement>(null);
  React.useImperativeHandle<typeof node.current, typeof node.current>(
    ref,
    () => node.current,
    []
  );
  React.useEffect(
    () =>
      node.current ? attachMinecraftTextarea(node.current, scale) : undefined,
    [scale]
  );

  return (
    <textarea
      ref={node}
      data-slot="textarea"
      spellCheck={false}
      autoCorrect="off"
      className={cn("mc-textarea", className)}
      style={minecraftStyle(style, (__style): TextareaStyle => ({
        "--mc-scale": scale,
        ...__style,
      }))}
      {...props}
    />
  );
};

export { Textarea };
