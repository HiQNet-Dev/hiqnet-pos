/**
 * Aritmética exacta de cantidades para vistas previas.
 *
 * La autoridad final sigue siendo Rust. Estas funciones sólo permiten mostrar
 * "20 → 19" sin introducir errores de coma flotante antes de confirmar.
 */

const QUANTITY_PATTERN = /^\d+(\.\d{1,3})?$/;

/** Convierte "1.25" en 1250 milésimas. `null` si el texto no es válido. */
export function parseQuantity(value: string): number | null {
  const normalized = value.trim();
  if (!QUANTITY_PATTERN.test(normalized)) return null;
  const [whole, fraction = ""] = normalized.split(".");
  return Number(whole) * 1000 + Number(fraction.padEnd(3, "0"));
}

/** Formatea milésimas como cantidad legible: "19", "19.5", "19.125". */
export function formatQuantityMinor(minor: number): string {
  const sign = minor < 0 ? "-" : "";
  const absolute = Math.abs(Math.trunc(minor));
  const whole = Math.floor(absolute / 1000);
  const fraction = String(absolute % 1000)
    .padStart(3, "0")
    .replace(/0+$/, "");
  return fraction ? `${sign}${whole}.${fraction}` : `${sign}${whole}`;
}

/** Stock resultante de una merma; `null` si es inválida o supera el stock. */
export function subtractQuantity(stock: string, amount: string): string | null {
  const stockMinor = parseQuantity(stock);
  const amountMinor = parseQuantity(amount);
  if (stockMinor === null || amountMinor === null || amountMinor <= 0)
    return null;
  const result = stockMinor - amountMinor;
  if (result < 0) return null;
  return formatQuantityMinor(result);
}
