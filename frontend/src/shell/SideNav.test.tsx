import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { SideNav } from "./SideNav";
import { SECTIONS } from "./navigation";

const sections = SECTIONS.slice(0, 3);

function baseProps() {
  return {
    sections,
    active: "sales" as const,
    collapsed: false,
    onSelect: vi.fn(),
    onToggle: vi.fn(),
    cartCount: 0,
    version: "0.1.0",
  };
}

describe("SideNav", () => {
  it("expone el nombre completo de cada sección en cualquier estado", () => {
    const { unmount } = render(<SideNav {...baseProps()} />);
    for (const section of sections) {
      expect(screen.getByRole("button", { name: section.label })).toBeInTheDocument();
    }
    unmount();

    // Colapsada: la etiqueta ya no es visible, el nombre accesible permanece.
    render(<SideNav {...baseProps()} collapsed />);
    for (const section of sections) {
      expect(screen.getByRole("button", { name: section.label })).toBeInTheDocument();
    }
    expect(screen.getByRole("button", { name: "Ventas" })).toHaveAttribute(
      "aria-current",
      "page",
    );
  });

  it("anuncia el contador del ticket con singular y plural correctos", () => {
    const { unmount } = render(<SideNav {...baseProps()} cartCount={1} />);
    expect(
      screen.getByRole("button", { name: "Ventas, 1 artículo en el ticket" }),
    ).toBeInTheDocument();
    unmount();

    render(<SideNav {...baseProps()} cartCount={4} />);
    expect(
      screen.getByRole("button", { name: "Ventas, 4 artículos en el ticket" }),
    ).toBeInTheDocument();
    expect(screen.getByText("4")).toBeInTheDocument();
  });

  it("abrevia el contador por encima de 99 para no desbordar el riel", () => {
    render(<SideNav {...baseProps()} cartCount={120} />);
    expect(screen.getByText("99+")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Ventas, 120 artículos en el ticket" }),
    ).toBeInTheDocument();
  });

  it("informa el estado del control de colapso expandido", async () => {
    const user = userEvent.setup();
    const onToggle = vi.fn();
    render(<SideNav {...baseProps()} onToggle={onToggle} />);

    const collapse = screen.getByRole("button", { name: "Contraer navegación" });
    expect(collapse).toHaveAttribute("aria-expanded", "true");
    await user.click(collapse);
    expect(onToggle).toHaveBeenCalledTimes(1);
  });

  it("informa el estado del control de colapso contraído", async () => {
    const user = userEvent.setup();
    const onToggle = vi.fn();
    render(<SideNav {...baseProps()} collapsed onToggle={onToggle} />);

    const expand = screen.getByRole("button", { name: "Expandir navegación" });
    expect(expand).toHaveAttribute("aria-expanded", "false");
    await user.click(expand);
    expect(onToggle).toHaveBeenCalledTimes(1);
  });

  it("selecciona la sección al pulsar la entrada", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    render(<SideNav {...baseProps()} onSelect={onSelect} />);

    await user.click(screen.getByRole("button", { name: "Productos" }));
    expect(onSelect).toHaveBeenCalledWith("catalog");
  });
});
