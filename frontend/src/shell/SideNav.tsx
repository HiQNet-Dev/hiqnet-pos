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
        {sections.map((section) => (
          <li key={section.id}>
            <button
              type="button"
              className="nav-item"
              data-section={section.id}
              aria-current={active === section.id ? "page" : undefined}
              onClick={() => onSelect(section.id)}
              title={collapsed ? section.label : section.description}
            >
              <section.Icon />
              <span className="nav-label">{section.label}</span>
              <span className="nav-label-short">{section.shortLabel}</span>
              {section.id === "sales" && cartCount > 0 && (
                <span
                  className="nav-count"
                  aria-label={`${cartCount} ${cartCount === 1 ? "artículo" : "artículos"} en el ticket`}
                >
                  {cartCount}
                </span>
              )}
            </button>
          </li>
        ))}
      </ul>

      <div className="sidenav-footer">
        <span className="sidenav-version">v{version}</span>
        <button
          type="button"
          className="sidenav-toggle"
          onClick={onToggle}
          aria-label={collapsed ? "Expandir navegación" : "Contraer navegación"}
          aria-expanded={!collapsed}
        >
          <ChevronIcon className={collapsed ? "chevron chevron-right" : "chevron"} />
        </button>
      </div>
    </nav>
  );
}
