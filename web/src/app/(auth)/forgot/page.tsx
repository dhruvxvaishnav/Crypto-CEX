"use client";

import { CheckCircle } from "lucide-react";
import Link from "next/link";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { zodResolver } from "@/lib/zod-resolver";

const schema = z.object({
  email: z.string().email("Enter a valid email address"),
});

type FormValues = z.infer<typeof schema>;

export default function ForgotPage() {
  const [sent, setSent] = useState(false);

  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting },
  } = useForm<FormValues>({ resolver: zodResolver(schema) });

  async function onSubmit(_values: FormValues) {
    // Password reset is mocked for the demo — no backend endpoint exists.
    await new Promise<void>((resolve) => setTimeout(resolve, 600));
    setSent(true);
  }

  if (sent) {
    return (
      <div className="flex flex-col items-center gap-4 py-8 text-center">
        <CheckCircle className="h-10 w-10 text-emerald-400" aria-hidden />
        <h2 className="text-lg font-semibold text-zinc-100">Check your inbox</h2>
        <p className="text-sm text-zinc-400">
          If an account exists for that email, a reset link has been sent.
        </p>
        <Link
          href="/login"
          className="text-sm text-emerald-400 hover:text-emerald-300 transition-colors"
        >
          Back to sign in
        </Link>
      </div>
    );
  }

  return (
    <>
      <h1 className="mb-2 text-xl font-semibold text-zinc-100">Reset your password</h1>
      <p className="mb-6 text-sm text-zinc-500">Enter your email and we'll send a reset link.</p>

      <form onSubmit={handleSubmit(onSubmit)} noValidate className="flex flex-col gap-4">
        <Input
          label="Email"
          type="email"
          autoComplete="email"
          placeholder="you@example.com"
          error={errors.email?.message}
          {...register("email")}
        />

        <Button type="submit" isLoading={isSubmitting} className="mt-1 w-full">
          Send reset link
        </Button>
      </form>

      <p className="mt-6 text-center text-xs text-zinc-500">
        <Link href="/login" className="text-emerald-400 hover:text-emerald-300 transition-colors">
          Back to sign in
        </Link>
      </p>
    </>
  );
}
