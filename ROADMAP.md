# PliegoCSS — ruta de trabajo

Fecha base: 2026-07-12

## Objetivo

Construir el compilador y verificador standards-first de CSS para humanos y agentes: CSS estándar de
entrada/salida, políticas ejecutables, procedencia, tokens, budgets, accesibilidad, explicación y
receipts deterministas. La sintaxis Rust compacta y la integración PliegoRS son frontends first-class,
no requisitos exclusivos de adopción.

El [contrato estratégico CSS/Rust/IA](docs/product/strategic-product-contract-2026.md) y la
[matriz de requisitos derivados del research](docs/product/research-requirements.md) A1–A12 son
normativos junto a esta ruta. Hacen explícitos audit, compatibilidad Baseline, DTCG, scopes, adapters,
migración, budgets, accesibilidad, agent contracts, language tooling y corpus de escala que no pueden
cerrarse por inferencia desde los gates existentes.

## Modelo de ejecución cerrado

- Desarrollo principal con **GPT-5.6 Ultra**.
- Ejecución autónoma, enfocada y continua sobre un solo workspace.
- Los agentes auxiliares solo se usarán cuando una tarea sea realmente paralelizable y tenga un contrato claro.
- Trabajo enfocado, pero sujeto a investigación y cambios propios de un compilador nuevo.
- Las ETA son rangos, no fechas contractuales.
- El MVP R0 cubre auditoría/compilación verificable; no pretende implementar el 100% de CSS ni todo
  Tailwind, y tampoco amplía el lenguaje de utilities antes de validar el wedge.
- PliegoRS debe estar disponible para la integración de SSR de las fases posteriores.

## Resumen de fundación implementada

F0–F8 conservan la historia y deuda de la fundación Rust/PliegoRS. Desde la decisión ADR-0015 no son
por sí solos el plan de release; E0–E4 y R0 del `EXECUTION_PLAN_ULTRA.md` controlan `0.1.0`.

| Fase | Resultado | GPT-5.6 Ultra | Calendario acumulado |
|---|---|---:|---:|
| F0 | Contrato y benchmarks base | 1–2 días | Semana 1 |
| F1 | Parser utility-first + IR | 2–3 días | Semana 1 |
| F2 | Valores, tokens y validación | 2–4 días | Semanas 1–2 |
| F3 | Compilador y CSS determinista | 3–4 días | Semana 2 |
| F4 | Variantes y responsive | 2–3 días | Semana 3 |
| F5 | Integración PliegoRS | 3–5 días | Semana 4 |
| F6 | Dev experience y recarga live full-page | 3–5 días | Semana 5 |
| F7 | Optimización y extracción | 3–5 días | Semana 6 |
| F8 | Hardening y MVP público | 4–7 días | Semana 7 |

La ETA original de **5–7 semanas** queda como registro del alcance utility-first inicial. La ETA
rebased desde el estado actual es **6–10 semanas calendario para R0**, condicionada al acceso al
corpus de 10–15 entrevistas y 20 incidentes reales. R1–R3 son posteriores.

El checkpoint técnico original de 8–13 días ya fue superado. El siguiente checkpoint decide sobre la
categoría después de un `audit` usable y las cinco reglas de mayor señal del corpus.

---

## F0 — Contrato fundacional

ETA: 1–2 días

### Tareas

- [x] Escribir la gramática inicial de `pc!("...")`.
- [x] Definir qué significa conflicto semántico.
- [x] Definir política de valores arbitrarios y CSS externo.
- [x] Seleccionar las primeras 30–40 utilidades.
- [x] Diseñar el formato del IR y los identificadores deterministas.
- [x] Preparar cinco fixtures: button, card, navbar, form y dashboard.
- [x] Implementar las mismas vistas con Tailwind v4 como baseline.
- [x] Registrar métricas iniciales: build cold/incremental y CSS raw/gzip.
- [x] Crear glosario inicial y guía de principios de diseño.
- [x] Registrar decisiones fundacionales como ADRs.

### Gate

La sintaxis, alcance del spike y métricas quedan congelados. No se programa el catálogo completo antes de pasar este gate.

## F1 — Parser utility-first e IR

ETA: 2–3 días

### Tareas

