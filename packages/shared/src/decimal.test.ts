import { describe, expect, it } from "vitest";

import { D } from "./decimal";

describe("Decimal", () => {
  it("keeps arithmetic string-safe", () => {
    const total = D("0.1").add("0.2").mul("10");

    expect(total.toFixedString()).toBe("3");
  });
});
