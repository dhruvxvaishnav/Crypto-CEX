"use client";

import Link from "next/link";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { ApiError } from "@/lib/api-client";
import { useAuthActions } from "@/lib/auth-actions";
import { zodResolver } from "@/lib/zod-resolver";

const schema = z
  .object({
    email: z.string().email("Enter a valid email address"),
    password: z
      .string()
      .min(12, "Password must be at least 12 characters")
      .regex(/[A-Z]/, "Include at least one uppercase letter")
      .regex(/[0-9]/, "Include at least one number"),
    confirm: z.string(),
  })
  .refine((d) => d.password === d.confirm, {
    message: "Passwords do not match",
    path: ["confirm"],
  });

type FormValues = z.infer<typeof schema>;

export default function SignupPage() {
  const { signup } = useAuthActions();
  const [serverError, setServerError] = useState<string | null>(null);

  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting },
  } = useForm<FormValues>({ resolver: zodResolver(schema) });

  async function onSubmit(values: FormValues) {
    setServerError(null);
    try {
      await signup(values.email, values.password);
    } catch (err) {
      if (err instanceof ApiError) {
        const messages: Record<string, string> = {
          EMAIL_TAKEN: "An account with this email already exists.",
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
      <h1 className="mb-2 text-xl font-semibold text-zinc-100">Create your account</h1>
      <p className="mb-6 text-sm text-zinc-500">
        Trade on the Aether demo exchange. No real funds.
      </p>

      <form onSubmit={handleSubmit(onSubmit)} noValidate className="flex flex-col gap-4">
        <Input
          label="Email"
          type="email"
          autoComplete="email"
          placeholder="you@example.com"
          error={errors.email?.message}
          {...register("email")}
        />

        <Input
          label="Password"
          type="password"
          autoComplete="new-password"
          placeholder="••••••••"
          hint="At least 12 chars, one uppercase, one number"
          error={errors.password?.message}
          {...register("password")}
        />

        <Input
          label="Confirm password"
          type="password"
          autoComplete="new-password"
          placeholder="••••••••"
          error={errors.confirm?.message}
          {...register("confirm")}
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
          Create account
        </Button>
      </form>

      <p className="mt-6 text-center text-xs text-zinc-500">
        Already have an account?{" "}
        <Link
          href="/auth/login"
          className="text-emerald-400 hover:text-emerald-300 transition-colors"
        >
          Sign in
        </Link>
      </p>
    </>
  );
}