- [x] Crear workspace y crates base.
- [x] Implementar tokenizer de utilidades.
- [x] Parsear prefijos, propiedad, valor y modificadores.
- [x] Crear `StyleRule`, `Property`, `Value`, `Condition` y `SourceSpan`.
- [x] Implementar `pc!()` como macro procedural.
- [x] Generar errores con archivo, línea y sugerencias.
- [x] Añadir fuzzing determinista básico del parser y corpus de truncaciones; snapshots amplios siguen abiertos.
- [x] Garantizar que el IR sea serializable y estable mediante un envoltorio binario canónico de
  formato 1, consciente del tema y separado de los enums/tablas Rust internos.
- [x] Documentar gramática, mensajes de error y modelo mental del parser.
- [x] Añadir rustdoc y ejemplos compilables para la superficie candidata de aplicación y build.

### Gate

`pc!("flex items-center gap-4")` produce IR correcto; errores como `flec`, variantes incompletas y tokens mal formados fallan durante compilación.

## F2 — Tipos, tokens y validación semántica

ETA: 2–4 días

### Tareas

- [x] Implementar tipos para longitud, porcentaje, color, número y keywords.
- [x] Crear `theme!` para spacing, colors, radius, typography y breakpoints.
- [x] Validar dominio propiedad/valor.
- [x] Detectar conflictos dentro del mismo contexto.
- [x] Permitir overrides válidos entre breakpoints o estados.
- [x] Implementar sugerencias por distancia de edición.
- [x] Crear escape explícito para valores arbitrarios.
- [x] Validar CSS arbitrario con un parser real.
- [x] Documentar tokens, conflictos, valores arbitrarios y escape hacia CSS normal.
- [x] Crear una tabla de utilidades y tokens generada desde el catálogo real, verificada en CI.

### Gate

El compilador rechaza `flex grid`, acepta `flex md:grid`, detecta tokens inexistentes y permite valores arbitrarios válidos sin perder trazabilidad.

## F3 — Generador y pipeline CSS

ETA: 3–4 días

### Tareas

- [x] Traducir IR a reglas CSS.
- [x] Crear nombres de clase deterministas mediante hash estable.
- [x] Deduplicar reglas equivalentes.
- [x] Ordenar output independientemente del orden de compilación.
- [x] Integrar Lightning CSS para targets, lowering, prefijos y minificación.
- [x] Emitir CSS `pretty` en desarrollo y `minified` en producción mediante Lightning CSS.
- [x] Crear el manifest de estilo → orígenes, tema, targets e integridad CSS (entregado inicialmente
  como schema 2; contrato actual schema 3 tras exponer las versiones de identidad).
- [x] Extender el manifest opt-in a grafo ruta/island → componente → declaración semántica → token,
  con ownership exacto suministrado por un sidecar neutral y fallo cerrado si falta cobertura.
- [x] Añadir manifest schema 5 / graph schema 2 con trace fail-closed de declaraciones y reglas
  físicas después del emitter y Lightning CSS, rangos UTF-8 exactos y contribuciones many-to-many;
  schema 4 conserva exclusivamente el grafo semántico estable.
- [x] Añadir asset load plan schema 1 para bundles explícitos, con identidad común, hashes exactos,
  selección separada por ruta/island y registro externo de pruning.
- [x] Añadir Project Index schema 1 para compartir snapshots y sites portables, desde fuente y
  StyleId hasta declaraciones semánticas, tokens, componentes y output CSS físico.
- [x] Añadir pruebas reproducibles byte por byte.
- [x] Documentar el pipeline de compilación y el manifest de procedencia.
- [x] Publicar una guía de inspección del CSS y manifest generados.

### Gate

Dos builds idénticos producen exactamente los mismos bytes; estilos compartidos aparecen una sola vez y el CSS es válido en los browsers objetivo.

## Checkpoint de viabilidad A

ETA acumulada: 8–13 días

En este punto se compara el subconjunto core de los cinco fixtures con Tailwind v4. El checkpoint B,
al cerrar F4, compara los fixtures completos con responsive y estados.

Continuar si:

- la sintaxis está cerca de Tailwind en velocidad de escritura;
- los errores son claramente superiores;
- CSS gzip es igual o menor en el subconjunto core;
- `cargo check` incremental sigue siendo tolerable;
- los valores arbitrarios no convierten el sistema en una jaula.

Si no pasa, se corrige la sintaxis o se reduce el alcance tipado antes de integrar PliegoRS.

## F4 — Variantes, responsive y composición

ETA: 2–3 días

