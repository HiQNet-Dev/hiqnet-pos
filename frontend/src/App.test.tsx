import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Actor, AppStatus, Product } from "./contracts";

const api = vi.hoisted(() => ({
  status: vi.fn(),
  bootstrap: vi.fn(),
  login: vi.fn(),
  logout: vi.fn(),
  currentSession: vi.fn(),
  createProduct: vi.fn(),
  listProducts: vi.fn(),
  listAuditEvents: vi.fn(),
  commercialState: vi.fn(),
  backupDatabase: vi.fn(),
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
  permissions: ["catalog.manage", "sales.sell", "audit.view", "backups.manage"],
};

const product: Product = {
  id: "p-1",
  sku: "CAF-001",
  name: "Café americano",
  unit_price: "12.50",
  tax_rate_bp: 1600,
  active: true,
};

describe("App", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.currentSession.mockResolvedValue(null);
    api.listProducts.mockResolvedValue([]);
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

    expect(
      await screen.findByRole("heading", { name: /entrar a la caja/i }),
    ).toBeVisible();
  });

  it("lista el catálogo después de autenticarse", async () => {
    api.status.mockResolvedValue({
      initialized: true,
      product_version: "0.1.0",
      commercial,
      user_count: 1,
    });
    api.login.mockResolvedValue(owner);
    api.listProducts.mockResolvedValue([product]);

    render(<App />);

    const user = userEvent.setup();
    await user.type(await screen.findByLabelText(/usuario/i), "propietario");
    await user.type(screen.getByLabelText(/contraseña/i), "contrasena-segura");
    await user.click(screen.getByRole("button", { name: /entrar/i }));

    expect(await screen.findByRole("cell", { name: "CAF-001" })).toBeVisible();
    expect(screen.getByRole("cell", { name: /café americano/i })).toBeVisible();
    expect(screen.getByText(/\$12\.50/)).toBeVisible();
  });

  it("muestra el aviso de solo lectura cuando la licencia venció", async () => {
    api.status.mockResolvedValue({
      initialized: true,
      product_version: "0.1.0",
      commercial: { ...commercial, status: "read_only", read_only: true },
      user_count: 1,
    });
    api.login.mockResolvedValue(owner);

    render(<App />);

    const user = userEvent.setup();
    await user.type(await screen.findByLabelText(/usuario/i), "propietario");
    await user.type(screen.getByLabelText(/contraseña/i), "contrasena-segura");
    await user.click(screen.getByRole("button", { name: /entrar/i }));

    await waitFor(() => {
      expect(screen.getByRole("status")).toHaveTextContent(
        /licencia está vencida/i,
      );
    });
    expect(
      screen.getByRole("button", { name: /agregar producto/i }),
    ).toBeDisabled();
  });
});
