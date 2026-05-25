export type TradingHotkeyAction = "buy" | "sell" | "focusOrderForm" | "submitOrder";

export interface TradingHotkeyEvent {
  key: string;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  target: EventTarget | null;
}

export function matchTradingHotkey(event: TradingHotkeyEvent): TradingHotkeyAction | null {
  if (event.metaKey || event.ctrlKey || event.altKey || isEditableHotkeyTarget(event.target)) {
    return null;
  }

  const key = event.key.toLowerCase();
  if (key === "b") return "buy";
  if (key === "s") return "sell";
  if (key === "o") return "focusOrderForm";
  if (key === "enter") return "submitOrder";
  return null;
}

export function isEditableHotkeyTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  const tag = target.tagName.toLowerCase();
  return tag === "input" || tag === "select" || tag === "textarea";
}
