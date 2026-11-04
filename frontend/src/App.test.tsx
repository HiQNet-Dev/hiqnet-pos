import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Actor, AppStatus, Product } from "./contracts";

const api = vi.hoisted(() => ({
  status: vi.fn(),
  bootstrap: vi.fn(),
  login: vi.fn(),
  logout: vi.fn(),
  currentSession: vi.fn(),
  branchSummary: vi.fn(),
  listProducts: vi.fn(),
  listCategories: vi.fn(),
  listStockLevels: vi.fn(),
  listCashRegisters: vi.fn(),
  listCashShiftHistory: vi.fn(),
  activeCashShift: vi.fn(),
  listAuditEvents: vi.fn(),
  listModifierGroups: vi.fn(),
  listModifierOptions: vi.fn(),
  listProductVariants: vi.fn(),
  listUsers: vi.fn(),
  changePassword: vi.fn(),
}));

vi.mock("./api", () => ({
  api,
  IpcError: class IpcError extends Error {
    code: string;
    constructor(code: string, message: string) {
      super(message);
      this.code = code;
    }
  },
}));

import App from "./App";

const commercial: AppStatus["commercial"] = {
  plan: "trial",
  status: "trial",
  valid_until: "2026-12-03T00:00:00Z",
  features: ["catalog", "sales"],
  read_only: false,
};

const owner: Actor = {
  user_id: "u-1",
  organization_id: "o-1",
  branch_id: "b-1",
  display_name: "Ana Propietaria",
  role: "owner",
  permissions: [
    "catalog.manage",
    "sales.sell",
    "inventory.view",
    "cash.shift.view",
    "audit.view",
    "backups.manage",
  ],
};

const product: Product = {
  id: "p-1",
  sku: "CAF-001",
  name: "Café americano",
  unit_price: "12.50",
  tax_rate_bp: 1600,
  active: true,
};

async function login(user = owner) {
  api.status.mockResolvedValue({
    initialized: true,
    product_version: "0.1.0",
    commercial,
    user_count: 1,
  });
  api.login.mockResolvedValue(user);

  render(<App />);
  const events = userEvent.setup();
  await events.type(await screen.findByLabelText(/usuario/i), "propietario");
  await events.type(screen.getByLabelText(/contraseña/i), "contrasena-segura");
  await events.click(screen.getByRole("button", { name: /entrar/i }));
  return events;
}

describe("App", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.currentSession.mockResolvedValue(null);
    api.branchSummary.mockResolvedValue({ id: "b-1", name: "Matriz", code: "MX-01" });
    api.listProducts.mockResolvedValue([]);
    api.listCategories.mockResolvedValue([]);
    api.listStockLevels.mockResolvedValue([]);
    api.listProductVariants.mockResolvedValue([]);
    api.listModifierGroups.mockResolvedValue([]);
    api.listModifierOptions.mockResolvedValue([]);
    api.listUsers.mockResolvedValue([]);
    api.listCashRegisters.mockResolvedValue([]);
    api.listCashShiftHistory.mockResolvedValue([]);
    api.activeCashShift.mockResolvedValue(null);
    api.listAuditEvents.mockResolvedValue([]);
  });

  it("muestra la configuración inicial cuando la instalación no está inicializada", async () => {
    api.status.mockResolvedValue({
      initialized: false,
      product_version: "0.1.0",
      commercial: null,
      user_count: 0,
    });

    render(<App />);

    expect(
      await screen.findByRole("heading", { name: /configura tu negocio/i }),
    ).toBeVisible();
    expect(screen.getByLabelText(/nombre del negocio/i)).toBeVisible();
  });

  it("pide sesión cuando la instalación ya existe", async () => {
    api.status.mockResolvedValue({
      initialized: true,
      product_version: "0.1.0",
      commercial,
      user_count: 1,
    });

    render(<App />);

    expect(await screen.findByRole("heading", { name: /entrar a la caja/i })).toBeVisible();
  });

  it("abre el shell en ventas y permite navegar al catálogo", async () => {
    api.listProducts.mockResolvedValue([product]);
    const events = await login();

    const nav = await screen.findByRole("navigation", { name: /navegación principal/i });
    expect(within(nav).getByRole("button", { name: /ventas/i })).toBeVisible();
    expect(within(nav).getByRole("button", { name: /cajas y turnos/i })).toBeVisible();
    expect(within(nav).getByRole("button", { name: /gestión/i })).toBeVisible();
    expect(screen.getByRole("heading", { name: /^ventas$/i })).toBeVisible();
    expect(await screen.findByText("Café americano")).toBeVisible();

    await events.click(within(nav).getByRole("button", { name: /productos/i }));

    expect(await screen.findByRole("cell", { name: "CAF-001" })).toBeVisible();
    expect(screen.getByRole("heading", { name: /productos y catálogo/i })).toBeVisible();
  });

  it("muestra el aviso de solo lectura cuando la licencia venció", async () => {
    api.status.mockResolvedValue({
      initialized: true,
      product_version: "0.1.0",
      commercial: { ...commercial, status: "read_only", read_only: true },
      user_count: 1,
    });
    api.login.mockResolvedValue(owner);
    api.listProducts.mockResolvedValue([product]);

    render(<App />);
    const events = userEvent.setup();
    await events.type(await screen.findByLabelText(/usuario/i), "propietario");
    await events.type(screen.getByLabelText(/contraseña/i), "contrasena-segura");
    await events.click(screen.getByRole("button", { name: /entrar/i }));

    await waitFor(() => {
      expect(screen.getByRole("status")).toHaveTextContent(/licencia está vencida/i);
    });

    const nav = screen.getByRole("navigation", { name: /navegación principal/i });
    await events.click(within(nav).getByRole("button", { name: /productos/i }));
    expect(await screen.findByRole("button", { name: /nuevo producto/i })).toBeDisabled();
  });

  it("oculta secciones que el rol no autoriza", async () => {
    const cashier: Actor = {
      user_id: "u-2",
      organization_id: "o-1",
      branch_id: "b-1",
      display_name: "Luis Cajero",
      role: "cashier",
      permissions: ["sales.sell", "cash.shift.view", "cash.shift.open"],
    };
    await login(cashier);

    const nav = await screen.findByRole("navigation", { name: /navegación principal/i });
    expect(within(nav).getByRole("button", { name: /ventas/i })).toBeVisible();
    expect(within(nav).queryByRole("button", { name: /productos/i })).not.toBeInTheDocument();
    expect(within(nav).queryByRole("button", { name: /gestión/i })).not.toBeInTheDocument();
    expect(within(nav).getByRole("button", { name: /cajas y turnos/i })).toBeVisible();
  });
});
