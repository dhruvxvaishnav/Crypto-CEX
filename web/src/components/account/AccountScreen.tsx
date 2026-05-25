"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { KeyRound, QrCode, ShieldCheck, Trash2, UserRound } from "lucide-react";
import Image from "next/image";
import type { FormEventHandler, InputHTMLAttributes, ReactNode } from "react";
import { useState } from "react";
import { type UseFormRegisterReturn, useForm } from "react-hook-form";
import { z } from "zod";

import { AccountShell } from "@/components/account/AccountShell";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import {
  createApiKey,
  disableTotp,
  getProfile,
  listApiKeys,
  revokeApiKey,
  setupTotp,
  verifyTotp,
} from "@/lib/account-api";
import { ApiError } from "@/lib/api-client";
import { zodResolver } from "@/lib/zod-resolver";
import type { ApiKey, ApiKeyPermission, CreatedApiKey, TotpSetupResponse } from "@/types/api.types";

const API_KEY_SCHEMA = z.object({
  label: z.string().trim().min(1, "Label is required").max(64, "Use 64 characters or fewer"),
  read: z.boolean(),
  trade: z.boolean(),
});

const TOTP_VERIFY_SCHEMA = z.object({
  code: z.string().regex(/^\d{6}$/, "Enter a 6-digit code"),
});

const TOTP_DISABLE_SCHEMA = z.object({
  code: z.string().regex(/^\d{6}$/, "Enter a 6-digit code"),
  password: z.string().min(1, "Password is required"),
});

type ApiKeyValues = z.infer<typeof API_KEY_SCHEMA>;
type TotpVerifyValues = z.infer<typeof TOTP_VERIFY_SCHEMA>;
type TotpDisableValues = z.infer<typeof TOTP_DISABLE_SCHEMA>;

