# PliegoCSS — research fundacional

Fecha: 2026-07-12

Estado: **research fundacional histórico**. Sus conclusiones técnicas sobre Rust, Lightning CSS,
zero-runtime y no construir un browser engine permanecen vigentes. La categoría de producto y el
orden de release fueron supersedidos el 2026-07-14 por
[ADR-0015](docs/adr/0015-adopt-verifiable-css-control-layer.md) y el
[contrato estratégico CSS/Rust/IA](docs/product/strategic-product-contract-2026.md): PliegoCSS es
standards-first, `audit` entra antes que una migración y la utility syntax es un frontend opcional.

## Veredicto

Sí es viable crear un motor de estilos específicamente para Rust y PliegoRS, siempre que **motor CSS** signifique un compilador de autoría y no un reemplazo del motor de renderizado del navegador.

PliegoCSS debería aceptar una representación tipada en Rust, validarla y compilarla durante el build a CSS estándar, estático y mínimo. Chrome, Firefox y Safari seguirían resolviendo la cascada, el layout y la pintura.

Reemplazar esa segunda parte exigiría construir o integrar una browser engine. CSS no es una especificación pequeña: el snapshot vigente agrega módulos de cascada, grid, fuentes, color, animaciones, condiciones y muchos otros. Servo muestra la magnitud real: selector matching, cascade, style resolution, box tree, fragment tree, display list y renderer son subsistemas separados.

## Qué existe y qué demuestra

### Tailwind CSS v4

Tailwind ya movió partes sensibles de rendimiento a Rust. Su versión 4 introdujo un motor de alto rendimiento y reporta builds completos hasta cinco veces más rápidos e incrementales por encima de cien veces más rápidos cuando no aparece CSS nuevo. Esto valida Rust para escaneo, generación y transformación, pero Tailwind conserva strings como interfaz de autoría.

### Lightning CSS

Lightning CSS demuestra que Rust puede parsear, transformar, hacer bundling, compatibilidad por browser targets y minificar CSS a velocidad muy alta. Usa cimientos provenientes del ecosistema de Firefox y modela reglas, propiedades y valores en estructuras normalizadas.

No conviene reimplementar esa capa. PliegoCSS puede usarla como backend de compatibilidad y emisión mientras concentra su innovación en el IR tipado, las reglas de composición y la integración con PliegoRS.

### Ecosistema web Rust

Leptos y Dioxus todavía tratan CSS como una capa externa. Dioxus documenta Tailwind mediante npm, un scanner de archivos Rust y strings en `class`. Crates como `stylist`, `stylers` y `leptos_styling` ofrecen CSS-in-Rust, scoped CSS y algunas clases verificadas, pero no presentan un sistema integral de propiedades tipadas, conflictos semánticos, SSR, extracción mínima y procedencia.

Ahí existe un espacio real para PliegoCSS.

## Propuesta

PliegoCSS sería un **compilador de estilos tipados para PliegoRS**:

```text
Rust DSL / macros
       ↓
Pliego Style IR tipado
       ↓
validación + normalización + resolución de conflictos
       ↓
extracción de estilos alcanzables
       ↓
Lightning CSS: targets, prefijos, lowering, minificación
       ↓
CSS estándar + manifest + mapa de procedencia
```

No debería comenzar como un clon de la sintaxis de Tailwind. La superficie natural debe sentirse como Rust:

```rust
let card = style!(
    display::flex(),
    direction::column(),
    gap::token(Space::M),
    padding::token(Space::L),
    background::token(Color::Surface),
    radius::token(Radius::L),
    on::hover((background::token(Color::SurfaceRaised),)),
    at::md((direction::row(),)),
);

view! { <article style=card>...</article> }
```

La macro produciría un identificador estable y el compilador emitiría algo equivalente a:

```css
.p_a31f { display:flex; flex-direction:column; gap:var(--space-m); ... }
@media (min-width:48rem) { .p_a31f { flex-direction:row } }
.p_a31f:hover { background:var(--surface-raised) }
```

