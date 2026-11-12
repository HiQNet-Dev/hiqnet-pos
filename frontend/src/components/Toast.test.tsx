import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Toast } from "./Toast";

describe("Toast", () => {
  it("anuncia el mensaje y expone una acción y un cierre accesibles", async () => {
    const onAction = vi.fn();
    const onDismiss = vi.fn();
    render(
      <Toast
        message="Línea eliminada: Café"
        actionLabel="Deshacer"
        onAction={onAction}
        onDismiss={onDismiss}
      />,
    );

    const region = document.querySelector(".toast-region");
    expect(region).toHaveAttribute("aria-live", "polite");
    expect(screen.getByRole("status")).toHaveTextContent(
      "Línea eliminada: Café",
    );

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Deshacer" }));
    expect(onAction).toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Descartar mensaje" }));
    expect(onDismiss).toHaveBeenCalled();
  });

  it("mantiene la región viva aunque no haya mensaje", () => {
    render(<Toast message={null} onDismiss={() => {}} />);
    expect(document.querySelector(".toast-region")).toHaveAttribute(
      "aria-live",
      "polite",
    );
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });
});
