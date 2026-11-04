/** Utilidades de importes exactos para presentación.
 *
 * Rust es la autoridad del cálculo: aquí solo se suman enteros de centavos ya
 * calculados por el backend, nunca `parseFloat` sobre importes definitivos.
 */

const MONEY_PATTERN = /^\d+(\.\d{1,2})?$/;

/** Convierte "12.50" en 1250 centavos. Devuelve `null` si el texto no es válido. */
export function parseMinor(value: string): number | null {
  const normalized = value.trim();
  if (!MONEY_PATTERN.test(normalized)) return null;
  const [whole, fraction = ""] = normalized.split(".");
  return Number(whole) * 100 + Number(fraction.padEnd(2, "0"));
}

/** Formatea centavos como "12.50" sin símbolo de moneda. */
export function formatMinor(minor: number): string {
  const sign = minor < 0 ? "-" : "";
  const absolute = Math.abs(Math.trunc(minor));
  return `${sign}${Math.floor(absolute / 100)}.${String(absolute % 100).padStart(2, "0")}`;
}

/** Importe con símbolo, listo para mostrar. */
export function displayMoney(value: string): string {
  const minor = parseMinor(value);
  return minor === null ? value : `$${formatMinor(minor)}`;
}

/** Muestra "18.000" como "18" y conserva fracciones reales de existencias. */
export function formatQuantity(value: string): string {
  const numeric = Number(value);
  if (!Number.isFinite(numeric)) return value;
  return String(Number(numeric.toFixed(3)));
}
