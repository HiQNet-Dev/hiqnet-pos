import { useState } from "react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { Modal } from "./Modal";
import { handleBackButton } from "../lib/backNavigation";

function Harness() {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button type="button" onClick={() => setOpen(true)}>
        Abrir
      </button>
      {open && (
        <Modal title="Cambiar contraseña" onClose={() => setOpen(false)}>
          <p>Contenido</p>
        </Modal>
      )}
    </>
  );
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("Modal", () => {
  it("abre el diálogo accesible y no altera el estilo del cuerpo", async () => {
    const onClose = vi.fn();
    const { unmount } = render(
      <Modal title="Nueva categoría" onClose={onClose}>
        <p>Contenido</p>
      </Modal>,
    );

    expect(screen.getByRole("dialog", { name: "Nueva categoría" })).toBeInTheDocument();
    // El shell ya recorta el documento: alternar `body.style.overflow` aquí
    // convertía la raíz en un contenedor desplazable con cada diálogo.
    expect(document.body.getAttribute("style")).toBeNull();

    unmount();
    expect(document.body.getAttribute("style")).toBeNull();
  });

  it("devuelve el foco al elemento de origen sin desplazar la pantalla", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    const opener = screen.getByRole("button", { name: "Abrir" });
    await user.click(opener);

    const focus = vi.spyOn(opener, "focus");
    await user.keyboard("{Escape}");

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(focus).toHaveBeenCalledWith({ preventScroll: true });
  });

  it("cierra con Escape", async () => {
    const onClose = vi.fn();
    const user = userEvent.setup();
    render(
      <Modal title="Gasto" onClose={onClose}>
        <p>Contenido</p>
      </Modal>,
    );

    await user.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("el botón Back de Android lo cierra con el mismo cierre que la X", () => {
    const onClose = vi.fn();
    render(
      <Modal title="Gasto" onClose={onClose}>
        <p>Contenido</p>
      </Modal>,
    );

    expect(handleBackButton()).toBe(true);
    expect(onClose).toHaveBeenCalledTimes(1);
    // Consume el gesto una sola vez: no hay doble cierre.
    onClose.mockClear();
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });

  it("deja de consumir Back cuando se desmonta", () => {
    const onClose = vi.fn();
    const { unmount } = render(
      <Modal title="Gasto" onClose={onClose}>
        <p>Contenido</p>
      </Modal>,
    );

    unmount();

    expect(handleBackButton()).toBe(false);
    expect(onClose).not.toHaveBeenCalled();
  });
});
