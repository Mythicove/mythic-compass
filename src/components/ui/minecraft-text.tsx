import * as React from "react";

import { minecraftGlyphs } from "./minecraft-font";
import {
  isMinecraftTextContent,
  minecraftClasses as cn,
} from "./minecraft-render";

import "./minecraft-text.css";

interface MinecraftGlyphStyle extends React.CSSProperties {
  "--mc-advance": number;
  "--mc-glyph-x": number;
  "--mc-glyph-y": number;
}

const MinecraftText = ({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) => {
  if (
    !isMinecraftTextContent(children) ||
    !/^[\u0020-\u007E]*$/u.test(String(children))
  ) {
    return <span className={className}>{children}</span>;
  }

  const text = String(children);

  return (
    <span className={cn(`mc-bitmap`, className)}>
      <span className="mc-text-pixels" aria-hidden="true">
        {Array.from(text, (char, index) => {
          const [x, y, advance] =
            minecraftGlyphs[(char.codePointAt(0) ?? Number.NaN) - 32];

          const glyphStyle: MinecraftGlyphStyle = {
            "--mc-advance": advance,
            "--mc-glyph-x": x,
            "--mc-glyph-y": y,
          };

          return (
            <span
              key={index}
              className={cn("mc-glyph", char === " " && "mc-glyph-space")}
              style={glyphStyle}
            />
          );
        })}
      </span>
      <span className="mc-text-accessible">{text}</span>
    </span>
  );
};

export { MinecraftText };
