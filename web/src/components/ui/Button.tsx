"use client";

import { Loader2 } from "lucide-react";
import { forwardRef } from "react";

import type { ButtonProps } from "./Button.types";

const VARIANT_CLASSES: Record<string, string> = {
  primary:
    "bg-emerald-500 text-zinc-950 hover:bg-emerald-400 active:bg-emerald-600 disabled:bg-emerald-900 disabled:text-emerald-600",
  secondary:
    "bg-zinc-800 text-zinc-100 hover:bg-zinc-700 active:bg-zinc-900 disabled:bg-zinc-900 disabled:text-zinc-500",
  ghost:
    "bg-transparent text-zinc-300 hover:bg-zinc-800 hover:text-zinc-100 disabled:text-zinc-600",
  danger:
    "bg-rose-600 text-white hover:bg-rose-500 active:bg-rose-700 disabled:bg-rose-900 disabled:text-rose-600",
};

const SIZE_CLASSES: Record<string, string> = {
  sm: "h-7 px-3 text-xs rounded",
  md: "h-9 px-4 text-sm rounded-md",
  lg: "h-11 px-6 text-base rounded-md",
};

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  (
    { variant = "primary", size = "md", isLoading, disabled, className = "", children, ...rest },
    ref,
  ) => {
    const base =
      "inline-flex items-center justify-center gap-2 font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-emerald-500 focus-visible:ring-offset-2 focus-visible:ring-offset-zinc-950 disabled:cursor-not-allowed select-none";

    return (
      <button
        ref={ref}
        disabled={disabled ?? isLoading}
        className={`${base} ${VARIANT_CLASSES[variant] ?? ""} ${SIZE_CLASSES[size] ?? ""} ${className}`}
        {...rest}
      >
        {isLoading && <Loader2 className="h-4 w-4 animate-spin" aria-hidden />}
        {children}
      </button>
    );
  },
);

Button.displayName = "Button";
