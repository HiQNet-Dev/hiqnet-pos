---
name: HiQNet POS
description: Punto de venta local-first para escritorio y tablet
colors:
  background: "#f2f4f7"
  surface: "#ffffff"
  surface-muted: "#f8f9fb"
  border: "#e1e5ea"
  border-strong: "#aeb6c2"
  control-border: "#8a93a3"
  text: "#151a22"
  text-muted: "#5a6472"
  accent: "#1d4ed8"
  accent-strong: "#1e40af"
  accent-soft: "#eef2ff"
  danger: "#b42318"
  danger-bg: "#fef3f2"
  success: "#067647"
  success-bg: "#ecfdf3"
  warning: "#93370d"
  warning-bg: "#fffaeb"
typography:
  body:
    fontFamily: "Segoe UI, Inter, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.45
  title:
    fontFamily: "Segoe UI, Inter, system-ui, sans-serif"
    fontSize: "20px"
    fontWeight: 700
    lineHeight: 1.25
  heading:
    fontFamily: "Segoe UI, Inter, system-ui, sans-serif"
    fontSize: "16px"
    fontWeight: 700
    lineHeight: 1.3
  label:
    fontFamily: "Segoe UI, Inter, system-ui, sans-serif"
    fontSize: "12px"
    fontWeight: 600
  data:
    fontFamily: "Cascadia Mono, Consolas, monospace"
    fontSize: "13px"
    fontWeight: 400
rounded:
  sm: "6px"
  md: "10px"
  lg: "14px"
spacing:
  1: "4px"
  2: "8px"
  3: "12px"
  4: "16px"
  5: "24px"
  6: "32px"
components:
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.surface}"
    rounded: "{rounded.sm}"
    padding: "0 14px"
    height: "36px"
  button-primary-hover:
    backgroundColor: "{colors.accent-strong}"
  button-secondary:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.sm}"
    padding: "0 14px"
    height: "36px"
  input:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.sm}"
    padding: "0 10px"
    height: "36px"
  nav-item:
    backgroundColor: "transparent"
    textColor: "{colors.text-muted}"
    rounded: "{rounded.md}"
    padding: "9px 10px"
  nav-item-active:
    backgroundColor: "{colors.accent-soft}"
    textColor: "{colors.accent}"
  surface:
    backgroundColor: "{colors.surface}"
    rounded: "{rounded.lg}"
    padding: "16px"
---

# Design System — HiQNet POS

## Overview

HiQNet POS es una aplicación de trabajo, no una landing page. Su superficie es
**Operate**: la persona está en medio de una tarea y la interfaz debe desaparecer
en ella. La dirección visual es deliberadamente sobria: superficies neutras,
bordes finos, un solo azul institucional para la acción y el estado, y densidad
compacta con jerarquía clara.

La referencia viva es `frontend/src/styles.css`. Los tokens de este documento
son normativos; el código es la fuente de verdad de sus valores.

## Colors

Estrategia **Restrained**: neutros fríos y un acento.

- `accent` se usa solo en la acción principal, la selección actual, enlaces y
  estado activo de navegación. No se usa como decoración.
- `success`, `warning` y `danger` tienen fondos tonales propios y se reservan
  para estados operativos, no para adornar.
- `control-border` (`#8a93a3`) cumple 3:1 contra `surface` y es el borde de
  campos, selectores y controles que necesitan límite visible.
- `border` y `border-strong` son límites de layout, no de control.
- No hay modo oscuro implementado. Si se añade, debe preservar las relaciones de
  contraste, no invertir colores automáticamente.

## Typography

Una sola familia para toda la interfaz: Segoe UI con respaldo en Inter y la
sans del sistema. Es una decisión de producto: el usuario no debe aprender una
voz tipográfica nueva en cada pantalla.

Escala fija, sin `clamp`:

| Rol | Tamaño | Peso | Uso |
| --- | --- | --- | --- |
| `title` | 20 px | 700 | Título de pantalla |
| `heading` | 16 px | 700 | Título de panel o sección |
| `body` | 14 px | 400 | Texto y controles |
| `base` | 13 px | 400/600 | Filas de tabla, texto secundario |
| `label` | 12 px | 600 | Etiquetas de campo, metadatos |
| `micro` | 11 px | 700 | Encabezados de tabla y cifras pequeñas |
| `data` | 13 px | 400 | Cantidades, SKU e identificadores |

Reglas:

- Los importes usan `font-variant-numeric: tabular-nums` y el mono solo para
  datos, nunca como disfraz “técnico”.
