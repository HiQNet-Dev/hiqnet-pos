import type { ModifierConfiguration } from "../contracts";
import { formatMinor, parseMinor } from "./money";

/** Partida del ticket. Los importes provienen de Rust y se conservan como texto. */
export interface CartLine {
  /** Identidad de la configuración: producto + variante + opciones. */
  key: string;
  productId: string;
  variantId: string | null;
  selectedOptionIds: string[];
  name: string;
  variantName: string | null;
  modifiers: string[];
  quantity: number;
  /** Precio unitario con impuesto, calculado por Rust. */
  unitTotal: string;
  /** Impuesto unitario, calculado por Rust. */
  tax: string;
}

export interface CartTotals {
  subtotal: string;
  tax: string;
  total: string;
  subtotalMinor: number;
  taxMinor: number;
  totalMinor: number;
  items: number;
}

export function lineSignature(
  productId: string,
  variantId: string | null,
  optionIds: string[],
): string {
  return [productId, variantId ?? "-", [...optionIds].sort().join(",")].join("|");
}

/** Crea una partida a partir de la configuración validada por Rust. */
export function lineFromConfiguration(
  configuration: ModifierConfiguration,
  quantity: number,
): CartLine {
  const optionIds = configuration.selected_options.map((option) => option.selected_option_id);
  return {
    key: lineSignature(configuration.product_id, configuration.variant_id, optionIds),
    productId: configuration.product_id,
    variantId: configuration.variant_id,
    selectedOptionIds: optionIds,
    name: configuration.product_name,
    variantName: configuration.variant_name,
    modifiers: configuration.selected_options.map((option) => option.name),
    quantity,
    unitTotal: configuration.unit_price,
    tax: configuration.tax,
  };
}

/** Agrega una partida, fusionando configuraciones idénticas. */
export function mergeLine(cart: CartLine[], line: CartLine): CartLine[] {
  const existing = cart.find((item) => item.key === line.key);
  if (!existing) return [...cart, line];
  return cart.map((item) =>
    item.key === line.key
      ? { ...item, quantity: item.quantity + line.quantity }
      : item,
  );
}

export function lineNetMinor(line: CartLine): number {
  const total = parseMinor(line.unitTotal) ?? 0;
  const tax = parseMinor(line.tax) ?? 0;
  return total - tax;
}

export function lineTotalMinor(line: CartLine): number {
  return (parseMinor(line.unitTotal) ?? 0) * line.quantity;
}

export function cartTotals(cart: CartLine[]): CartTotals {
  let netMinor = 0;
  let taxMinor = 0;
  let totalMinor = 0;
  let items = 0;
  for (const line of cart) {
    netMinor += lineNetMinor(line) * line.quantity;
    taxMinor += (parseMinor(line.tax) ?? 0) * line.quantity;
    totalMinor += lineTotalMinor(line);
    items += line.quantity;
  }
  return {
    subtotal: formatMinor(netMinor),
    tax: formatMinor(taxMinor),
    total: formatMinor(totalMinor),
    subtotalMinor: netMinor,
    taxMinor,
    totalMinor,
    items,
  };
}

/** Cambio estimado; `null` si el efectivo escrito no es un importe válido. */
export function estimatedChange(cashReceived: string, totalMinor: number): number | null {
  const received = parseMinor(cashReceived);
  if (received === null) return null;
  return received - totalMinor;
}
