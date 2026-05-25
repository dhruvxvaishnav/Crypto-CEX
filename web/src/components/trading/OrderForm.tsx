"use client";

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { SendHorizontal } from "lucide-react";
import { useEffect, useState } from "react";
import { useForm } from "react-hook-form";

import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { ApiError } from "@/lib/api-client";
import { placeOrder } from "@/lib/trading-api";
import { zodResolver } from "@/lib/zod-resolver";
import type { Order, PlaceOrderInput } from "@/types/api.types";

import type { OrderFormProps, OrderFormValues } from "./OrderForm.types";
import {
  buildPlaceOrderPayload,
  createBrowserClientOrderId,
  DEFAULT_ORDER_FORM_VALUES,
  ORDER_TYPE_OPTIONS,
  orderFormSchema,
  requiresPrice,
  requiresQuoteQuantity,
  requiresStopPrice,
} from "./order-form-model";

interface OrderMutationContext {
  previousOrders: Order[] | undefined;
}

export function OrderForm({
  market,
  symbol,
  side,
  formId,
  onSideChange,
  onPlaced,
}: OrderFormProps) {
  const queryClient = useQueryClient();
  const [serverMessage, setServerMessage] = useState<string | null>(null);
  const form = useForm<OrderFormValues>({
    resolver: zodResolver(orderFormSchema),
    defaultValues: { ...DEFAULT_ORDER_FORM_VALUES, side },
  });
  const orderType = form.watch("orderType");
  const selectedSide = form.watch("side");
  const isMarketBuy = requiresQuoteQuantity({ side: selectedSide, orderType });
  const showIceberg = orderType === "limit";

  useEffect(() => {
    form.setValue("side", side, { shouldDirty: true, shouldValidate: true });
  }, [form, side]);

  const mutation = useMutation<Order, Error, PlaceOrderInput, OrderMutationContext>({
    mutationFn: placeOrder,
    async onMutate(payload) {
      await queryClient.cancelQueries({ queryKey: ["orders", symbol] });
      const previousOrders = queryClient.getQueryData<Order[]>(["orders", symbol]);
      queryClient.setQueryData<Order[]>(["orders", symbol], (current) => [
        optimisticOrder(payload),
        ...(current ?? []),
      ]);
      return { previousOrders };
    },
    onSuccess(order) {
      setServerMessage(null);
      onPlaced(order);
      queryClient.setQueryData<Order[]>(["orders", symbol], (current) =>
        (current ?? []).map((row) => (row.clientOrderId === order.clientOrderId ? order : row)),
      );
      form.reset({ ...DEFAULT_ORDER_FORM_VALUES, side: selectedSide });
      void queryClient.invalidateQueries({ queryKey: ["orders", symbol] });
      void queryClient.invalidateQueries({ queryKey: ["balances"] });
    },
    onError(err, _payload, context) {
      queryClient.setQueryData<Order[]>(["orders", symbol], context?.previousOrders);
      if (err instanceof ApiError) {
        setServerMessage(err.message);
        return;
      }
      setServerMessage("Order rejected. Please try again.");
    },
  });

  async function onSubmit(values: OrderFormValues) {
    if (!market) return;
    setServerMessage(null);
    const payload = buildPlaceOrderPayload(values, market, createBrowserClientOrderId);
    mutation.mutate(payload);
  }

  function setSide(nextSide: OrderFormValues["side"]) {
    onSideChange(nextSide);
    form.setValue("side", nextSide, { shouldDirty: true, shouldValidate: true });
  }

  return (
    <section className="flex min-h-0 flex-col border-b border-zinc-800 bg-zinc-950">
      <div className="flex h-10 items-center justify-between border-b border-zinc-800 px-3">
        <h2 className="text-sm font-semibold text-zinc-100">Order entry</h2>
        <span className="text-[11px] text-zinc-500">{symbol}</span>
      </div>

      <form
        id={formId}
        onSubmit={form.handleSubmit(onSubmit)}
        noValidate
        className="flex min-h-0 flex-col gap-3 overflow-y-auto p-3"
      >
        <div className="grid grid-cols-2 gap-2">
          <Button
            type="button"
            variant={selectedSide === "buy" ? "primary" : "secondary"}
            onClick={() => setSide("buy")}
          >
            Buy
          </Button>
          <Button
            type="button"
            variant={selectedSide === "sell" ? "danger" : "secondary"}
            onClick={() => setSide("sell")}
          >
            Sell
          </Button>
        </div>

        <label className="flex flex-col gap-1">
          <span className="text-xs font-medium text-zinc-400">Type</span>
          <select
            className="h-9 rounded-md border border-zinc-700 bg-zinc-900 px-3 text-sm text-zinc-100 transition-colors focus:outline-none focus:ring-2 focus:ring-emerald-500 focus:ring-offset-1 focus:ring-offset-zinc-950"
            {...form.register("orderType")}
          >
            {ORDER_TYPE_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>

        {requiresPrice(orderType) && (
          <Input
            label="Price"
            inputMode="decimal"
            placeholder={market?.lastPrice ?? "0.00"}
            error={form.formState.errors.price?.message}
            {...form.register("price")}
          />
        )}

        {requiresStopPrice(orderType) && (
          <Input
            label="Stop price"
            inputMode="decimal"
            placeholder={market?.lastPrice ?? "0.00"}
            error={form.formState.errors.stopPrice?.message}
            {...form.register("stopPrice")}
          />
        )}

        {isMarketBuy ? (
          <Input
            id="order-form-quantity"
            label={`Spend ${market?.quoteAsset ?? "quote"}`}
            inputMode="decimal"
            placeholder="0.00"
            error={form.formState.errors.quoteQuantity?.message}
            {...form.register("quoteQuantity")}
          />
        ) : (
          <Input
            id="order-form-quantity"
            label={`Quantity ${market?.baseAsset ?? "base"}`}
            inputMode="decimal"
            placeholder="0.00"
            error={form.formState.errors.quantity?.message}
            {...form.register("quantity")}
          />
        )}

        {showIceberg && (
          <label className="flex items-center justify-between gap-3 rounded-md border border-zinc-800 bg-zinc-900/60 px-3 py-2">
            <span className="text-sm text-zinc-300">Iceberg</span>
            <input
              type="checkbox"
              className="h-4 w-4 accent-emerald-500"
              {...form.register("iceberg")}
            />
          </label>
        )}

        {showIceberg && form.watch("iceberg") && (
          <Input
            label="Display quantity"
            inputMode="decimal"
            placeholder="0.00"
            error={form.formState.errors.displayQuantity?.message}
            {...form.register("displayQuantity")}
          />
        )}

        {serverMessage && (
          <p
            role="alert"
            className="rounded-md border border-rose-800 bg-rose-950/60 px-3 py-2 text-xs text-rose-300"
          >
            {serverMessage}
          </p>
        )}

        <Button
          type="submit"
          isLoading={mutation.isPending || form.formState.isSubmitting}
          disabled={market ? undefined : true}
          className="mt-1 w-full"
        >
          <SendHorizontal className="h-4 w-4" aria-hidden />
          Place order
        </Button>
      </form>
    </section>
  );
}

function optimisticOrder(payload: PlaceOrderInput): Order {
  return {
    id: payload.clientOrderId,
    clientOrderId: payload.clientOrderId,
    market: payload.market,
    side: payload.side,
    type: payload.type,
    status: "pending",
    price: payload.price ?? null,
    stopPrice: payload.stopPrice ?? null,
    quantity: payload.quantity ?? null,
    quoteQuantity: payload.quoteQuantity ?? null,
    displayQuantity: payload.displayQuantity ?? null,
    filledQuantity: "0",
    avgFillPrice: null,
    fills: [],
    createdAt: "",
    updatedAt: "",
  };
}
