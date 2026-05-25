import { D } from "@aether/shared/decimal";
import { compareDecimalStrings } from "@/components/trading/order-book-model";
import type { Balance, Market } from "@/types/api.types";

export interface WithdrawValidationInput {
  asset: string;
  amount: string;
  address: string;
}

export interface WithdrawValidationResult {
  amountError: string | null;
  addressError: string | null;
  canSubmit: boolean;
}

export interface PortfolioEstimate {
  totalUsd: string;
  pricedAssetCount: number;
}

interface WithdrawRule {
  minWithdrawal: string;
  pattern: RegExp;
  networkLabel: string;
}

const DECIMAL_RE = /^(?:0|[1-9]\d*)(?:\.\d+)?$/;
const MONEY_DISPLAY_FRACTION_LIMIT = 8;

const WITHDRAW_RULES: Record<string, WithdrawRule> = {
  BTC: {
    minWithdrawal: "0.0001",
    pattern: /^(bc1|[13])[a-zA-HJ-NP-Z0-9]{25,62}$/,
    networkLabel: "BTC",
  },
  ETH: {
    minWithdrawal: "0.001",
    pattern: /^0x[a-fA-F0-9]{40}$/,
    networkLabel: "ERC-20",
  },
  SOL: {
    minWithdrawal: "0.01",
    pattern: /^[1-9A-HJ-NP-Za-km-z]{32,44}$/,
    networkLabel: "Solana",
  },
  USDT: {
    minWithdrawal: "10",
    pattern: /^0x[a-fA-F0-9]{40}$/,
    networkLabel: "ERC-20",
  },
};

export function isPositiveDecimal(value: string): boolean {
  const trimmed = value.trim();
  return DECIMAL_RE.test(trimmed) && compareDecimalStrings(trimmed, "0") > 0;
}

export function validateFaucetInput(asset: string, amount: string): string | null {
  if (asset.trim() === "") return "Asset is required";
  if (!isPositiveDecimal(amount)) return "Amount must be a positive decimal";
  return null;
}

export function validateWithdrawInput(input: WithdrawValidationInput): WithdrawValidationResult {
  const asset = input.asset.toUpperCase();
  const rule = WITHDRAW_RULES[asset] ?? null;
  const amount = input.amount.trim();
  const address = input.address.trim();

  let amountError: string | null = null;
  let addressError: string | null = null;

  if (!isPositiveDecimal(amount)) {
    amountError = "Amount must be a positive decimal";
  } else if (rule && compareDecimalStrings(amount, rule.minWithdrawal) < 0) {
    amountError = `Minimum withdrawal is ${rule.minWithdrawal} ${asset}`;
  }

  if (!rule) {
    addressError = "Unsupported withdrawal asset";
  } else if (!rule.pattern.test(address)) {
    addressError = `Enter a valid ${rule.networkLabel} address`;
  }

  return {
    amountError,
    addressError,
    canSubmit: amountError === null && addressError === null,
  };
}

export function estimatePortfolioValue(balances: Balance[], markets: Market[]): PortfolioEstimate {
  const prices = new Map<string, string>([["USDT", "1"]]);
  for (const market of markets) {
    if (market.quoteAsset === "USDT" && market.lastPrice) {
      prices.set(market.baseAsset, market.lastPrice);
    }
  }

  let total = D("0");
  let pricedAssetCount = 0;
  for (const balance of balances) {
    const price = prices.get(balance.asset);
    if (!price || !isPositiveOrZeroDecimal(balance.total) || !isPositiveDecimal(price)) continue;
    total = total.add(D(balance.total).mul(price));
    pricedAssetCount += compareDecimalStrings(balance.total, "0") > 0 ? 1 : 0;
  }

  return {
    totalUsd: formatDecimal(total.toFixedString()),
    pricedAssetCount,
  };
}

export function formatDecimal(value: string): string {
  const [integer = "0", fraction = ""] = value.split(".");
  if (fraction === "") return integer;
  const trimmed = fraction.slice(0, MONEY_DISPLAY_FRACTION_LIMIT).replace(/0+$/, "");
  return trimmed ? `${integer}.${trimmed}` : integer;
}

export function getWithdrawMinimum(asset: string): string | null {
  return WITHDRAW_RULES[asset.toUpperCase()]?.minWithdrawal ?? null;
}

function isPositiveOrZeroDecimal(value: string): boolean {
  return DECIMAL_RE.test(value.trim());
}
