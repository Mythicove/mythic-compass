"use client";

import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { cva } from "class-variance-authority";
import type { VariantProps } from "class-variance-authority";
// Primitive composition: shadcn-ui/ui apps/v4/registry/bases/base/ui/button.tsx (MIT).
import * as React from "react";
import { isPortal } from "react-is";

import { minecraftGlyphs } from "./minecraft-font";
import {
  isMinecraftEmptyChild,
  isMinecraftTextScalar,
  minecraftClasses as cn,
  minecraftStyle,
} from "./minecraft-render";
import type { MinecraftProps } from "./minecraft-render";

import "./minecraft-button.css";
import { playMinecraftClick } from "./minecraft-sound";
import { MinecraftUnicodeLabel } from "./minecraft-unicode-label";

interface ButtonStyle extends React.CSSProperties {
  "--mc-scale": number;
}

interface ButtonGlyphStyle extends React.CSSProperties {
  "--mc-advance": number;
  "--mc-glyph-x": number;
  "--mc-glyph-y": number;
}

const bitmapText = function bitmapText(
  content: React.ReactNode
): React.ReactNode {
  if (
    Array.isArray(content) ||
    (React.isValidElement(content) && content.type === React.Fragment)
  ) {
    const parts: { child: React.ReactNode; key: string }[] = [];
    let run = "";
    let runKey = "";

    const flush = () => {
      if (run) {
        parts.push({ child: run, key: runKey });
        run = "";
      }
    };

    const visit = (child: React.ReactNode, scope: string[]) => {
      if (isMinecraftEmptyChild(child)) {
        return;
      }

      if (Array.isArray(child)) {
        let position = 0;
        // oxlint-disable-next-line unicorn/no-array-for-each -- Preserve this consumer ReactNode array's existing method dispatch and sparse/mutation behavior; indexed traversal would bypass custom forEach overrides.
        child.forEach((entry: React.ReactNode) => {
          const identity = `position:${position}`;

          position += 1;
          visit(entry, [...scope, identity]);
        });

        return;
      }

      const isElement = React.isValidElement<{
        children?: React.ReactNode;
      }>(child);

      const childKey = isElement || isPortal(child) ? child.key : null;

      const childScope =
        childKey === null ? scope : [...scope.slice(0, -1), `key:${childKey}`];

      if (isElement && child.type === React.Fragment) {
        visit(child.props.children, [
          ...childScope,
          `fragment:${childKey ?? ""}`,
        ]);

        return;
      }

      if (isMinecraftTextScalar(child)) {
        if (!run) {
          runKey = `text:${JSON.stringify(childScope)}`;
        }

        run += String(child);
      } else {
        flush();
        parts.push({ child, key: `child:${JSON.stringify(childScope)}` });
      }
    };

    visit(content, []);
    flush();

    return parts.map(({ child, key }) => (
      <React.Fragment key={key}>{bitmapText(child)}</React.Fragment>
    ));
  }

  if (!isMinecraftTextScalar(content)) {
    return content;
  }

  const text = String(content);

  if (!/^[\u0020-\u007E]*$/u.test(text)) {
    return <MinecraftUnicodeLabel text={text} />;
  }

  return (
    <span className="mc-text">
      <span className="mc-text-pixels" aria-hidden="true">
        {Array.from(text, (char, index) => {
          const codePoint = char.codePointAt(0);

          if (codePoint === undefined) {
            throw new Error("Expected a non-empty glyph character");
          }

          const [x, y, advance] = minecraftGlyphs[codePoint - 32];

          const glyphStyle: ButtonGlyphStyle = {
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

const buttonVariants = cva(
  "mc-button relative inline-flex shrink-0 items-center justify-center whitespace-nowrap outline-none disabled:pointer-events-none [&_svg]:pointer-events-none [&_svg]:shrink-0",
  {
    defaultVariants: {
      size: "default",
      variant: "default",
    },
    variants: {
      size: {
        default: "",
      },
      variant: {
        default: "",
      },
    },
  }
);

const skin = (content: React.ReactNode) => (
  <>
    <span className="mc-button-surface" aria-hidden="true">
      {Array.from({ length: 9 }, (_, index) => (
        <span key={index} />
      ))}
    </span>
    <span className="mc-button-label">{bitmapText(content)}</span>
  </>
);

const Button = ({
  className,
  variant = "default",
  size = "default",
  render,
  scale = 2,
  style,
  children,
  onClick,
  ...props
}: MinecraftProps<typeof ButtonPrimitive> &
  VariantProps<typeof buttonVariants> & {
    scale?: 1 | 2 | 3;
  }) => {
  const Comp = ButtonPrimitive;

  const content = skin(children);

  return (
    <Comp
      render={render}
      data-slot="button"
      data-variant={variant}
      data-size={size}
      className={cn(cn(buttonVariants({ size, variant }), className))}
      style={minecraftStyle(style, (__style): ButtonStyle => ({
        "--mc-scale": scale,
        ...__style,
      }))}
      {...props}
      onClick={(event) => {
        onClick?.(event);

        if (
          !event.defaultPrevented &&
          !(props.disabled ?? false) &&
          !Object.hasOwn(event.currentTarget.dataset, "disabled")
        ) {
          playMinecraftClick();
        }
      }}
    >
      {content}
    </Comp>
  );
};

// oxlint-disable-next-line react-doctor/only-export-components -- Preserve the upstream shadcn component-and-variant export API used by consumer styling composition.
export { Button, buttonVariants };
