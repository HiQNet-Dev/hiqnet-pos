import type { SectionDefinition, SectionId } from "./navigation";
import { ChevronIcon } from "./icons";

interface Props {
  sections: SectionDefinition[];
  active: SectionId;
  collapsed: boolean;
  onSelect: (section: SectionId) => void;
  onToggle: () => void;
  cartCount: number;
  version: string;
}

/**
 * El contador se dibuja como píldora y, por encima de 99, se abrevia para no
 * desbordar el riel compacto. El texto completo va en la etiqueta accesible.
 */
function counterText(count: number): string {
  return count > 99 ? "99+" : String(count);
}

function ticketLabel(count: number): string {
  return `${count} ${count === 1 ? "artículo" : "artículos"} en el ticket`;
}

/**
 * Navegación lateral.
 *
 * Tres estados, todos derivados de CSS para no duplicar el estado responsive en
 * JavaScript:
 *
 * - expandida (escritorio): icono + etiqueta completa, contador en línea.
 * - colapsada (escritorio): riel estrecho con iconos centrados, contador sobre
 *   el icono y tooltip con la etiqueta completa al enfocar o pasar el puntero.
 * - riel compacto (≤1080 px): icono + etiqueta corta, sin control de colapso y
 *   sin tooltip, porque la etiqueta ya es visible.
 *
 * El nombre accesible es explícito en los tres estados: no depende de qué
 * etiqueta esté visible.
 */
export function SideNav({
  sections,
  active,
  collapsed,
  onSelect,
  onToggle,
  cartCount,
  version,
}: Props) {
  return (
    <nav className="sidenav" aria-label="Navegación principal">
      <ul className="sidenav-list">
        {sections.map((section) => {
          const showCount = section.id === "sales" && cartCount > 0;
          const accessibleLabel = showCount
            ? `${section.label}, ${ticketLabel(cartCount)}`
            : section.label;

          return (
            <li key={section.id}>
              <button
                type="button"
                className="nav-item"
                data-section={section.id}
                aria-current={active === section.id ? "page" : undefined}
                aria-label={accessibleLabel}
                onClick={() => onSelect(section.id)}
                title={collapsed ? undefined : section.description}
              >
                <section.Icon />
                <span className="nav-label" aria-hidden="true">
                  {section.label}
                </span>
                <span className="nav-label-short" aria-hidden="true">
                  {section.shortLabel}
                </span>
                {showCount && (
                  <span className="nav-count" aria-hidden="true">
                    {counterText(cartCount)}
                  </span>
                )}
                <span className="nav-tooltip" aria-hidden="true">
                  {section.label}
                  {showCount ? ` · ${ticketLabel(cartCount)}` : ""}
                </span>
              </button>
            </li>
          );
        })}
      </ul>

      <div className="sidenav-footer">
        <span className="sidenav-version" aria-hidden="true">
          v{version}
        </span>
        <button
          type="button"
          className="sidenav-toggle"
          onClick={onToggle}
          aria-label={collapsed ? "Expandir navegación" : "Contraer navegación"}
          aria-expanded={!collapsed}
          title={collapsed ? "Expandir navegación" : "Contraer navegación"}
        >
          <ChevronIcon className={collapsed ? "chevron chevron-right" : "chevron"} />
        </button>
      </div>
    </nav>
  );
}
