import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Actor, CommercialState, User } from "../contracts";

const api = vi.hoisted(() => ({
  listUsers: vi.fn(),
  createUser: vi.fn(),
  setUserActive: vi.fn(),
  resetUserPassword: vi.fn(),
  listAuditEvents: vi.fn(),
  backupDatabase: vi.fn(),
  safBackupAvailable: vi.fn(),
  exportSafBackup: vi.fn(),
  importSafBackup: vi.fn(),
  scheduleSafRestore: vi.fn(),
  restoreStartupNotice: vi.fn(),
}));

vi.mock("../api", () => ({
  api,
  IpcError: class IpcError extends Error {
    code: string;
    constructor(code: string, message: string) {
      super(message);
      this.code = code;
    }
  },
}));

import { AdminScreen } from "./AdminScreen";

const commercial: CommercialState = {
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
  permissions: ["users.manage", "audit.view", "backups.manage"],
};

const cashier: User = {
  id: "u-2",
  username: "luis.cajero",
  display_name: "Luis Cajero",
  role: "cashier",
  active: true,
  created_at: "2026-11-04T08:00:00Z",
};

const ownerUser: User = {
  id: "u-1",
  username: "propietario",
  display_name: "Ana Propietaria",
  role: "owner",
  active: true,
  created_at: "2026-11-01T08:00:00Z",
};

function renderScreen(props: Partial<Parameters<typeof AdminScreen>[0]> = {}) {
  render(
    <AdminScreen
      actor={owner}
      commercial={commercial}
      readOnly={false}
      {...props}
    />,
  );
}

