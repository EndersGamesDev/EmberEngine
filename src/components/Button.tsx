"use client";

import type { ButtonHTMLAttributes, ReactNode, Ref } from "react";

export type ButtonVariant = "primary" | "secondary" | "ghost";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  children: ReactNode;
  ref?: Ref<HTMLButtonElement>;
}

/**
 * Grundform aller Schaltflächen: Display-Schrift, dicke Kontur, harter
 * Schatten, der beim Drücken zusammenfällt. Mindesthöhe 48 px (Abschnitt 6.4).
 */
const BASE_CLASSES = [
  "font-display border-ink inline-flex min-h-12 cursor-pointer items-center justify-center",
  "rounded-xl border-[3px] px-5 py-2 text-lg tracking-wide",
  "transition-[transform,box-shadow] duration-100",
  "focus-visible:outline-ink focus-visible:outline-4 focus-visible:outline-offset-2",
  "disabled:cursor-not-allowed disabled:opacity-45",
].join(" ");

/** Nur im aktivierten Zustand: Schatten und Versatz beim Drücken. */
const ENABLED_CLASSES = [
  "shadow-[4px_4px_0_0_var(--color-ink)]",
  "active:translate-x-[3px] active:translate-y-[3px] active:shadow-none",
].join(" ");

const VARIANT_CLASSES: Record<ButtonVariant, string> = {
  primary: "bg-accent text-ink",
  secondary: "bg-card text-ink",
  ghost: "bg-transparent text-ink underline decoration-[3px] underline-offset-4",
};

const GHOST_ENABLED_CLASSES = "active:translate-x-[3px] active:translate-y-[3px]";

export function Button({ variant = "primary", className, ...rest }: ButtonProps) {
  const disabled = rest.disabled === true;
  const depth = variant === "ghost" ? GHOST_ENABLED_CLASSES : ENABLED_CLASSES;
  const classes = [BASE_CLASSES, VARIANT_CLASSES[variant], disabled ? "" : depth, className ?? ""]
    .filter((part) => part !== "")
    .join(" ");

  return <button type="button" {...rest} className={classes} />;
}