- No se usan “eyebrows” ni etiquetas en mayúsculas sobre un título. El título
  carga su propia jerarquía.
- Los encabezados de tabla sí pueden ir en mayúsculas pequeñas con tracking
  porque funcionan como columnas de datos, no como kickers.

## Layout

- Shell de escritorio: barra superior de 56 px, navegación lateral persistente y
  un área de trabajo con scroll interno.
- Navegación lateral, tres estados deliberados:

  | Estado | Ancho | Cuándo | Etiquetas | Control |
  | --- | --- | --- | --- | --- |
  | Expandida | 216 px | Escritorio, preferencia del usuario | Nombre completo | “Contraer navegación” |
  | Colapsada | 72 px | Escritorio, preferencia del usuario | Ninguna; tooltip con el nombre completo al pasar el puntero o enfocar | “Expandir navegación” |
  | Riel compacto | 88 px | ≤ 1080 px | Etiqueta corta bajo el icono | Ninguno: el colapso no tendría efecto |

  El colapso es una transición de layout (la columna del `grid` cambia en
  160 ms) y respeta `prefers-reduced-motion`. El contador del ticket se
  posiciona sobre el icono y se abrevia como `99+` cuando excede dos dígitos.
  La preferencia se guarda por instalación en `localStorage`
  (`hiqnet.nav.collapsed`) y nunca contiene datos personales.
- Barra superior: la identidad y las acciones de cuenta no se comprimen. En
  ventanas estrechas cede primero la etiqueta del chip de caja, después el sello
  del plan (≤ 980 px), el nombre de usuario (≤ 720 px) y el texto del chip de
  caja, que queda como icono con nombre accesible.
- Ventas en escritorio: catálogo a la izquierda y ticket a la derecha
  (360–430 px). El ticket nunca sale de la pantalla; sus totales y “Cobrar” viven
  en un pie fijo.
- Por debajo de 960 px, ventas alterna catálogo y ticket, con barra inferior que
  conserva el total y el cobro.
- Inventario y caja usan una columna principal y un panel lateral en escritorio;
  se apilan por debajo de 960 px.
- Breakpoints: 1360, 1180, 1080, 980, 960 y 720 px. El comportamiento responsive
  es estructural, no tipográfico.
- No hay scroll global: el `body` no scrollea; cada pantalla gestiona su propio
  desbordamiento.
- Los contenedores del shell (`.app`, `.app-body`, `.app-content`) y el `body`
  usan `overflow: clip`, no `hidden`: `hidden` sigue siendo un contenedor
  desplazable y el foco, `scrollIntoView` o el anclaje de scroll podían moverlo,
  desplazando el diseño hacia una esquina y dejando el resto de la ventana vacío.
  Los diálogos no alternan `body.style.overflow` por la misma razón, y devuelven
  el foco con `preventScroll`. Lo vigila `tools/windows-native-e2e/e2e_layout_drift.py`.
- Autenticación: el grupo se centra con márgenes automáticos, no con
  `justify-content: center`, para que con el teclado virtual abierto o una
  ventana baja el formulario se alinee arriba y sus campos sigan siendo
  alcanzables mediante scroll.
- Back de Android: un único listener (`lib/backNavigation.ts`) resuelve por capas
  —teclado del sistema, diálogo, vista de ticket, raíz— y en la raíz consume el
  evento sin finalizar la Activity. Un diálogo se cierra con el mismo `onClose`
  que la X, «Cancelar» y Escape; el Back nunca altera el ticket. Un componente
  nuevo que sea descartable con Escape debe registrar también esa capa.

## Elevation & Depth

Sistema tonal y de bordes, no de sombras decorativas.

- Las superficies se separan por `surface`, `surface-muted` y un borde de 1 px.
- `shadow-1` solo para paneles de autenticación; `shadow-2` para modales.
- Sin halos de color, sin glassmorphism, sin `box-shadow` de desplazamiento duro
  y sin degradados decorativos.
- Los modales viven en un portal, bloquean el fondo y atrapan el foco.

## Shapes

- Radios: 6 px en controles, 10 px en tarjetas y navegación, 14 px en superficies
  grandes y modales.
- Chips, badges y etiquetas usan radio de píldora.
- Los bordes son de 1 px; no se usan bordes laterales de color como decoración.

## Marca

Un solo dibujo de marca en todo el producto: cuadrado redondeado con el acento
`#1d4ed8` y una «H» geométrica en blanco.

