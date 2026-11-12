import { describe, expect, it } from "vitest";
import {
  formatQuantityMinor,
  parseQuantity,
  subtractQuantity,
} from "./quantity";

describe("quantity", () => {
  it("parsea cantidades con hasta tres decimales", () => {
    expect(parseQuantity("20")).toBe(20_000);
    expect(parseQuantity("1.5")).toBe(1_500);
    expect(parseQuantity("0.125")).toBe(125);
    expect(parseQuantity("abc")).toBeNull();
    expect(parseQuantity("1.2345")).toBeNull();
  });

  it("formatea milésimas sin ceros sobrantes", () => {
    expect(formatQuantityMinor(20_000)).toBe("20");
    expect(formatQuantityMinor(19_500)).toBe("19.5");
    expect(formatQuantityMinor(19_125)).toBe("19.125");
  });

  it("calcula el stock resultante sin coma flotante", () => {
    expect(subtractQuantity("20.000", "1")).toBe("19");
    expect(subtractQuantity("1.5", "0.25")).toBe("1.25");
  });

  it("rechaza cantidades inválidas o superiores al stock", () => {
    expect(subtractQuantity("20", "0")).toBeNull();
    expect(subtractQuantity("20", "21")).toBeNull();
    expect(subtractQuantity("20", "abc")).toBeNull();
  });
});
