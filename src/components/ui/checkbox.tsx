import { Checkbox as CheckboxPrimitive } from "@base-ui/react/checkbox";
import * as React from "react";

import { minecraftClasses as cn, minecraftStyle } from "./minecraft-render";
import type { MinecraftProps } from "./minecraft-render";

import "./minecraft-checkbox.css";
import { playMinecraftClick } from "./minecraft-sound";

interface CheckboxPaintStyle extends React.CSSProperties {
  "--mc-checkbox-checked": string;
  "--mc-checkbox-checked-focused": string;
  "--mc-checkbox-focused": string;
  "--mc-checkbox-normal": string;
  "--mc-scale": number;
}

const Checkbox = ({
  className,
  scale = 2,
  style,
  onClick,
  ...props
}: MinecraftProps<typeof CheckboxPrimitive.Root> & {
  scale?: 1 | 2 | 3;
}) => (
  <CheckboxPrimitive.Root
    data-slot="checkbox"
    className={cn("peer mc-checkbox", className)}
    style={minecraftStyle(style, (__style): CheckboxPaintStyle => ({
      "--mc-checkbox-checked": `var(--mc-checkbox-selected-native-s${scale})`,
      "--mc-checkbox-checked-focused": `var(--mc-checkbox-selected-highlighted-native-s${scale})`,
      "--mc-checkbox-focused": `var(--mc-checkbox-highlighted-native-s${scale})`,
      "--mc-checkbox-normal": `var(--mc-checkbox-native-s${scale})`,
      "--mc-scale": scale,
      ...__style,
    }))}
    {...props}
    onClick={(event) => {
      onClick?.(event);

      if (
        !event.defaultPrevented &&
        !(event.baseUIHandlerPrevented ?? false) &&
        !(props.disabled ?? false) &&
        !Object.hasOwn(event.currentTarget.dataset, "disabled")
      ) {
        playMinecraftClick();
      }
    }}
  >
    <CheckboxPrimitive.Indicator
      data-slot="checkbox-indicator"
      className="mc-checkbox-indicator"
      aria-hidden="true"
    />
  </CheckboxPrimitive.Root>
);

export { Checkbox };