### Tareas

- [x] Implementar `hover`, `focus`, `focus-visible`, `active` y `disabled`.
- [x] Implementar `dark`, reduced-motion y preferencias de contraste.
- [x] Implementar breakpoints configurables.
- [x] Implementar combinaciones como `dark:md:hover:`.
- [x] Añadir composición de estilos reutilizables.
- [x] Diseñar `pcx!` para clases condicionales.
- [x] Resolver custom properties para valores reactivos.
- [x] Fusionar media queries adyacentes con AST exactamente igual, preservando orden y con gate
  raw/gzip; equivalencias no idénticas y agrupación global quedan fuera de este pase.
- [x] Documentar variantes, precedencia, responsive y composición condicional.
- [ ] Crear un cookbook de patrones interactivos accesibles.

### Gate

Las variantes son deterministas, composables y validadas; las condiciones dinámicas registran todas las ramas sin safelists.

## F5 — Integración nativa con PliegoRS

ETA: 3–5 días

### Tareas

- [x] Integrar `Style` con `pliego-dom` mediante `Display`/`String`, sin acoplar el renderer al compilador.
- [x] Integrar `pc!()` con `view!()`.
- [x] Verificar que `render_html()` emite la clase compilada.
- [x] Verificar en Chromium que la reanudación conserva el mismo
  document/island/button/bound-text element SSR, el ID y las clases mientras el estado avanza 15→20.
- [x] Derivar desde un registro de producto PliegoRS validado la topología de componentes, rutas e
  islands; capturar cada source registrado en su declaración y generar reachability schema 1.
- [x] Probar cobertura CSS-source del grafo Cargo exacto construido: dep-info de `site-lib`,
  `site-ssg` y `browser-client`, 12 units estables y fallo ante macros compilados sin registro.
  Features/targets no construidos quedan fuera del claim.
- [x] Emitir `<link rel="stylesheet">` correctos mediante `Head` durante SSR/SSG.
- [x] Derivar automáticamente particiones `shared`/route/island/unreachable y un plan declarativo
  schema 1 desde el registro, antes de la publicación agrupada recuperable.
- [x] Verificar el bundle-plan schema 2 aditivo con `dtcg-resolver`, `path` e `inputs`, preservando
  schema 1 seed/config y sin añadir flags de bundle; E2E Windows/WSL, rollback, `--check`, workspace
  y paquetes están verdes localmente.
- [x] Generar `pliego.assets.json` desde manifests 4/5 verificados y usarlo en el smoke SSG para unir
  la ruta con sus islands renderizadas, verificar hashes y excluir un bundle muerto podado.
- [x] Implementar el core público de ownership schema 1: binding exacto por bytes/SHA-256 al Asset
  Plan schema 1, cobertura bundle→package total/exclusiva, composición route→islands total,
  resolución deduplicada en orden canónico y productor Rust de bytes validados.
- [x] Cerrar `audit --asset-plan ... --ownership ...` con budgets tipados package/route, ledger
  `ownership`, rechazo de autoridades manuales y routes superpuestas que no se proyectan como
  particiones del Control Manifest. El core, el E2E Asset Plan y el replay del ejemplo están verdes
  localmente en Debian WSL2; no constituye evidencia hosted ni cierre de release.
- [x] Consumir `pliego.index.json` en el smoke SSG fijado por revisión y verificar fuentes exactas,
  backlinks, Asset Plan y referencias físicas antes de publicar el sitio.
- [x] Añadir preload CSS explícito y validado en `Head`; seleccionar solo el bundle compartido con
  tema y comprobar en Chromium que preload + stylesheet reutilizan una única descarga.
- [x] Mantener `class` y CSS externo como interoperabilidad.
- [x] Verificar cero runtime de estilos en el micro-fixture WASM; la app completa sigue abierta.
- [x] Compilar, publicar, medir y ejecutar un cliente WASM real en el fixture SSG acotado: 31,423 B
  raw / 12,584 B gzip-9; la comparación de una aplicación productiva sigue abierta.
- [x] Verificar SSG determinista, assets CSS, aislamiento derivado por ruta/island y markup resumible.
- [ ] Crear Getting Started de PliegoRS + PliegoCSS desde proyecto vacío.
- [ ] Documentar SSR, resumability, rutas, islands y uso de CSS externo desde proyecto vacío.

### Gate