## Beneficios para PliegoRS

1. **Errores en compilación.** Propiedades inválidas, unidades incompatibles y tokens inexistentes fallan en `cargo check`, no en el navegador.
2. **Conflictos detectables.** El IR puede rechazar dos valores de la misma propiedad dentro de la misma condición, en vez de depender silenciosamente del orden de clases.
3. **Autocompletado y refactors reales.** Colores, espacios, breakpoints y estados son símbolos Rust; renombrarlos usa el compilador y el IDE.
4. **Cero scanner de strings.** Las dependencias estilísticas nacen del AST/IR de las macros, no de expresiones regulares sobre archivos fuente. Las clases construidas dinámicamente dejan de ser un caso frágil.
5. **CSS mínimo por ruta o island.** PliegoRS conoce el árbol SSR y sus componentes alcanzables. Puede emitir hojas por ruta sin inferir el uso desde texto.
6. **SSR e hidratación deterministas.** Server y cliente comparten IDs derivados del mismo IR. No se generan estilos en runtime ni hay diferencias de orden.
7. **Menos WASM y menos trabajo runtime.** El resultado visual está en CSS estático; el navegador lo procesa nativamente. PliegoRS no debe transportar un parser o runtime de estilos al cliente.
8. **Design system obligatorio por defecto.** Los valores libres pueden existir como escape explícito, pero la ruta normal usa tokens tipados.
9. **Procedencia de estilos.** Un manifest puede mapear cada regla emitida al crate, componente, archivo, línea, token, variante y versión que la originó.
10. **Integración futura con Hyphae.** Temas y tokens pueden versionarse como eventos verificables y compilarse a una proyección CSS. Esto debe ser una fase posterior; no conviene hacer reactivo el CSS base en el MVP.

## Qué no aporta

- No hará que el navegador calcule Flexbox o Grid más rápido: esa tarea sigue dentro del browser.
- No elimina la necesidad de entender CSS, accesibilidad, responsive design o compatibilidad.
- No garantiza automáticamente diseños hermosos.
- No justifica enviar un runtime WASM de estilos. La ganancia principal sucede en build-time y developer experience.
- No debe impedir CSS normal. Sin un escape hatch, el framework quedaría detrás de la evolución del estándar.

## Riesgos

### Cobertura del lenguaje

CSS cambia continuamente y tiene una superficie enorme. Si cada propiedad se modela manualmente, mantener paridad puede consumir al proyecto.

Mitigación: tipar profundamente el núcleo de alto uso y permitir `raw_property!`/CSS externo para el long tail. Generar parte del catálogo desde datos de especificaciones cuando sea posible.

### Tiempos de compilación Rust

Macros procedurales grandes y tipos genéricos muy anidados pueden empeorar `cargo check` y los mensajes de error.

Mitigación: macro con IR compacto, pocos genéricos públicos, cache incremental y compilador separado (`pliego-cssc`) cuando convenga.

### Expresividad y ergonomía

Una DSL excesivamente verbosa perdería contra `class="flex gap-4"`.

Mitigación: medir escritura real, ofrecer presets/composición y diseñar primero diez componentes representativos. El objetivo no es maximizar pureza de tipos, sino velocidad con garantías.

### Valores dinámicos

Posición, progreso, drag y datos visuales cambian en runtime. Generar una clase por valor sería inviable.

Mitigación: CSS custom properties como frontera dinámica:

```rust
style!(width::var(Var::Progress))
```

PliegoRS actualiza `--progress`; la estructura estilística permanece estática.

### Lock-in

Una DSL exclusiva reduce la portabilidad hacia otros frameworks.

Mitigación: output CSS estándar, manifest legible, API de CSS normal y un core separable de `pliego-dom`.

## Arquitectura recomendada

Workspace inicial:

