import type { InputHTMLAttributes } from "react";

export interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  // Allows string | undefined from react-hook-form's formState.errors.x?.message.
  error?: string | undefined;
  hint?: string | undefined;
}