Una aplicación PliegoRS renderiza SSR, resume eventos sin reconstruir la vista y carga únicamente su
CSS alcanzable. El gate actual cierra el wiring del registro, la partición automática, SSG, pruning y
selección route/island desde el Asset Plan; el gate CDP ejecuta un evento real preservando identidad
DOM y arranca el cliente WASM medido. Sigue abierta la evidencia de una aplicación productiva
completa y de futuras combinaciones Cargo de features/targets.

## F6 — Experiencia de desarrollo

ETA: 3–5 días

### Tareas

- [x] Crear `pliego-cssc`.
- [x] Implementar watch por polling para input line-oriented, árboles Rust y configuración
  explícita o auto-descubierta.
- [x] Certificar recarga full-page local mediante `pliego-cssc watch` + `pliego dev`, con CSS/SSE,
  navegador real, p50/p95 y casos negativos. CSS-only HMR queda diferido explícitamente.
- [x] Cachear unidades Rust parseadas por archivo usando snapshots de bytes exactos.
- [x] Cachear IR semántico por archivo e invalidarlo por identidad de tema.
- [x] Crear una superficie instalable `pliego-cssc check` con versión verificable.
- [ ] Delegar `pliego css check` desde el CLI upstream de PliegoRS.
- [x] Crear formatter determinista y lint semántico para listas explícitas/line-oriented.
- [x] Integrar diagnósticos read-only con rangos y reemplazos sobre literales Rust.
- [x] Exponer diagnósticos estructurados schema 1 para tooling de editor y CI.
- [x] Rechazar de forma uniforme opciones CLI de valor único repetidas; solo `--style`, `--source`,
  `--compose` y otras opciones marcadas explícitamente pueden repetirse.
- [x] Añadir un smoke downstream local bajo Rust 1.85 que compile la API candidata, configure un
  tema y cruce identidades/formatos con los comandos one-shot de `pliego-cssc` desde otro CWD.
- [ ] Aplicar fixes de formato sobre Rust de forma opt-in y collision-safe.
- [x] Exponer manifest para integración con editor.
- [x] Diseñar contratos mínimos de autocompletado/hover con `catalog` JSON y `explain` JSON.
- [ ] Implementar transporte LSP y clientes de editor sobre esos contratos.
- [x] Mejorar diagnósticos usando los cinco fixtures reales.
- [x] Crear documentación de CLI, configuración y troubleshooting; editor setup sigue abierto.
- [x] Ejecutar doctests y ejemplos del workspace desde el workflow de CI.

### Gate

Editar un token o utilidad actualiza el navegador con latencia percibida inmediata y sin recompilar innecesariamente toda la aplicación.

El gate local limpio de 20 muestras mide CSS p50 232.4 ms / p95 284.5 ms y convergencia site/SSE
p50 2.008 s / p95 2.458 s en Debian WSL2 con binarios y target Linux nativos; Chromium confirma el
reload resultante por separado. Evidencia
hosted y multi-browser sigue pendiente para el release.

## F7 — Optimización inteligente

ETA: 3–5 días

### Tareas

- [x] Construir la base versionada componente → declaración semántica → token con
  ruta/island explícitos.
- [x] Extender esa base a estilo → declaración física → regla postprocesada para optimización.
- [x] Eliminar de forma opt-in reglas de StyleId sin ningún origen exacto alcanzable mediante
  `--prune-unreachable`, conservando todos los orígenes cuando el estilo es shared.
- [x] Proyectar manifests 4/5 y reachability explícito en un asset load plan framework-neutral con
  `all-compiled|reachable-style-ids`, hashes exactos y rutas/islands independientes.
- [x] Proyectar manifest 5, reachability, documentos y bundles en un Project Index único y
  determinista, con consumidor PliegoRS cerrado y vector negativo de integridad.
- [x] Emitir un usage report pre-pruning por `(bundleId, StyleId)` que separa reachability estática,
  observación scoped, veredicto `observed|unobserved|dead|unknown` y disposición conservadora;
  conserva tombstones, liga hashes exactos y falla ante evidencia contradictoria.
- [x] Añadir retención explícita de StyleIds estructuralmente muertos mediante policy hash-bound,
  sin relabeling de evidencia y con selección versionada coherente en Usage Analysis, Asset Plan,
  Project Index y ownership.
