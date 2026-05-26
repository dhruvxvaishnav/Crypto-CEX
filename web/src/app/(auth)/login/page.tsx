"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { ApiError } from "@/lib/api-client";
import { useAuthActions } from "@/lib/auth-actions";
import { zodResolver } from "@/lib/zod-resolver";

const schema = z.object({
  email: z.string().email("Enter a valid email"),
  password: z.string().min(1, "Password is required"),
});

type FormValues = z.infer<typeof schema>;

export default function LoginPage() {
  const { login } = useAuthActions();
  const router = useRouter();
  const [serverError, setServerError] = useState<string | null>(null);

  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting },
  } = useForm<FormValues>({ resolver: zodResolver(schema) });

  async function onSubmit(values: FormValues) {
    setServerError(null);
    try {
      const result = await login(values.email, values.password);
      if (result?.mfaRequired) {
        // Redirect to 2FA page with the mfa_token.
        router.push(`/2fa?token=${encodeURIComponent(result.mfaToken)}`);
      }
      // Otherwise login() already navigated to /trade/BTCUSDT.
    } catch (err) {
      if (err instanceof ApiError) {
        const messages: Record<string, string> = {
          INVALID_CREDENTIALS: "Invalid email or password.",
          ACCOUNT_DISABLED: "This account has been disabled.",
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
      <h1 className="mb-6 text-xl font-semibold text-zinc-100">Sign in to Aether</h1>

      <form onSubmit={handleSubmit(onSubmit)} noValidate className="flex flex-col gap-4">
        <Input
          label="Email"
          type="email"
          autoComplete="email"
          placeholder="you@example.com"
          error={errors.email?.message}
          {...register("email")}
        />

        <div className="flex flex-col gap-1">
          <Input
            label="Password"
            type="password"
            autoComplete="current-password"
            placeholder="••••••••"
            error={errors.password?.message}
            {...register("password")}
          />
          <Link
            href="/forgot"
            className="self-end text-xs text-zinc-500 hover:text-zinc-300 transition-colors"
          >
            Forgot password?
          </Link>
        </div>

        {serverError && (
          <p
            role="alert"
            className="rounded-md bg-rose-950/60 border border-rose-800 px-3 py-2 text-xs text-rose-400"
          >
            {serverError}
          </p>
        )}

        <Button type="submit" isLoading={isSubmitting} className="mt-1 w-full">
          Sign in
        </Button>
      </form>

      <p className="mt-6 text-center text-xs text-zinc-500">
        No account?{" "}
        <Link href="/signup" className="text-emerald-400 hover:text-emerald-300 transition-colors">
          Create one
        </Link>
      </p>
    </>
  );
}