export function AccountScreen() {
  const queryClient = useQueryClient();
  const [setup, setSetup] = useState<TotpSetupResponse | null>(null);
  const [createdKey, setCreatedKey] = useState<CreatedApiKey | null>(null);
  const [backupCodes, setBackupCodes] = useState<string[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const profileQuery = useQuery({ queryKey: ["profile"], queryFn: getProfile });
  const apiKeysQuery = useQuery({ queryKey: ["api-keys"], queryFn: listApiKeys });
  const profile = profileQuery.data ?? null;

  const apiKeyForm = useForm<ApiKeyValues>({
    resolver: zodResolver(API_KEY_SCHEMA),
    defaultValues: { label: "", read: true, trade: false },
  });
  const verifyForm = useForm<TotpVerifyValues>({
    resolver: zodResolver(TOTP_VERIFY_SCHEMA),
    defaultValues: { code: "" },
  });
  const disableForm = useForm<TotpDisableValues>({
    resolver: zodResolver(TOTP_DISABLE_SCHEMA),
    defaultValues: { code: "", password: "" },
  });

  const setupMutation = useMutation({
    mutationFn: setupTotp,
    onSuccess(data) {
      setSetup(data);
      setMessage(null);
    },
    onError(error) {
      setMessage(error instanceof ApiError ? error.message : "Unable to start 2FA setup");
    },
  });
  const verifyMutation = useMutation({
    mutationFn: verifyTotp,
    onSuccess(data) {
      setBackupCodes(data.backupCodes);
      setSetup(null);
      verifyForm.reset({ code: "" });
      setMessage("Two-factor authentication enabled");
      void queryClient.invalidateQueries({ queryKey: ["profile"] });
    },
    onError(error) {
      setMessage(error instanceof ApiError ? error.message : "Unable to verify code");
    },
  });
  const disableMutation = useMutation({
    mutationFn: disableTotp,
    onSuccess() {
      disableForm.reset({ code: "", password: "" });
      setBackupCodes([]);
      setMessage("Two-factor authentication disabled");
      void queryClient.invalidateQueries({ queryKey: ["profile"] });
    },
    onError(error) {
      setMessage(error instanceof ApiError ? error.message : "Unable to disable 2FA");
    },
  });
  const createKeyMutation = useMutation({
    mutationFn: createApiKey,
    onSuccess(data) {
      setCreatedKey(data);
      apiKeyForm.reset({ label: "", read: true, trade: false });
      void queryClient.invalidateQueries({ queryKey: ["api-keys"] });
    },
    onError(error) {
      setMessage(error instanceof ApiError ? error.message : "Unable to create API key");
    },
  });
  const revokeKeyMutation = useMutation({
    mutationFn: revokeApiKey,
    onSuccess() {
      void queryClient.invalidateQueries({ queryKey: ["api-keys"] });
    },
    onError(error) {
      setMessage(error instanceof ApiError ? error.message : "Unable to revoke API key");
    },
  });

  function submitApiKey(values: ApiKeyValues) {
    const permissions: ApiKeyPermission[] = [
      ...(values.read ? (["read"] as const) : []),
      ...(values.trade ? (["trade"] as const) : []),
    ];
    if (permissions.length === 0) {
      apiKeyForm.setError("read", { message: "Select at least one permission" });
      return;
    }
    setCreatedKey(null);
    createKeyMutation.mutate({ label: values.label.trim(), permissions });
  }

  return (
    <AccountShell>
      <header className="flex flex-col gap-1">
        <h1 className="text-xl font-semibold text-zinc-100">Account</h1>
        <p className="text-sm text-zinc-500">Profile, security, and API access.</p>
      </header>

      {message && <Alert>{message}</Alert>}

      <section className="grid gap-4 xl:grid-cols-[minmax(0,0.9fr)_minmax(420px,1.1fr)]">
        <div className="flex flex-col gap-4">
          <Panel icon={<UserRound className="h-4 w-4" aria-hidden />} title="Profile">
            {profile ? (
              <div className="grid gap-3 text-sm">
                <ProfileRow label="Email" value={profile.email} />
                <ProfileRow label="Status" value={profile.status} />
                <ProfileRow label="KYC level" value={profile.kycLevel.toString()} />
                <ProfileRow label="Email verified" value={profile.emailVerified ? "Yes" : "No"} />
                <ProfileRow label="Created" value={formatDate(profile.createdAt)} />
              </div>
            ) : (
              <p className="text-sm text-zinc-500">Loading profile...</p>
            )}
          </Panel>

          <Panel icon={<ShieldCheck className="h-4 w-4" aria-hidden />} title="Two-factor auth">
            <div className="flex flex-col gap-4">
              <div className="flex items-center justify-between rounded-md border border-zinc-800 bg-zinc-900/50 px-3 py-2">
                <span className="text-sm text-zinc-300">Authenticator app</span>
                <span
                  className={
                    profile?.totpEnabled
                      ? "text-xs font-medium text-emerald-300"
                      : "text-xs font-medium text-zinc-500"
                  }
                >
                  {profile?.totpEnabled ? "Enabled" : "Disabled"}
                </span>
              </div>
              {profile?.totpEnabled ? (
                <form
                  className="flex flex-col gap-3"
                  onSubmit={disableForm.handleSubmit((values) => disableMutation.mutate(values))}
                >
                  <Input
                    label="Authenticator code"
                    inputMode="numeric"
                    maxLength={6}
                    error={disableForm.formState.errors.code?.message}
                    {...disableForm.register("code")}
                  />
                  <Input
                    label="Password"
                    type="password"
                    error={disableForm.formState.errors.password?.message}
                    {...disableForm.register("password")}
                  />
                  <Button type="submit" variant="danger" isLoading={disableMutation.isPending}>
                    Disable 2FA
                  </Button>
                </form>
              ) : (
                <TotpSetupPanel
                  setup={setup}
                  isSettingUp={setupMutation.isPending}
                  isVerifying={verifyMutation.isPending}
                  onSetup={() => setupMutation.mutate()}
                  onVerify={verifyForm.handleSubmit((values) => verifyMutation.mutate(values.code))}
                  registerCode={verifyForm.register("code")}
                  codeError={verifyForm.formState.errors.code?.message}
                />
              )}
              {backupCodes.length > 0 && <BackupCodes codes={backupCodes} />}
            </div>
          </Panel>
        </div>

        <Panel icon={<KeyRound className="h-4 w-4" aria-hidden />} title="API keys">
          <form className="mb-4 grid gap-3" onSubmit={apiKeyForm.handleSubmit(submitApiKey)}>
            <Input
              label="Label"
              placeholder="Trading bot"
              error={apiKeyForm.formState.errors.label?.message}
              {...apiKeyForm.register("label")}
            />
            <div className="flex gap-3">
              <PermissionToggle label="Read" {...apiKeyForm.register("read")} />
              <PermissionToggle label="Trade" {...apiKeyForm.register("trade")} />
            </div>
            {apiKeyForm.formState.errors.read?.message && (
              <p className="text-xs text-rose-400">{apiKeyForm.formState.errors.read.message}</p>
            )}
            <Button type="submit" isLoading={createKeyMutation.isPending}>
              Create key
            </Button>
          </form>
          {createdKey && (
            <div className="mb-4 rounded-md border border-amber-900/70 bg-amber-950/20 p-3">
              <p className="mb-2 text-xs font-medium text-amber-200">Secret shown once</p>
              <code className="block overflow-auto rounded bg-zinc-950 p-2 text-xs text-zinc-100">
                {createdKey.secret}
              </code>
            </div>
          )}
          <ApiKeyList
            keys={apiKeysQuery.data ?? []}
            pendingRevokeId={revokeKeyMutation.variables ?? null}
            onRevoke={(keyId) => revokeKeyMutation.mutate(keyId)}
          />
        </Panel>
      </section>
    </AccountShell>
  );
}

function TotpSetupPanel({
  setup,
  isSettingUp,
  isVerifying,
  onSetup,
  onVerify,
  registerCode,
  codeError,
}: {
  setup: TotpSetupResponse | null;
  isSettingUp: boolean;
  isVerifying: boolean;
  onSetup: () => void;
  onVerify: FormEventHandler<HTMLFormElement>;
  registerCode: UseFormRegisterReturn<"code">;
  codeError: string | undefined;
}) {
  if (!setup) {
    return (
      <Button type="button" variant="secondary" isLoading={isSettingUp} onClick={onSetup}>
        Start setup
      </Button>
    );
  }

  return (
    <form className="flex flex-col gap-3" onSubmit={onVerify}>
      <div className="flex gap-3 rounded-md border border-zinc-800 bg-zinc-900/50 p-3">
        <Image
          src={setup.qr}
          alt="Authenticator QR code"
          width={112}
          height={112}
          unoptimized
          className="rounded bg-white p-1"
        />
        <div className="min-w-0 flex-1">
          <div className="mb-2 flex items-center gap-2 text-xs font-medium text-zinc-300">
            <QrCode className="h-3.5 w-3.5" aria-hidden />
            Manual key
          </div>
          <code className="block break-all text-xs leading-5 text-zinc-500">{setup.secret}</code>
        </div>
      </div>
      <Input
        label="Authenticator code"
        inputMode="numeric"
        maxLength={6}
        error={codeError}
        {...registerCode}
      />
      <Button type="submit" isLoading={isVerifying}>
        Verify and enable
      </Button>
    </form>
  );
}

function BackupCodes({ codes }: { codes: string[] }) {
  return (
    <div className="rounded-md border border-zinc-800 bg-zinc-900/50 p-3">
      <p className="mb-2 text-xs font-medium text-zinc-300">Backup codes</p>
      <div className="grid grid-cols-2 gap-2">
        {codes.map((code) => (
          <code key={code} className="rounded bg-zinc-950 px-2 py-1 text-xs text-zinc-200">
            {code}
          </code>
        ))}
      </div>
    </div>
  );
}

function ApiKeyList({
  keys,
  pendingRevokeId,
  onRevoke,
}: {
  keys: ApiKey[];
  pendingRevokeId: string | null;
  onRevoke: (keyId: string) => void;
}) {
  if (keys.length === 0) {
    return <p className="rounded-md border border-zinc-800 p-4 text-sm text-zinc-500">No keys.</p>;
  }

  return (
    <div className="overflow-auto">
      <div className="grid min-w-[620px] grid-cols-[1fr_140px_160px_56px] border-b border-zinc-900 py-2 text-xs text-zinc-500">
        <span>Label</span>
        <span>Permissions</span>
        <span>Created</span>
        <span />
      </div>
      {keys.map((key) => (
        <div
          key={key.keyId}
          className="grid min-w-[620px] grid-cols-[1fr_140px_160px_56px] py-2 text-sm"
        >
          <span className="truncate text-zinc-100">{key.label}</span>
          <span className="text-zinc-400">{key.permissions.join(", ")}</span>
          <span className="text-zinc-500">{formatDate(key.createdAt)}</span>
          <span className="flex justify-end">
            <Button
              type="button"
              variant="ghost"
              size="sm"
              aria-label="Revoke API key"
              title="Revoke"
              isLoading={pendingRevokeId === key.keyId}
              onClick={() => onRevoke(key.keyId)}
            >
              <Trash2 className="h-3.5 w-3.5" aria-hidden />
            </Button>
          </span>
        </div>
      ))}
    </div>
  );
}

function Panel({ icon, title, children }: { icon: ReactNode; title: string; children: ReactNode }) {
  return (
    <section className="rounded-md border border-zinc-800 bg-zinc-950 p-4">
      <div className="mb-4 flex items-center gap-2">
        <span className="text-emerald-300">{icon}</span>
        <h2 className="text-sm font-medium text-zinc-100">{title}</h2>
      </div>
      {children}
    </section>
  );
}

function ProfileRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between border-b border-zinc-900 pb-2 last:border-0">
      <span className="text-zinc-500">{label}</span>
      <span className="text-right text-zinc-100">{value}</span>
    </div>
  );
}

function PermissionToggle({
  label,
  ...rest
}: {
  label: string;
} & InputHTMLAttributes<HTMLInputElement>) {
  return (
    <label className="flex h-9 flex-1 items-center gap-2 rounded-md border border-zinc-800 bg-zinc-900 px-3 text-sm text-zinc-300">
      <input
        type="checkbox"
        className="h-4 w-4 rounded border-zinc-700 bg-zinc-950 accent-emerald-500"
        {...rest}
      />
      {label}
    </label>
  );
}

function Alert({ children }: { children: string }) {
  return (
    <p className="rounded-md border border-zinc-800 bg-zinc-900 px-3 py-2 text-sm text-zinc-300">
      {children}
    </p>
  );
}

function formatDate(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return "--";
  return date.toISOString().slice(0, 10);
}