- [ ] Podar variables/tokens no usados; `--theme` todavía emite completo su bloque soportado.
- [x] Derivar automáticamente una partición CSS shared/ruta/island/unreachable desde el registro de
  producto PliegoRS; para otros frameworks el asset plan conserva inputs explícitos.
- [ ] Implementar extracción de CSS crítico.
- [ ] Evaluar publicación crash-atomic frente a la publicación agrupada rollback-capable ya
  implementada y al output híbrido.
- [ ] Seleccionar estrategia por coste raw/gzip.
- [ ] Añadir reporte de tokens no usados y query interactiva sobre inclusión; el reporte seguro de
  StyleIds completos ya está implementado, sin afirmar pruning de declaraciones o variables.
- [x] Añadir explicación de inclusión de cada regla mediante la cadena completa de graph schema 2.
- [x] Comparar rendimiento contra Tailwind v4.
- [x] Documentar el modelo de optimización implementado con benchmarks controlados y límites
  explícitos, sin extrapolar porcentajes a aplicaciones reales.
- [x] Congelar snapshots locales reproducibles de Gate A, Gate B y Rust check desde el commit limpio
  `c47239c`; evidencia hosted/multiplataforma permanece en F8.
- [x] Añadir property tests de pipeline/identidad/IR/concurrencia y dos targets `cargo-fuzz` con
  presupuesto fijo por cambio, reproducciones retenidas y soak semanal configurado.
- [x] Verificar determinismo con 40 workers en cinco cohortes de ocho simultáneos, más referencias
  secuenciales y un probe determinista de rechazo del lock.

### Gate

La estrategia híbrida solo entra si demuestra una mejora medible. Si no, se conserva el modelo atómico más sencillo.

## F8 — Hardening y MVP

ETA: 4–7 días

Registro del alcance original. Tras ADR-0015, este hardening solo aporta evidencia parcial: el RC
también requiere E0–E4 y todos los Must de R0 en el contrato estratégico.

### Tareas

- [ ] Ampliar el catálogo a las utilidades necesarias para una aplicación real.
- [ ] Ejecutar la matriz hosted Windows/Linux/macOS, evidencia nativa ARM64 y el pipeline Cloudflare.
- [x] Configurar matriz Rust 1.85/1.96 para Windows, Linux y macOS con un vector ejecutable de
  CWD, Unicode/CRLF, symlink/junction, casing y `--check`; la evidencia hosted y Cloudflare siguen
  abiertas.
- [ ] Probar mensajes de error con usuarios/fixtures nuevos.
- [ ] Completar portal documental y catálogo navegable generado desde código.
- [ ] Crear guía de migración desde Tailwind.
  El productor Rust schema 1 y `pliego-cssc migration-inventory` ya inventarían un archivo
  Sass/Tailwind-entry/CSS-Modules con bytes/hash/spans, constructs dynamic/unsupported, dependencia
  Preflight y output stdout no-mutante. El core ya confirma un snapshot multi-file declarado por
  doble inventario canónico y deriva dependencias Sass/CSS/CSS Modules con resolución local exacta
  fail-closed y clasificaciones external/unresolved/local/dynamic; la declaración JSON schema 1
  permite persistir el mismo source set cerrado y el CLI read-only lo carga con límites/no-follow y
  emite el snapshot por stdout. Un fixture cross-toolchain versionado cubre las tres familias y las
  clasificaciones de edge; `@config`/`@plugin`/`@source` ya quedan visibles sin ejecución. Configs,
  plugins y templates declarados ya tienen identidad bytes/hash y linking relativo tipado. Faltan
  crawling, resolución específica del toolchain e inspección semántica profunda de auxiliares;
  templates ya exponen candidatos `class`/`className` literales y expresiones dynamic,
  mientras keys de config y APIs de plugin comunes quedan visibles como unsupported,
  consumers/aliases avanzados y fixtures
  de migración reales antes de escribir la guía completa.
- [ ] Crear tutorial completo, how-to guides y referencia de API.
- [ ] Crear guía de accesibilidad, theming y responsive design.
- [ ] Crear documentación para contribuidores, plugins y estabilidad semántica.
- [ ] Ejecutar prueba de documentación con developers que no conozcan el proyecto.
- [x] Implementar y documentar compatibilidad explícita mediante el atributo `class` y hojas externas.
- [x] Añadir un contrato plain-HTML no-Pliego que liga líneas explícitas a clases de manifest,
  verifica CSS calculado en Chrome y demuestra cero runtime de estilos para ese fixture.