describe("AdminScreen · usuarios", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.listUsers.mockResolvedValue([ownerUser, cashier]);
    api.listAuditEvents.mockResolvedValue([]);
    api.createUser.mockResolvedValue({ ...cashier, id: "u-3", username: "nuevo" });
    api.setUserActive.mockResolvedValue({ ...cashier, active: false });
    api.resetUserPassword.mockResolvedValue(null);
    api.safBackupAvailable.mockResolvedValue(false);
    api.restoreStartupNotice.mockResolvedValue(null);
  });

  it("lista las cuentas con rol y estado", async () => {
    renderScreen();

    expect(await screen.findByText("Luis Cajero")).toBeVisible();
    expect(screen.getByText("luis.cajero")).toBeVisible();
    expect(screen.getByText("Cajero")).toBeVisible();
    expect(screen.getByText("Propietario")).toBeVisible();
    expect(screen.getAllByText("Activo").length).toBe(2);
    expect(screen.getByText("Tú")).toBeVisible();
  });

  it("crea un cajero desde el diálogo", async () => {
    renderScreen();
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: /nuevo usuario/i }));
    const dialog = await screen.findByRole("dialog");
    await user.type(within(dialog).getByLabelText(/nombre del empleado/i), "Marta Cajera");
    await user.type(within(dialog).getByLabelText(/^usuario$/i), "marta.cajera");
    await user.selectOptions(within(dialog).getByLabelText(/^rol$/i), "cashier");
    await user.type(within(dialog).getByLabelText(/contraseña inicial/i), "clave-marta-2026");
    await user.click(within(dialog).getByRole("button", { name: /crear cuenta/i }));

    await waitFor(() =>
      expect(api.createUser).toHaveBeenCalledWith({
        username: "marta.cajera",
        display_name: "Marta Cajera",
        role: "cashier",
        password: "clave-marta-2026",
      }),
    );
    expect(await screen.findByText(/cuenta de marta cajera creada/i)).toBeVisible();
  });

  it("explica el error al crear un usuario duplicado", async () => {
    const { IpcError } = await import("../api");
    api.createUser.mockRejectedValueOnce(
      new IpcError("duplicate_username", "el usuario ya existe"),
    );
    renderScreen();
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: /nuevo usuario/i }));
    const dialog = await screen.findByRole("dialog");
    await user.type(within(dialog).getByLabelText(/nombre del empleado/i), "Luis");
    await user.type(within(dialog).getByLabelText(/^usuario$/i), "luis.cajero");
    await user.type(within(dialog).getByLabelText(/contraseña inicial/i), "clave-duplicada");
    await user.click(within(dialog).getByRole("button", { name: /crear cuenta/i }));

    expect(await screen.findByText(/el usuario ya existe/i)).toBeVisible();
  });

  it("desactiva una cuenta tras confirmar", async () => {
    renderScreen();
    const user = userEvent.setup();

    const fila = (await screen.findByText("Luis Cajero")).closest("tr")!;
    await user.click(within(fila).getByRole("button", { name: /^desactivar$/i }));

    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText(/perderá el acceso de inmediato/i)).toBeVisible();
    await user.click(within(dialog).getByRole("button", { name: /^desactivar$/i }));

    await waitFor(() => expect(api.setUserActive).toHaveBeenCalledWith("u-2", false));
    expect(await screen.findByText(/ya no puede iniciar sesión/i)).toBeVisible();
  });

  it("no permite desactivar la cuenta propia", async () => {
    renderScreen();

    const fila = (await screen.findByText("Ana Propietaria")).closest("tr")!;
    expect(within(fila).getByRole("button", { name: /^desactivar$/i })).toBeDisabled();
  });

  it("restablece la contraseña de un empleado", async () => {
    renderScreen();
    const user = userEvent.setup();

    const fila = (await screen.findByText("Luis Cajero")).closest("tr")!;
    await user.click(within(fila).getByRole("button", { name: /^contraseña$/i }));

    const dialog = await screen.findByRole("dialog");
    await user.type(within(dialog).getByLabelText(/nueva contraseña/i), "clave-nueva-2026");
    await user.click(within(dialog).getByRole("button", { name: /^restablecer$/i }));

    await waitFor(() =>
      expect(api.resetUserPassword).toHaveBeenCalledWith("u-2", "clave-nueva-2026"),
    );
    expect(await screen.findByText(/contraseña de luis cajero restablecida/i)).toBeVisible();
  });

  it("oculta la administración de usuarios sin permiso", async () => {
    renderScreen({ actor: { ...owner, permissions: ["audit.view"] } });

    expect(screen.queryByRole("button", { name: /nuevo usuario/i })).not.toBeInTheDocument();
    expect(screen.queryByText("Luis Cajero")).not.toBeInTheDocument();
    expect(api.listUsers).not.toHaveBeenCalled();
  });

  it("bloquea las acciones en modo solo lectura", async () => {
    renderScreen({ readOnly: true });

    expect(await screen.findByRole("button", { name: /nuevo usuario/i })).toBeDisabled();
    const fila = screen.getByText("Luis Cajero").closest("tr")!;
    expect(within(fila).getByRole("button", { name: /^contraseña$/i })).toBeDisabled();
    expect(within(fila).getByRole("button", { name: /^desactivar$/i })).toBeDisabled();
  });

  it("solo muestra SAF cuando Android lo informa disponible", async () => {
    api.safBackupAvailable.mockResolvedValue(true);
    renderScreen();

    expect(await screen.findByRole("button", { name: /exportar respaldo/i })).toBeVisible();
    expect(screen.getByRole("button", { name: /importar respaldo/i })).toBeVisible();
  });

  it("requiere confirmación antes de programar la restauración", async () => {
    api.safBackupAvailable.mockResolvedValue(true);
    api.importSafBackup.mockResolvedValue({
      backup_valid_until: "2026-11-30T00:00:00Z",
      preserves_commercial_state: true,
    });
    renderScreen();
    const user = userEvent.setup();

    await user.click(await screen.findByRole("button", { name: /importar respaldo/i }));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText(/datos posteriores/i)).toBeVisible();
    expect(api.scheduleSafRestore).not.toHaveBeenCalled();

    await user.click(within(dialog).getByRole("button", { name: /restaurar y reiniciar/i }));
    await waitFor(() => expect(api.scheduleSafRestore).toHaveBeenCalledTimes(1));
  });
});
