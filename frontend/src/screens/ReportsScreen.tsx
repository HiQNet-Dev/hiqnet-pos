import { useCallback, useEffect, useRef, useState } from "react";
import { api, IpcError } from "../api";
import type { Actor, ReportsView } from "../contracts";

interface Props {
  actor: Actor;
  readOnly: boolean;
}

const DAY_MS = 24 * 60 * 60 * 1000;

function isoDay(offsetDays = 0): string {
  return new Date(Date.now() + offsetDays * DAY_MS).toISOString().slice(0, 10);
}

/** Límites en UTC: el día final se incluye completo (intervalo semiabierto). */
function dayBounds(from: string, to: string): { from: string; to: string } | null {
  const start = Date.parse(`${from}T00:00:00Z`);
  const end = Date.parse(`${to}T00:00:00Z`);
  if (Number.isNaN(start) || Number.isNaN(end) || end < start) return null;
  return {
    from: new Date(start).toISOString(),
    to: new Date(end + DAY_MS).toISOString(),
  };
}

function formatMoney(value: string): string {
  return `$${value}`;
}

export function ReportsScreen({ actor, readOnly }: Props) {
  const canView = actor.permissions.includes("reports.view");
  const [initialRange] = useState(() => {
    const from = isoDay(-6);
    const to = isoDay();
    return { from, to, bounds: dayBounds(from, to) };
  });
  const [from, setFrom] = useState(initialRange.from);
  const [to, setTo] = useState(initialRange.to);
  const [report, setReport] = useState<ReportsView | null>(null);
  const [appliedRange, setAppliedRange] = useState<{ from: string; to: string } | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestRef = useRef(0);
  const resultsOutdated =
    report !== null &&
    (appliedRange === null || appliedRange.from !== from || appliedRange.to !== to);

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    const bounds = dayBounds(from, to);
    if (!bounds) {
      setError("El período no es válido: revisa las fechas.");
      return;
    }
    await loadBounds(bounds, { from, to });
  }

  const loadBounds = useCallback(async (
    bounds: { from: string; to: string },
    range: { from: string; to: string },
  ) => {
    const request = ++requestRef.current;
    setLoading(true);
    setError(null);
    try {
      const nextReport = await api.reportsSummary(bounds);
      if (request !== requestRef.current) return;
      setReport(nextReport);
      setAppliedRange(range);
    } catch (cause) {
      if (request !== requestRef.current) return;
      setError(
        cause instanceof IpcError ? cause.message : "No fue posible calcular los reportes.",
      );
    } finally {
      if (request === requestRef.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!initialRange.bounds) return;
    const timer = window.setTimeout(
      () => void loadBounds(initialRange.bounds!, { from: initialRange.from, to: initialRange.to }),
      0,
    );
    return () => window.clearTimeout(timer);
  }, [initialRange, loadBounds]);

  if (!canView) {
    return (
      <div className="empty-state">
        <p>No tienes permiso para consultar reportes.</p>
      </div>
    );
  }

  return (
    <div className="screen reports-screen">
      <header className="screen-header">
        <div>
          <h1>Reportes</h1>
          <p className="screen-subtitle">
            Período en UTC con día final incluido. Ventas y devoluciones se informan por
            separado; las entradas de caja no se suman como ingresos.
          </p>
        </div>
        <form className="inline-form report-filters" onSubmit={submit}>
          <label className="field field-inline">
            <span>Desde</span>
            <input type="date" value={from} onChange={(event) => setFrom(event.target.value)} />
          </label>
          <label className="field field-inline">
            <span>Hasta</span>
            <input type="date" value={to} onChange={(event) => setTo(event.target.value)} />
          </label>
          <button type="submit" className="button button-quiet" disabled={loading}>
            {loading ? "Calculando…" : "Actualizar"}
          </button>
        </form>
      </header>

      {error && (
        <div className="screen-inline-alert" aria-live="polite">
          <p className="alert alert-error">{error}</p>
        </div>
      )}

      {resultsOutdated && appliedRange && (
        <div className="screen-inline-alert" role="status">
          <p className="alert alert-info">
            Los resultados mostrados corresponden a {appliedRange.from}–{appliedRange.to}. Pulsa
            Actualizar para consultar el período seleccionado.
          </p>
        </div>
      )}

      {!report ? (
        <p className="muted">{loading ? "Calculando reportes…" : "Sin datos para el período."}</p>
      ) : (
        <div className="reports-grid">
          <section className="surface" aria-label="Reporte de ventas">
            <header className="surface-header">
              <div>
                <h2>Ventas</h2>
                <p className="muted">Importes calculados por Rust desde los snapshots de venta.</p>
              </div>
              <span className="badge badge-neutral">{report.sales.count}</span>
            </header>
            <dl className="mini-grid">
              <div><dt>Subtotal</dt><dd className="mono">{formatMoney(report.sales.subtotal)}</dd></div>
              <div><dt>Impuestos</dt><dd className="mono">{formatMoney(report.sales.tax)}</dd></div>
              <div><dt>Ventas brutas</dt><dd className="mono">{formatMoney(report.sales.total)}</dd></div>
              <div><dt>Devoluciones</dt><dd className="mono">{formatMoney(report.sales.returned_total)}</dd></div>
              <div className="metric-strong"><dt>Ventas netas</dt><dd>{formatMoney(report.sales.net_total)}</dd></div>
            </dl>
            {report.sales.products.length === 0 ? (
              <div className="empty-state"><p>Sin ventas en el período.</p></div>
            ) : (
              <div className="table-scroll">
                <table className="data-table">
                  <thead>
                    <tr><th>Producto</th><th>Cantidad</th><th>Neto</th><th>Impuesto</th><th className="align-end">Total</th></tr>
                  </thead>
                  <tbody>
                    {report.sales.products.map((row) => (
                      <tr key={row.product_id}>
                        <td>{row.product_name}</td>
                        <td className="mono">{row.quantity}</td>
                        <td className="mono">{formatMoney(row.net)}</td>
                        <td className="mono">{formatMoney(row.tax)}</td>
                        <td className="mono align-end">{formatMoney(row.total)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </section>

          <section className="surface" aria-label="Reporte de caja">
            <header className="surface-header">
              <div>
                <h2>Caja</h2>
                <p className="muted">Movimientos físicos de efectivo del período.</p>
              </div>
            </header>
            <dl className="mini-grid">
              <div><dt>Turnos abiertos</dt><dd className="mono">{report.cash.shifts_opened}</dd></div>
              <div><dt>Turnos cerrados</dt><dd className="mono">{report.cash.shifts_closed}</dd></div>
              <div><dt>Entradas</dt><dd className="mono">{formatMoney(report.cash.entries)}</dd></div>
              <div><dt>Salidas</dt><dd className="mono">{formatMoney(report.cash.exits)}</dd></div>
              <div><dt>Gastos desde caja</dt><dd className="mono">{formatMoney(report.cash.expenses_from_cash)}</dd></div>
              <div><dt>Reembolsos</dt><dd className="mono">{formatMoney(report.cash.refunds)}</dd></div>
              <div><dt>Diferencias de arqueo</dt><dd className="mono">{formatMoney(report.cash.difference)}</dd></div>
            </dl>
          </section>

          <section className="surface" aria-label="Reporte de compras">
            <header className="surface-header">
              <div>
                <h2>Compras</h2>
                <p className="muted">Solo la mercancía recibida cuenta como entrada.</p>
              </div>
            </header>
            <dl className="mini-grid">
              <div><dt>Órdenes creadas</dt><dd className="mono">{report.purchases.orders_created}</dd></div>
              <div><dt>Pendientes de recibir</dt><dd className="mono">{report.purchases.pending_orders}</dd></div>
              <div><dt>Cantidad recibida</dt><dd className="mono">{report.purchases.received_quantity}</dd></div>
              <div><dt>Costo registrado</dt><dd className="mono">{formatMoney(report.purchases.received_cost)}</dd></div>
            </dl>
          </section>

          <section className="surface" aria-label="Reporte de gastos">
            <header className="surface-header">
              <div>
                <h2>Gastos</h2>
                <p className="muted">Origen de caja y externo se informan por separado.</p>
              </div>
            </header>
            <dl className="mini-grid">
              <div><dt>Total</dt><dd className="mono">{formatMoney(report.expenses.total)}</dd></div>
              <div><dt>Desde caja</dt><dd className="mono">{formatMoney(report.expenses.from_cash)}</dd></div>
              <div><dt>Externos</dt><dd className="mono">{formatMoney(report.expenses.external)}</dd></div>
            </dl>
            {report.expenses.categories.length > 0 && (
              <ul className="compact-list">
                {report.expenses.categories.map((category) => (
                  <li key={category.name}>
                    <span>{category.name}</span>
                    <span className="mono">{formatMoney(category.total)}</span>
                  </li>
                ))}
              </ul>
            )}
          </section>

          <section className="surface" aria-label="Reporte de inventario">
            <header className="surface-header">
              <div>
                <h2>Inventario</h2>
                <p className="muted">Existencias actuales y movimientos del período.</p>
              </div>
            </header>
            <dl className="mini-grid">
              <div><dt>Existencias</dt><dd className="mono">{report.inventory.stock_quantity}</dd></div>
              <div><dt>Entradas</dt><dd className="mono">{report.inventory.inbound}</dd></div>
              <div><dt>Salidas</dt><dd className="mono">{report.inventory.outbound}</dd></div>
              <div><dt>Merma</dt><dd className="mono">{report.inventory.waste}</dd></div>
              <div><dt>Ajustes +</dt><dd className="mono">{report.inventory.adjustment_in}</dd></div>
              <div><dt>Ajustes −</dt><dd className="mono">{report.inventory.adjustment_out}</dd></div>
            </dl>
          </section>

          <p className="muted reports-note">
            {readOnly
              ? "Licencia en solo lectura: los reportes siguen disponibles."
              : "No se informa utilidad ni costo de ventas: no existe una política de costeo aprobada."}
          </p>
        </div>
      )}
    </div>
  );
}