- [ ] Añadir un adapter tipado Vite/React o Astro con cobertura exacta de branches y equivalencia de
  StyleId/clase/CSS/artifacts frente al frontend Rust/CLI.
- [x] Implementar policy de compatibilidad Baseline versionada, sin reset implícito, con perfiles
  `baseline-widely|modern|none`, decisiones por capability tier y rechazo de CSS no clasificado.
- [ ] Integrar y observar CI hosted de determinismo, tamaño y rendimiento; los gates locales y la
  matriz declarada ya cubren property tests, cinco cohortes de ocho workers, un probe de lock,
  fuzzing y package ceiling.
- [ ] Construir una aplicación PliegoRS completa con PliegoCSS.
- [x] Seleccionar y verificar por máquina la API candidata mínima de aplicación/build y el contrato
  de proceso one-shot, todavía sobre paquetes `0.0.0`.
- [ ] Activar esa selección como promesa SemVer en un RC aprobado y versionar `0.1.0`.

### Gate

El MVP puede construir una aplicación real sin parches internos, con documentación suficiente y benchmarks reproducibles.

---

## Trabajo R0/E2 actual — token graph, DTCG y accesibilidad estática

- [x] Implementar `pliegocss-token-graph/1` con aliases, derived values, deprecations, procedencia,
  rechazo de ciclos, themes/permutations y cobertura transitiva.
- [x] Implementar formato y Resolver DTCG 2025.10 bounded, y publicar el grafo canónico como
  `pliego.tokens.json` en compile/watch/bundle controlados.
- [x] Cerrar CLI `--tokens` para compile/build/check/inspect/watch con
  `--token-input modifier=context`, selección fail-closed, grafo completo, binding canónico en
  `configHash`, watch rollback, portabilidad local y package gate.
- [x] Cerrar el gate de bundle schema 2: Resolver exacto como `token-resolver`, grafo completo,
  selección ligada por los bytes exactos del plan, `--check` read-only y fallo sin publicación.
- [x] Conectar la selección DTCG al Cargo build macro con inputs literales ordenados, defaults,
  duplicados fail-closed, una llamada por paquete y artifact de la registry seleccionada. La
  suite completa, Clippy, MSRV y smoke de API pública pasan en Debian WSL2; el gate de paquete dirty
  Windows ejecuta consumidores DTCG/TOML idénticos, y el commit `9714b09` pasa el gate limpio Debian
  WSL2 con la misma salida exacta.
- [x] Implementar policy schema 1 / policy 1 para contraste declarado, motion, focus visibility,
  forced colors e input modality, con enforcement `fail|warn` separado para violation,
  unverified y manual-required, excepciones deterministas y resolución opcional contra
  `pliegocss-token-graph/1`. El gate local pasa 30 tests de control, nueve tests CLI de `audit`, un
  test Asset Plan, Clippy estricto y el package gate dirty. Esta rebanada es análisis estático
  configurable, no certificación WCAG; precisión/recall sobre el corpus, pruebas browser/manuales,
  evidencia hosted y la matriz nativa macOS/ARM64 permanecen abiertas.

## Trabajo R1/S3 actual — explicación de cascada

- [x] Implementar `explain-cascade` schema 1 sobre CSS estándar con un stylesheet de autor,
  descriptor HTML simple y set cerrado de longhands. El gate local prueba rangos exactos,
  extracción de shorthands, layers normales/importantes, especificidad, source order, JSON
  determinista y handoff `browser-required` para contexto no demostrable.
- [ ] Extender a cascada completa: múltiples sheets/origins, inline styles, selectors y scopes
  completos, layers anidados, propiedades lógicas/custom properties, herencia, animaciones,
  transiciones, joins con component/route/artifact y continuación browser. El slice schema 1 no
  reemplaza el motor de cascada del navegador ni cierra R1.

## Trabajo R1/S2 actual — plan y reparación agent

- [x] Implementar proposal/plan/dry-run schema 1.0.0 en `pliego-css-agent`, con edits UTF-8 exactos,
  autoridad limitada a findings verified/unexcepted y suggestions low-risk, ranges contenidos,
  prerequisitos, no-overlap, paths portables y budgets de files/edits/bytes.
