"use client";

import { mergeProps } from "@base-ui/react/merge-props";
import { useRender } from "@base-ui/react/use-render";
import * as React from "react";

import { cn as mergeClasses } from "@/lib/utils";

/** React refs call their callback branch; object refs retain direct current assignment. */
export const isMinecraftRefCallback = <T,>(
  ref: React.Ref<T> | undefined
): ref is React.RefCallback<T> => typeof ref === "function";

/** React 19 callback refs may return an unmount cleanup instead of void. */
export const isMinecraftRefCleanup = (
  cleanup: ReturnType<React.RefCallback<HTMLElement>>
): cleanup is Exclude<ReturnType<React.RefCallback<HTMLElement>>, void> =>
  typeof cleanup === "function";

interface CanvasPaintConnection {
  canvas: HTMLCanvasElement;
}

/** A committed attachment owns paint completion; Activity can reconnect the same node. */
export const useCanvasPaintConnection = (
  ref: React.RefObject<HTMLCanvasElement | null>
) => {
  const [connection, setConnection] =
    React.useState<CanvasPaintConnection | null>(null);

  const attach = React.useCallback(
    (canvas: HTMLCanvasElement | null) => {
      ref.current = canvas;
      setConnection(canvas === null ? null : { canvas });
    },
    [ref]
  );

  return { attach, connection };
};

type ClassValue = Parameters<typeof mergeClasses>[number];

/** Scalar content supported by the Minecraft bitmap text renderers. */
export const isMinecraftTextContent = <Content,>(
  content: Content
): content is Content & (string | number) =>
  typeof content === "string" || typeof content === "number";

/** A string document owns line/paragraph parsing; other React children remain opaque. */
export const isMinecraftDocumentText = (
  content: React.ReactNode
): content is string => typeof content === "string";

/** Text scalars concatenated by label renderers, including React bigint children. */
export const isMinecraftTextScalar = (
  content: React.ReactNode
): content is string | number | bigint =>
  isMinecraftTextContent(content) || typeof content === "bigint";

/** React children omitted while collecting visible label or message content. */
export const isMinecraftEmptyChild = (
  child: React.ReactNode
): child is null | undefined | boolean =>
  child === null || child === undefined || typeof child === "boolean";

/** Transparent inline containers whose children the bitmap labels may adapt. */
export const isMinecraftInlineTextElement = (
  value: React.ReactNode
): value is React.ReactElement<
  { children?: React.ReactNode },
  | "span"
  | "a"
  | "strong"
  | "em"
  | "b"
  | "i"
  | "u"
  | "s"
  | "small"
  | "code"
  | "mark"
  | "abbr"
  | "sub"
  | "sup"
  | typeof React.Fragment
> =>
  React.isValidElement<{ children?: React.ReactNode }>(value) &&
  (value.type === React.Fragment ||
    (typeof value.type === "string" &&
      [
        "span",
        "a",
        "strong",
        "em",
        "b",
        "i",
        "u",
        "s",
        "small",
        "code",
        "mark",
        "abbr",
        "sub",
        "sup",
      ].includes(value.type)));

/** Selectable code text preserves the narrower supported formatting vocabulary. */
export const isMinecraftSelectableTextElement = (
  value: React.ReactNode
): value is React.ReactElement<
  { children?: React.ReactNode },
  "span" | "a" | "strong" | "em" | "code" | "b" | "i" | typeof React.Fragment
> =>
  React.isValidElement<{ children?: React.ReactNode }>(value) &&
  (value.type === React.Fragment ||
    (typeof value.type === "string" &&
      ["span", "a", "strong", "em", "code", "b", "i"].includes(value.type)));

/** Only intrinsic anchors and buttons own activation audio at this adapter boundary. */
export const isMinecraftNativeActivationElement = (
  element: React.ReactElement
): element is React.ReactElement<unknown, "a" | "button"> =>
  typeof element.type === "string" && ["a", "button"].includes(element.type);

/** A custom tooltip renderer may expose its direct literal text for native wrapping. */
export const isMinecraftTooltipTextElement = (
  value: React.ReactNode
): value is React.ReactElement<{ children: string }> =>
  React.isValidElement<{ children?: React.ReactNode }>(value) &&
  typeof value.props.children === "string";

export type MinecraftRenderProp = useRender.RenderProp<object>;

/** Preserve each SDK callback signature while separating renderers from opaque React content. */
export const isMinecraftRenderCallback = <
  Renderer extends (...args: never[]) => React.ReactNode,
>(
  content: React.ReactNode | Renderer
): content is Renderer => typeof content === "function";

export type MinecraftProps<T extends React.ElementType> =
  React.ComponentPropsWithRef<T>;

export type MinecraftPolymorphicProps<T extends React.ElementType> =
  useRender.ComponentProps<T>;

/** Base UI composition for Minecraft's non-primitive artwork components. */
export const MinecraftRender = function MinecraftRender<
  T extends keyof React.JSX.IntrinsicElements = "div",
>({
  as,
  render,
  ref,
  ...props
}: Omit<React.ComponentPropsWithRef<T>, "render" | "ref"> & {
  ref?: React.ComponentPropsWithRef<T>["ref"] | React.Ref<Element>[];
  as?: T;
  render?: MinecraftRenderProp;
}) {
  return useRender<Record<string, never>, Element>({
    defaultTagName: as ?? "div",
    props: mergeProps({}, props),
    ref,
    render,
  });
};

type StateClass<State> = ClassValue | ((state: State) => ClassValue);

/** Distinguish Base UI's state-dependent class props from static class values. */
export const isMinecraftClassResolver = <State,>(
  value: StateClass<State>
): value is (state: State) => ClassValue => typeof value === "function";

export function minecraftClasses(...values: ClassValue[]): string;
export function minecraftClasses<State>(
  ...values: StateClass<State>[]
): string | ((state: State) => string);
export function minecraftClasses<State>(...values: StateClass<State>[]) {
  const resolve = (state: State) =>
    mergeClasses(
      ...values.map((value) =>
        isMinecraftClassResolver(value) ? value(state) : value
      )
    );

  const staticValues: ClassValue[] = [];

  for (const value of values) {
    if (isMinecraftClassResolver(value)) {
      return resolve;
    }

    staticValues.push(value);
  }

  return mergeClasses(...staticValues);
}

type StateStyle<State> =
  | React.CSSProperties
  | ((state: State) => React.CSSProperties | undefined)
  | undefined;

/** Resolve the state callback only when the caller supplied one. */
export const isMinecraftStyleResolver = <State,>(
  style: StateStyle<State>
): style is (state: State) => React.CSSProperties | undefined =>
  typeof style === "function";

export function minecraftStyle(
  style: React.CSSProperties | undefined,
  paint: (style: React.CSSProperties | undefined) => React.CSSProperties
): React.CSSProperties;
export function minecraftStyle<State>(
  style:
    | React.CSSProperties
    | ((state: State) => React.CSSProperties | undefined)
    | undefined,
  paint: (style: React.CSSProperties | undefined) => React.CSSProperties
): React.CSSProperties | ((state: State) => React.CSSProperties);
export function minecraftStyle<State>(
  style:
    | React.CSSProperties
    | ((state: State) => React.CSSProperties | undefined)
    | undefined,
  paint: (style: React.CSSProperties | undefined) => React.CSSProperties
) {
  return isMinecraftStyleResolver(style)
    ? (state: State) => paint(style(state))
    : paint(style);
}
