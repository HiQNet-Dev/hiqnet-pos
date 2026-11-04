import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  changePassword: vi.fn(),
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

import { ChangePasswordDialog } from "./ChangePasswordDialog";

function renderDialog(onChanged = vi.fn(), onClose = vi.fn()) {
  render(<ChangePasswordDialog onChanged={onChanged} onClose={onClose} />);
  return { onChanged, onClose };
}

describe("ChangePasswordDialog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.changePassword.mockResolvedValue(null);
  });

  it("exige que la nueva contraseña se repita igual", async () => {
    renderDialog();
    const user = userEvent.setup();

    await user.type(screen.getByLabelText(/contraseña actual/i), "contrasena-segura");
    await user.type(screen.getByLabelText(/^nueva contraseña$/i), "clave-nueva-2026");
    await user.type(screen.getByLabelText(/repetir contraseña/i), "otra-distinta");

    expect(await screen.findByText(/las contraseñas no coinciden/i)).toBeVisible();
    expect(screen.getByRole("button", { name: /cambiar contraseña/i })).toBeDisabled();
  });

  it("cambia la contraseña y notifica", async () => {
    const { onChanged, onClose } = renderDialog();
    const user = userEvent.setup();

    await user.type(screen.getByLabelText(/contraseña actual/i), "contrasena-segura");
    await user.type(screen.getByLabelText(/^nueva contraseña$/i), "clave-nueva-2026");
    await user.type(screen.getByLabelText(/repetir contraseña/i), "clave-nueva-2026");
    await user.click(screen.getByRole("button", { name: /cambiar contraseña/i }));

    await waitFor(() =>
      expect(api.changePassword).toHaveBeenCalledWith({
        current_password: "contrasena-segura",
        new_password: "clave-nueva-2026",
      }),
    );
    expect(onChanged).toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });

  it("muestra el error cuando la contraseña actual no coincide", async () => {
    const { IpcError } = await import("../api");
    api.changePassword.mockRejectedValueOnce(
      new IpcError("invalid_credentials", "el usuario o la contraseña no son válidos"),
    );
    renderDialog();
    const user = userEvent.setup();

    await user.type(screen.getByLabelText(/contraseña actual/i), "incorrecta");
    await user.type(screen.getByLabelText(/^nueva contraseña$/i), "clave-nueva-2026");
    await user.type(screen.getByLabelText(/repetir contraseña/i), "clave-nueva-2026");
    await user.click(screen.getByRole("button", { name: /cambiar contraseña/i }));

    expect(await screen.findByText(/no son válidos/i)).toBeVisible();
  });

  it("no permite contraseñas cortas", async () => {
    renderDialog();
    const user = userEvent.setup();

    await user.type(screen.getByLabelText(/contraseña actual/i), "contrasena-segura");
    await user.type(screen.getByLabelText(/^nueva contraseña$/i), "corta");
    await user.type(screen.getByLabelText(/repetir contraseña/i), "corta");

    expect(screen.getByRole("button", { name: /cambiar contraseña/i })).toBeDisabled();
  });
});
