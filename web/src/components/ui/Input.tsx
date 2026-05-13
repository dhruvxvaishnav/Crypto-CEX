"use client";

import { forwardRef, useId } from "react";

import type { InputProps } from "./Input.types";

export const Input = forwardRef<HTMLInputElement, InputProps>(
  ({ label, error, hint, id, className = "", ...rest }, ref) => {
    const generatedId = useId();
    const inputId = id ?? generatedId;
    const errorId = `${inputId}-error`;
    const hintId = `${inputId}-hint`;

    return (
      <div className="flex flex-col gap-1">
        {label && (
          <label
            htmlFor={inputId}
            className="text-xs font-medium text-zinc-400 uppercase tracking-wide"
          >
            {label}
          </label>
        )}
        <input
          ref={ref}
          id={inputId}
          aria-describedby={
            [error ? errorId : null, hint ? hintId : null].filter(Boolean).join(" ") || undefined
          }
          aria-invalid={error ? true : undefined}
          className={[
            "h-9 w-full rounded-md border bg-zinc-900 px-3 text-sm text-zinc-100 placeholder-zinc-500",
            "transition-colors focus:outline-none focus:ring-2 focus:ring-emerald-500 focus:ring-offset-1 focus:ring-offset-zinc-950",
            error ? "border-rose-500 focus:ring-rose-500" : "border-zinc-700 hover:border-zinc-600",
            "disabled:cursor-not-allowed disabled:opacity-50",
            className,
          ].join(" ")}
          {...rest}
        />
        {hint && !error && (
          <p id={hintId} className="text-xs text-zinc-500">
            {hint}
          </p>
        )}
        {error && (
          <p id={errorId} role="alert" className="text-xs text-rose-400">
            {error}
          </p>
        )}
      </div>
    );
  },
);

Input.displayName = "Input";