- [x] Implementar `pliego-cssc plan` y `fix --dry-run` con binding exacto del FindingDocument,
  snapshots source before/after, patch `pliegocss-byte-edits/1`, plan/payload hashes y estados
  atómicos `ready|already-applied`; ninguna ruta muta archivos.
- [x] Añadir apply autorizado por token exacto del plan, lock cooperativo persistente,
  replace/publicación agrupada con rollback y Change Receipt 1.0.0 honesto. El token selecciona un
  plan; no autentica identidad. El receipt queda `checks-pending`, con checks `not-run` y browser
  evidence `not-collected`.
- [ ] Completar required checks y evidencia final. Policy/Verification Receipt 1.4.0 conserva
  lectura canónica 1.0.0/1.1.0/1.2.0/1.3.0; `verify` ejecuta únicamente `standard-css-audit`,
  `token-graph-integrity` y `css-budget-audit` en proceso, y valida `test-suite-evidence` producida
  por el `run-tests` explícito de perfil Cargo fijo más `browser-evidence` producida por un
  `run-browser` PliegoRS/Chromium fijo. Policy/plan no pueden aportar comandos, scripts, URLs,
  selectores ni argumentos. Budget
  liga política exacta y subjects package/route; test liga Change Receipt, root Cargo manifest y
  lockfile; browser liga Change Receipt, ocho inputs del perfil, identidad Node/CDP y observaciones
  DOM/runtime cerradas; cada check publica FindingDocument canónico con receipt-last y rollback.
  Faltan evidencia hosted/firmada y Firefox/WebKit; los gates locales test/browser están cerrados.
- [ ] Medir el loop sobre el corpus congelado y demostrar menos turnos sin aumentar violations. La
  infraestructura local ya congela y reejecuta 16 casos sintéticos del boundary de autoridad (un
  control aceptado y 15 rechazos fail-closed), con provenance y claim boundary explícitos; no cuenta
  como los 20 incidentes reales ni como evidencia de turn reduction.

## Backlog posterior al MVP

No forma parte de la ETA de 10–12 semanas:

- [ ] Container queries y variantes de grupo/peer.
- [ ] Animaciones y `keyframes!` tipados.
- [ ] Typography avanzada y OpenType.
- [ ] Gradients y color spaces avanzados.
- [ ] CSS nesting estructurado.
- [ ] Plugins mediante traits/IR, sin ejecución arbitraria.
- [ ] Temas versionados/proyectados desde Hyphae.
- [ ] DevTools visual de procedencia.
- [ ] Compatibilidad más amplia con utilidades Tailwind.
- [ ] Integración opcional para frameworks Rust distintos de PliegoRS.

## Métricas que se reportarán en cada milestone

- Tiempo de `cargo check` cold e incremental.
- Tiempo de compilación CSS cold e incremental.
- Tamaño CSS raw y gzip.
- Tamaño WASM antes/después.
- Número de reglas generadas, deduplicadas y eliminadas.
- Número de escapes a CSS normal/arbitrario.
- Tiempo de implementación de los fixtures.
- Casos inválidos detectados en compilación.
- Diferencias SSR/resumability.

## Riesgos que pueden mover la ETA

| Riesgo | Impacto probable |
|---|---:|
| Macros aumentan demasiado `cargo check` | +1–2 semanas |
| La recarga live requiere cambios profundos en tooling | +1 semana |
| Integración PliegoRS aún no tiene seams estables | +1–3 semanas |
| Intentar cubrir todo Tailwind en el MVP | +2–4 meses |
| Dejar la documentación para el final | +2–3 semanas y APIs más difíciles de corregir |
| Crear parser/minificador CSS propio | +3–6 meses |
| Crear motor de layout/renderizado propio | proyecto multianual |

## Orden de ejecución recomendado

Gate A y Gate B están cerrados como GO condicionado. La ejecución actual cruza **F5/F6/F7/F8**:
integración PliegoRS completa, watch/recarga live y hardening local de API, paquetes y documentación.
La resumability local en Chromium y el Asset Plan route/island sobre partición derivada ya están
implementados;
el ownership exacto ya resuelve packages y composiciones, y la retención bundle-qualified comparte
selección schema 2 con Usage Analysis, Asset Plan y Project Index. Sus integraciones CLI están verdes
localmente. El registro de producto, su cobertura sobre los tres targets Cargo exactos, la partición
automática y el preload explícito/acotado ya están verdes; política de performance general y el
release `0.1.0` siguen abiertos.