```text
pliego-style-values   tipos: Length, Color, Display, token refs
pliego-style-ir       reglas, condiciones, variantes, source spans
pliego-style-macros   style!, theme!, keyframes!
pliego-style-compiler reachability, dedupe, hashing, conflicts
pliego-style          facade pública para usuarios
pliego-cssc           CLI/build integration y manifest
```

Integración con PliegoRS:

- `pliego-dom` acepta `StyleId`, no strings como camino principal.
- `render_html()` coloca IDs estables durante SSR.
- El build conoce estilos por componente/ruta/island.
- El modo desarrollo soporta hot reload de estilos sin recompilar toda la aplicación, si el IR se serializa hacia el proceso `pliego-cssc`.
- CSS externo y `class` siguen disponibles como interoperabilidad.

## MVP recomendado: seis semanas de gates, no seis semanas prometidas

### P0 — prueba semántica

Implementar 20–30 propiedades: display, flex/grid básico, spacing, sizing, color, border, radius y typography. Incluir `hover`, `focus-visible`, `disabled`, dark mode y un breakpoint.

Gate: construir button, card, navbar, form y dashboard sin CSS manual; propiedades o combinaciones inválidas deben fallar con mensajes claros.

### P1 — compilación estática

IDs deterministas, deduplicación, CSS minificado y manifest de procedencia.

Gate: dos builds idénticos producen bytes idénticos; un estilo compartido aparece una sola vez.

### P2 — PliegoRS SSR

Conectar `StyleId` al árbol de vistas y a `render_html()`.

Gate: SSR e hidratación generan las mismas clases; cero inyección de estilos en runtime.

### P3 — extracción y comparación

CSS por ruta/island y benchmark contra Tailwind v4 en una app equivalente.

Medir:

- tiempo cold e incremental;
- tamaño CSS gzip por ruta y total;
- aumento de `cargo check`;
- tamaño WASM;
- tiempo para implementar diez componentes;
- calidad de errores;
- número de escapes a CSS normal.

## Criterio de decisión

Continuar como producto central si el spike demuestra simultáneamente:

- ergonomía cercana a Tailwind en componentes reales;
- errores sustancialmente mejores;
- CSS y WASM iguales o menores;
- build incremental tolerable;
- SSR/hidratación sin runtime de estilos;
- escape limpio hacia CSS estándar.

Si falla ergonomía, conservar el IR y usar una sintaxis utility-first más compacta. Si falla mantenimiento de tipos, reducir el núcleo tipado y delegar parsing/compatibilidad a Lightning CSS. Si no mejora claramente el flujo frente a Tailwind, PliegoRS debería integrar Tailwind en vez de sostener un framework propio.

## Conclusión

La oportunidad no es “hacer CSS más rápido que el navegador”. La oportunidad es **hacer imposible expresar accidentalmente gran parte del CSS incorrecto**, integrar estilos con el árbol compilado de PliegoRS y entregar al navegador CSS estándar, mínimo y determinista.

Tailwind convirtió CSS en una gramática de strings productiva. PliegoCSS puede convertirlo en un IR de Rust verificable. Esa diferencia es técnicamente viable, encaja con PliegoRS y tiene valor propio, siempre que el proyecto se mantenga como compilador y no derive hacia una browser engine.

## Fuentes primarias y técnicas

- W3C, CSS Snapshot 2025: https://www.w3.org/TR/css-2025/
- W3C, CSS Typed OM Level 1: https://www.w3.org/TR/css-typed-om-1/
- Tailwind CSS v4.0: https://tailwindcss.com/blog/tailwindcss-v4
- Lightning CSS: https://lightningcss.dev/
- Servo style system: https://book.servo.org/design-documentation/style.html
- Servo layout system: https://book.servo.org/design-documentation/layout.html
- Leptos Book: https://book.leptos.dev/
- Dioxus Tailwind guide: https://dioxuslabs.com/learn/0.7/guides/utilities/tailwind/
- Stylist crate: https://docs.rs/stylist/latest/stylist/
- Leptos Styling crate: https://docs.rs/leptos_styling/latest/leptos_styling/
