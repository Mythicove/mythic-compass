"use client";

import * as React from "react";

import {
  isMinecraftRefCallback,
  isMinecraftRefCleanup,
  minecraftClasses as cn,
  minecraftStyle,
} from "./minecraft-render";
import { playMinecraftClick } from "./minecraft-sound";
import { Select, SelectTrigger } from "./select";

import "./minecraft-native-select.css";

interface NativeSelectScaleStyle extends React.CSSProperties {
  "--mc-scale": number;
}

type NativeSelectProps = Omit<React.ComponentProps<"select">, "size"> & {
  size?: "default";
  scale?: 1 | 2 | 3;
  wrapperProps?: React.ComponentProps<"div">;
};

const NativeSelect = ({
  size = "default",
  scale = 2,
  wrapperProps,
  className,
  style,
  children,
  ref,
  onChange,
  onPointerDown,
  onKeyDown,
  multiple,
  ...props
}: NativeSelectProps) => {
  if (size !== "default") {
    throw new Error(
      "Minecraft native select supports the original default CycleButton height only"
    );
  }

  const node = React.useRef<HTMLSelectElement>(null);
  const [label, setLabel] = React.useState("");

  const forwarded = React.useCallback(
    (element: HTMLSelectElement | null): (() => void) | undefined => {
      node.current = element;

      if (isMinecraftRefCallback(ref)) {
        const cleanup = ref(element);

        if (isMinecraftRefCleanup(cleanup)) {
          return () => {
            node.current = null;
            cleanup();
          };
        }

        return undefined;
      }

      if (ref) {
        ref.current = element;
      }

      return undefined;
    },
    [ref]
  );

  React.useLayoutEffect((): (() => void) | undefined => {
    const element = node.current;

    if (!element) {
      return undefined;
    }

    let frame = 0;

    const update = () => {
      setLabel(
        [...element.selectedOptions].map((option) => option.label).join(", ")
      );
    };

    const deferred = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(update);
    };

    update();
    const observer = new MutationObserver(update);
    observer.observe(element, {
      attributeFilter: ["selected", "label", "value"],
      attributes: true,
      characterData: true,
      childList: true,
      subtree: true,
    });
    element.form?.addEventListener("reset", deferred);

    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
      element.form?.removeEventListener("reset", deferred);
    };
    // oxlint-disable-next-line react/exhaustive-effect-dependencies -- Controlled/default values and form/options/multiple change native selectedOptions; refresh the painted label and reset association after commit.
  }, [props.value, props.defaultValue, props.form, children, multiple]);

  return (
    <div
      {...wrapperProps}
      data-slot="native-select-wrapper"
      className={`mc-native-select-wrapper ${(multiple ?? false) ? "mc-native-select-multiple" : ""} ${wrapperProps?.className ?? ""}`}
      // oxlint-disable-next-line shadcn/no-inline-styles -- Preserve the supported caller wrapper style API; this factory adds only the owned scale variable and forwards caller properties unchanged.
      style={minecraftStyle(
        wrapperProps?.style,
        (__style): NativeSelectScaleStyle => ({
          "--mc-scale": scale,
          ...__style,
        })
      )}
    >
      <select
        data-slot="native-select"
        data-size={size}
        ref={forwarded}
        className={cn(`mc-native-select-control`, className)}
        style={style}
        multiple={multiple}
        {...props}
        onChange={(event) => {
          onChange?.(event);
          queueMicrotask(() => {
            if (node.current) {
              setLabel(
                [...node.current.selectedOptions]
                  .map((option) => option.label)
                  .join(", ")
              );
            }
          });
        }}
        onPointerDown={(event) => {
          onPointerDown?.(event);

          if (
            !event.defaultPrevented &&
            !(props.disabled ?? false) &&
            event.button === 0
          ) {
            playMinecraftClick();
          }
        }}
        onKeyDown={(event) => {
          onKeyDown?.(event);

          if (
            !event.defaultPrevented &&
            !(props.disabled ?? false) &&
            !event.repeat &&
            (event.key === "Enter" ||
              event.key === " " ||
              (event.altKey && event.key === "ArrowDown"))
          ) {
            playMinecraftClick();
          }
        }}
      >
        {children}
      </select>
      {!(multiple ?? false) && (
        <div className="mc-native-select-art" aria-hidden="true" inert>
          <Select
            scale={scale}
            open={false}
            value={label}
            disabled={props.disabled}
          >
            <SelectTrigger
              aria-disabled={props.disabled ?? undefined}
              tabIndex={-1}
              className="h-full w-full"
            >
              {label}
            </SelectTrigger>
          </Select>
        </div>
      )}
    </div>
  );
};

const NativeSelectOption = ({
  className,
  ...props
}: React.ComponentProps<"option">) => (
  <option
    data-slot="native-select-option"
    className={cn(`mc-native-select-option`, className)}
    {...props}
  />
);

const NativeSelectOptGroup = ({
  className,
  ...props
}: React.ComponentProps<"optgroup">) => (
  <optgroup
    data-slot="native-select-optgroup"
    className={cn(`mc-native-select-optgroup`, className)}
    {...props}
  />
);

export { NativeSelect, NativeSelectOption, NativeSelectOptGroup };

export type { NativeSelectProps };
