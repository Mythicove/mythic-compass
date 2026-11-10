"use client";

import { Progress as ProgressPrimitive } from "@base-ui/react/progress";
import * as React from "react";

import { minecraftProgressAssets } from "./minecraft-progress-assets";
import { MinecraftProgressPaint } from "./minecraft-progress-paint";
import { minecraftClasses as cn, minecraftStyle } from "./minecraft-render";
import type { MinecraftProps } from "./minecraft-render";

import "./minecraft-progress.css";

interface OwnedProgressStyle extends React.CSSProperties {
  "--mc-z-progress-paint-1-width"?: string;
}

interface ProgressStyle extends React.CSSProperties {
  "--mc-scale": number;
}

interface ProgressSurfaceStyle extends React.CSSProperties {
  "--mc-progress-image": string;
}

type BossBarColor =
  | "pink"
  | "blue"
  | "red"
  | "green"
  | "yellow"
  | "purple"
  | "white";

type BossBarOverlay =
  | "progress"
  | "notched_6"
  | "notched_10"
  | "notched_12"
  | "notched_20";

type ProgressProps = Omit<
  MinecraftProps<typeof ProgressPrimitive.Root>,
  "color"
> & {
  color?: BossBarColor;
  overlay?: BossBarOverlay;
  scale?: 1 | 2 | 3;
};

/** Native determinate progress accepts only numeric values within its maximum. */
const isDeterminateProgressValue = (
  value: ProgressProps["value"],
  maximum: number
): value is number =>
  typeof value === "number" &&
  !Number.isNaN(value) &&
  value >= 0 &&
  value <= maximum;

// Base UI supplies percentage/height defaults; this private artwork part owns native pixel geometry.
const ProgressIndicatorArtwork = ({
  style: _primitiveStyle,
  ...props
}: React.ComponentProps<"div">) => <div {...props} />;

const ProgressTrack = ({
  className,
  ...props
}: MinecraftProps<typeof ProgressPrimitive.Track>) => (
  <ProgressPrimitive.Track
    data-slot="progress-track"
    className={cn(className)}
    {...props}
  />
);

const Progress = ({
  className,
  value,
  max = 100,
  color = "pink",
  overlay = "progress",
  scale = 2,
  style,
  children,
  render,
  ref,
  ...props
}: ProgressProps) => {
  const maximum = !Number.isNaN(max) && max > 0 ? max : 100;

  const valid = isDeterminateProgressValue(value, maximum);

  const ratio = valid ? value / maximum : 0;
  const fraction = Number.isFinite(ratio) ? Math.fround(ratio) : 0;
  // Original Mth.lerpDiscrete uses a float product and includes the first pixel for any positive progress.
  const filled = Math.floor(Math.fround(fraction * 181)) + Number(fraction > 0);

  const background = minecraftProgressAssets[`${color}_background`];
  const foreground = minecraftProgressAssets[`${color}_progress`];

  const notchBackground =
    overlay === "progress"
      ? undefined
      : minecraftProgressAssets[`${overlay}_background`];

  const notchForeground =
    overlay === "progress"
      ? undefined
      : minecraftProgressAssets[`${overlay}_progress`];

  if (
    !background ||
    !foreground ||
    (overlay !== "progress" && !notchBackground)
  ) {
    throw new Error(
      "Progress supports only original Minecraft boss-bar colors and overlays"
    );
  }

  const content = children;

  const backgroundStyle: ProgressSurfaceStyle = {
    "--mc-progress-image": `url("${background}")`,
  };

  const notchBackgroundStyle: ProgressSurfaceStyle | undefined = notchBackground
    ? { "--mc-progress-image": `url("${notchBackground}")` }
    : undefined;

  const foregroundStyle: ProgressSurfaceStyle = {
    "--mc-progress-image": `url("${foreground}")`,
  };

  const notchForegroundStyle: ProgressSurfaceStyle | undefined = notchForeground
    ? { "--mc-progress-image": `url("${notchForeground}")` }
    : undefined;

  const ownedStyle1: OwnedProgressStyle = {
    "--mc-z-progress-paint-1-width": `calc(${filled} * var(--mc-pixel))`,
  };

  const artwork = (
    <>
      {overlay !== "progress" && (
        <MinecraftProgressPaint
          background={`${color}_background`}
          foreground={`${color}_progress`}
          notchBackground={`${overlay}_background`}
          notchForeground={`${overlay}_progress`}
          filled={filled}
        />
      )}
      <ProgressTrack
        className="mc-progress-fallback"
        aria-hidden="true"
        style={ownedStyle1}
      >
        <span
          aria-hidden="true"
          className="mc-progress-surface"
          style={backgroundStyle}
        />
        {notchBackground && (
          <span
            aria-hidden="true"
            className="mc-progress-surface"
            style={notchBackgroundStyle}
          />
        )}
        <ProgressPrimitive.Indicator
          data-slot="progress-indicator"
          aria-hidden="true"
          className="mc-progress-indicator mc-owned-paint-progress-1"
          render={<ProgressIndicatorArtwork />}
        >
          <span className="mc-progress-surface" style={foregroundStyle} />
          {notchForeground && (
            <span
              className="mc-progress-surface"
              style={notchForegroundStyle}
            />
          )}
        </ProgressPrimitive.Indicator>
      </ProgressTrack>
      {content}
    </>
  );

  return (
    <ProgressPrimitive.Root
      render={render}
      ref={ref}
      value={value}
      max={max}
      data-slot="progress"
      data-color={color}
      data-overlay={overlay}
      className={cn(`mc-progress`, className)}
      style={minecraftStyle(style, (__style): ProgressStyle => ({
        "--mc-scale": scale,
        ...__style,
      }))}
      {...props}
    >
      {artwork}
    </ProgressPrimitive.Root>
  );
};

const ProgressIndicator = ({
  className,
  ...props
}: MinecraftProps<typeof ProgressPrimitive.Indicator>) => (
  <ProgressPrimitive.Indicator
    data-slot="progress-indicator"
    className={cn(`mc-progress-indicator`, className)}
    {...props}
  />
);

const ProgressLabel = (
  props: MinecraftProps<typeof ProgressPrimitive.Label>
) => <ProgressPrimitive.Label data-slot="progress-label" {...props} />;

const ProgressValue = (
  props: MinecraftProps<typeof ProgressPrimitive.Value>
) => <ProgressPrimitive.Value data-slot="progress-value" {...props} />;

export {
  Progress,
  ProgressTrack,
  ProgressIndicator,
  ProgressLabel,
  ProgressValue,
};

export type { ProgressProps, BossBarColor, BossBarOverlay };