- Fuente normativa: `apps/pos-desktop/src-tauri/app-icon.svg` (1024 px) y
  `frontend/public/favicon.svg` (24 px). Los iconos del instalador se generan
  desde ese SVG.
- En la interfaz, `BrandMark` (`frontend/src/shell/icons.tsx`) dibuja el mismo
  glifo con `var(--color-accent)`; se usa en la barra superior y en las pantallas
  de onboarding y de acceso.
- La marca no lleva texto («HQ»): a 16 px el texto deja de ser legible y el
  icono del sistema terminaba sin parecido con la cabecera.
- La marca no se recolorea, no lleva degradados ni sombras y no convive con un
  segundo logotipo.

## Components

- **Botón primario**: fondo `accent`, texto blanco, 36 px de alto; deshabilitado
  con opacidad reducida. El botón de cobro usa 46 px.
- **Botón secundario**: superficie blanca y borde `border-strong`.
- **Botón de peligro**: solo para acciones destructivas confirmadas.
- **Campo**: borde `control-border`, 36 px de alto, etiqueta de 12 px arriba.
- **Tabla**: encabezado fijo, filas de 36–44 px, importes alineados a la derecha,
  identificadores sin partirse.
- **Tarjeta de producto**: nombre, SKU, precio y disponibilidad; la tarjeta
  completa es el objetivo de clic; “Agotado” es un estado explícito.
- **Ticket**: partidas con cantidad editable, total por línea y eliminación
  explícita. La cantidad no baja de 1; para quitar una partida se usa el botón de
  papelera. “Vaciar” exige confirmación.
- **Navegación**: el elemento activo usa `accent-soft` y `accent`; el contador
  del ticket se anuncia con singular o plural correcto. El nombre accesible de
  cada entrada es explícito (`aria-label`), de modo que no depende de qué
  etiqueta esté visible en cada ancho.
- **Búsqueda y filtros**: el campo de búsqueda cede ancho antes que los chips de
  categoría; los chips conservan su tamaño natural y pasan a una fila propia
  cuando no caben junto a la búsqueda, en lugar de quedar cortados a media
  palabra.
- **Acción destructiva secundaria**: `.button-quiet-danger` conserva el peso de
  una acción discreta de fila, pero usa el color de peligro para no confundirse
  con la acción neutra vecina. La confirmación y el botón de peligro pleno viven
  en el diálogo.
- **Auditoría**: cada evento se muestra con una frase en español; la clave
  técnica vive en `data-action` y nunca en pantalla. La meta de cada fila es la
  fecha y una referencia corta del registro, sin repetir el sustantivo que ya
  lleva la acción.
- **Historial lateral**: `.ledger-history` para las listas de dos columnas (texto
  y arqueo). No reutiliza la rejilla de cuatro columnas del libro de movimientos;
  compartir contenedor no significa compartir composición.
- **Estado**: éxito, advertencia, error e información tienen su propio par de
  color y fondo. Un cierre de caja con diferencia es advertencia, no éxito.
- **Carga**: esqueletos en la rejilla de venta; texto breve en paneles pequeños.
- **Error**: mensaje en español que nombra el problema y ofrece reintento cuando
  existe una acción de recuperación.

## Do's and Don'ts

**Do**

- Mantén visibles el total y la acción de cobro en todo momento.
- Deriva permisos, sucursal e importes del `Actor` y de Rust; la UI no los
  inventa.
- Confirma acciones destructivas y bloquea estados inválidos antes de enviar.
- Repite el mismo vocabulario de botones, campos, tablas y modales en todas las
  pantallas.
- Verifica el render real en 1920×1080, 1440×900, 1366×768, 1024×768 y
  768×1024.

**Don't**

- No añadas kickers, eyebrows ni etiquetas en mayúsculas sobre títulos.
- No uses degradados, brillos, glassmorphism ni sombras duras como decoración.
- No ocultes las etiquetas de navegación en tablet: usa la etiqueta corta.
- No dejes el control de colapso visible cuando el ancho lo vuelve inoperante.
- No dejes una etiqueta de navegación visible dentro de la columna colapsada.
- No muestres identificadores técnicos (`sale.completed`, `active`,
  `inventory_movement`) al usuario; tradúcelos.
- No reduzcas el texto a iconos sin nombre accesible.
- No introduzcas un segundo acento cromático sin una razón de estado.
- No dejes que una tabla o un modal desborde la pantalla: usa scroll interno o
  composición responsive.
- No declares una verificación visual sin capturar la pantalla renderizada.
