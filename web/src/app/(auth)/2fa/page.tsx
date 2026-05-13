"use client";

import { useSearchParams } from "next/navigation";
import { Suspense, useState } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { ApiError } from "@/lib/api-client";
import { useAuthActions } from "@/lib/auth-actions";
import { zodResolver } from "@/lib/zod-resolver";

const schema = z.object({
  code: z
    .string()
    .length(6, "Code must be 6 digits")
    .regex(/^\d{6}$/, "Enter a 6-digit code"),
});

type FormValues = z.infer<typeof schema>;

function TwoFaForm() {
  const params = useSearchParams();
  const mfaToken = params.get("token") ?? "";
  const { verify2fa } = useAuthActions();
  const [serverError, setServerError] = useState<string | null>(null);

  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting },
  } = useForm<FormValues>({ resolver: zodResolver(schema) });

  async function onSubmit(values: FormValues) {
    setServerError(null);
    try {
      await verify2fa(mfaToken, values.code);
    } catch (err) {
      if (err instanceof ApiError) {
        const messages: Record<string, string> = {
          INVALID_TOTP: "Invalid or expired code. Try again.",
          TOO_MANY_REQUESTS: "Too many attempts. Try again later.",
        };
        setServerError(messages[err.code] ?? err.message);
      } else {
        setServerError("Something went wrong. Please try again.");
      }
    }
  }

  return (
    <>
      <h1 className="mb-2 text-xl font-semibold text-zinc-100">Two-factor authentication</h1>
      <p className="mb-6 text-sm text-zinc-500">
        Enter the 6-digit code from your authenticator app.
      </p>

      <form onSubmit={handleSubmit(onSubmit)} noValidate className="flex flex-col gap-4">
        <Input
          label="Authenticator code"
          type="text"
          inputMode="numeric"
          autoComplete="one-time-code"
          placeholder="000000"
          maxLength={6}
          error={errors.code?.message}
          {...register("code")}
        />

        {serverError && (
          <p
            role="alert"
            className="rounded-md bg-rose-950/60 border border-rose-800 px-3 py-2 text-xs text-rose-400"
          >
            {serverError}
          </p>
        )}

        <Button type="submit" isLoading={isSubmitting} className="mt-1 w-full">
          Verify
        </Button>
      </form>
    </>
  );
}

export default function TwoFaPage() {
  return (
    <Suspense fallback={<div className="h-40 animate-pulse rounded-lg bg-zinc-800" />}>
      <TwoFaForm />
    </Suspense>
  );
}
