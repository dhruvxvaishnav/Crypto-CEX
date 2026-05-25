import { describe, expect, it } from "vitest";

import { isEditableHotkeyTarget, matchTradingHotkey } from "../hotkeys";

describe("trading hotkeys", () => {
  it("maps plain keys to trading actions", () => {
    expect(matchTradingHotkey(eventFor("b"))).toBe("buy");
    expect(matchTradingHotkey(eventFor("S"))).toBe("sell");
    expect(matchTradingHotkey(eventFor("o"))).toBe("focusOrderForm");
    expect(matchTradingHotkey(eventFor("Enter"))).toBe("submitOrder");
  });

  it("ignores editable targets", () => {
    const input = document.createElement("input");

    expect(isEditableHotkeyTarget(input)).toBe(true);
    expect(matchTradingHotkey(eventFor("b", input))).toBeNull();
  });

  it("ignores modified key chords", () => {
    expect(matchTradingHotkey({ ...eventFor("b"), metaKey: true })).toBeNull();
  });
});

function eventFor(key: string, target: EventTarget | null = null) {
  return {
    key,
    metaKey: false,
    ctrlKey: false,
    altKey: false,
    target,
  };
}
