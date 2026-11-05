/**
 * Evita que la interfaz completa se desplace dentro de la ventana.
 *
 * `overflow: hidden` (y su respaldo cuando el motor no soporta `overflow: clip`)
 * sigue siendo un contenedor desplazable: el foco, `scrollIntoView` o el anclaje
 * de scroll pueden moverlo. Cuando eso ocurre, el diseño se desplaza hacia una
 * esquina y el resto de la ventana queda vacío sin forma de volver.
 *
 * El guardia solo corrige los contenedores raíz. Las zonas con desplazamiento
 * propio (catálogo, tablas, cuerpo de un diálogo) no se tocan.
 */

const ROOT_SELECTORS = ["html", "body", "#root"] as const;

function rootScrollTargets(): HTMLElement[] {
  const targets: (HTMLElement | null | undefined)[] = [
    document.scrollingElement as HTMLElement | null | undefined,
    document.documentElement,
    document.body,
    document.getElementById("root"),
  ];
  return targets.filter((target): target is HTMLElement => Boolean(target));
}

function isRootNode(node: EventTarget | null): boolean {
  if (!(node instanceof HTMLElement)) return false;
  return ROOT_SELECTORS.some((selector) => node.matches(selector));
}

export function installViewportGuard(): () => void {
  if (typeof window === "undefined" || typeof document === "undefined") {
    return () => {};
  }

  let frame = 0;

  const reset = () => {
    frame = 0;
    for (const target of rootScrollTargets()) {
      if (target.scrollTop !== 0) target.scrollTop = 0;
      if (target.scrollLeft !== 0) target.scrollLeft = 0;
    }
  };

  const schedule = () => {
    if (frame !== 0) return;
    frame = window.requestAnimationFrame(reset);
  };

  const onScroll = (event: Event) => {
    // Los desplazamientos legítimos de la pantalla no programan ninguna
    // corrección; solo interesan los de la raíz.
    if (event.target === document || isRootNode(event.target)) schedule();
  };

  const onViewportChange = () => schedule();

  document.addEventListener("scroll", onScroll, true);
  window.addEventListener("resize", onViewportChange);
  window.addEventListener("focusin", onViewportChange);
  // Corrige un estado heredado de un montaje anterior.
  schedule();

  return () => {
    document.removeEventListener("scroll", onScroll, true);
    window.removeEventListener("resize", onViewportChange);
    window.removeEventListener("focusin", onViewportChange);
    if (frame !== 0) {
      window.cancelAnimationFrame(frame);
      frame = 0;
    }
  };
}
