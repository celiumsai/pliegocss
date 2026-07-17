# PliegoCSS — plan de ejecución GPT-5.6 Ultra

Fecha base: 2026-07-12

## Objetivo de entrega

Publicar PliegoCSS `0.1.0` como compilador y verificador standards-first para humanos y agentes, con:

- entrada y salida CSS estándar sin migración obligatoria;
- `pliegocss audit` como primer valor sobre repos existentes;
- compatibilidad, accesibilidad, cascade, tokens y budgets como políticas versionadas;
- diagnósticos humanos, JSON y SARIF semánticamente equivalentes;
- manifest y receipt deterministas con hashes, backend, targets, decisiones y evidencia;
- PliegoRS first-class mediante manifests tipados, sin excluir otros stacks;
- `pc!` y `pcx!` conservados como frontend tipado opcional;
- cero runtime de estilos por defecto y browsers reales para evidencia de layout;
- documentación completa y ejemplos/gates ejecutados en CI.

La ETA original de 5–7 semanas correspondía al alcance de framework utility-first y queda
supersedida. Desde el estado actual, R0 requiere **25–40 días de ingeniería (5–8 semanas activas)**;
la ETA calendario responsable para `0.1.0` es **6–10 semanas**, condicionada además al acceso a
10–15 entrevistas y 20 incidentes reales. R1–R3 son fases posteriores y no se comprimen dentro de
esa ETA.

El [contrato estratégico CSS/Rust/IA](./docs/product/strategic-product-contract-2026.md), la matriz
A1–A12 en `docs/product/research-requirements.md` y este plan son normativos. Un gate no está cerrado
sin código, comando reproducible y artifact; compatibilidad, DTCG, adapters, migración, budgets,
accesibilidad, receipts y tooling de lenguaje no pueden quedar ocultos bajo una etiqueta genérica de
hardening.

## Rebase de ejecución adoptado el 2026-07-14

Las semanas 1–6 de este documento registran trabajo fundacional ya realizado. Sus gates prueban el
frontend Rust, el IR, el emitter, la procedencia y el loop PliegoRS, pero no equivalen al nuevo R0. A
partir de este rebase, el orden controlador es:

| Bloque | Entrega | Carga GPT-5.6 Ultra | Gate |
|---|---|---:|---|
| E0 Evidence + schemas | Inventario actual, corpus/interviews, schema de diagnóstico y diseño del manifest/receipt unificado | 2–4 días de ingeniería; 1–2 semanas de investigación paralela | Casos y métricas definidos; schemas revisados antes de ampliar sintaxis |
| E1 Audit + CSS estándar | `audit`, ingestión CSS normal, backend versionado, datos oficiales de compatibilidad y decisiones explicables | 5–8 días | Repos genéricos producen output/diagnóstico determinista sin migración |
| E2 Guard + tokens | Budgets completos, duplicación, especificidad, token graph/aliases/themes/cycles y políticas básicas de contraste/motion/focus | 8–12 días | Top cinco reglas del corpus pasan precisión/recall y excepciones configurables |
| E3 Agent + receipt | Paridad humana/JSON/SARIF, manifest unificado, receipt, planes y dry-run read-only | 6–10 días | Schemas byte-estables, spans exactos e idempotencia demostrada |
| E4 Bridges + release proof | Inventarios Sass/Tailwind/CSS Modules, hosted cross-OS, corpus competitivo, docs y onboarding | 4–6 días | R0.1–R0.7/R0.9 verdes; R0.8 cerrado o movido por ADR explícito |

No se amplía el lenguaje utility-first, el catálogo de componentes ni un plugin ecosystem antes de
cerrar esas dependencias. El orden posterior es R1 explain/agent loop, R2 browser-assisted y R3
ecosystem gobernado.

## Semana 1 — lenguaje y núcleo

### Entregables

- Workspace y crates fundacionales.
- Gramática inicial de `pc!`.
- Parser utility-first.
- IR semántico con source spans.
- Primeras 30–40 utilidades.
- Cinco fixtures equivalentes en Tailwind v4.
- ADRs, glosario y principios de diseño.

### Tareas

- [x] Congelar sintaxis del spike.
- [x] Definir modelo de conflictos.
- [x] Crear baseline Tailwind de button, card, navbar, form y dashboard.
- [x] Implementar tokenizer y parser.
- [x] Implementar macro procedural `pc!`.
- [x] Crear tipos base de IR.
- [x] Añadir errores con ubicación y sugerencias.
- [x] Añadir doctests y fuzzing determinista inicial; snapshots amplios siguen abiertos.
- [x] Documentar modelo mental y gramática.

### Gate semanal

`pc!("flex items-center gap-4")` compila; errores tipográficos o sintácticos fallan con diagnóstico útil.

## Semana 2 — semántica y compilación CSS

### Entregables

- Tema y tokens.
- Validación propiedad/valor.
- Detección de conflictos.
- Valores arbitrarios.
- Generador CSS determinista.
- Lightning CSS integrado.
- Manifest de procedencia.
- Benchmark contra Tailwind v4.

### Tareas

- [x] Implementar length, percentage, color, number y keywords.
- [x] Implementar `theme!`.
- [x] Validar tokens y dominios.
- [x] Rechazar `flex grid` y aceptar `flex md:grid`.
- [x] Implementar valores/propiedades arbitrarias y conservar CSS normal como interoperabilidad.
- [x] Emitir IDs estables y reglas deduplicadas.
- [x] Integrar minificación y browser targets.
- [x] Congelar compatibilidad Baseline como policy schema 1 / policy 1, con acciones por feature,
  reset/scope explícitos, diagnósticos CMP fail-closed y gate byte-exacto reproducible.
- [x] Verificar determinismo byte por byte en fixtures; matriz clean/paralela/multiplataforma abierta.
- [x] Generar referencia desde el catálogo real.
- [x] Medir build, CSS gzip, ergonomía y diagnósticos.

### Gate A — viabilidad del núcleo

La semana 3 solo comienza si:

- la sintaxis mantiene velocidad cercana a Tailwind;
- los errores son claramente mejores;
- CSS gzip es igual o menor en el subconjunto core de los fixtures;
- `cargo check` incremental es aceptable;
- no aumenta el WASM;
- los valores arbitrarios preservan libertad suficiente.

Si un criterio falla, se corrige durante 2–3 días. Si continúa fallando, se reduce el alcance tipado o se reconsidera construir PliegoCSS.

Resultado revalidado 2026-07-13: **GO condicionado** para continuar. El matched core pasa latencia,
CSS gzip y libertad de valores arbitrarios. El snapshot pareado limpio centra el overhead
incremental en +24.316 ms no-op y +28.228 ms changed; el cold overhead de +9.783 s y la varianza
observada mientras Defender/WDAC estaban activos siguen como deuda. El smoke Chromium y el
micro-fixture WASM pasan;
targets/prefixing y tema configurable ya están implementados. La app PliegoRS completa y la matriz
multi-browser permanecen abiertas. Ver [benchmark](./docs/benchmarks/pliego-gate-a.md) y
[estado del gate](./docs/status/gate-a.md).

## Semana 3 — variantes y composición

### Entregables

- Estados interactivos.
- Responsive y dark mode.
- Composición.
- `pcx!` condicional.
- Custom properties dinámicas.

### Tareas

- [x] Añadir hover, focus-visible, active y disabled.
- [x] Añadir dark, reduced-motion y contraste.
- [x] Añadir breakpoints configurables.
- [x] Componer variantes encadenadas.
- [x] Implementar estilos constantes reutilizables.
- [x] Implementar ramas dinámicas conocidas durante build.
- [x] Resolver valores reactivos mediante custom properties.
- [x] Documentar variantes, precedencia y patrones accesibles.

### Gate B — fixtures completos

Los cinco fixtures cubren responsive, estados y tema sin safelists ni generación dinámica de nombres.
Se compara CSS gzip y también HTML gzip + CSS gzip contra Tailwind v4.

Resultado revalidado 2026-07-13: **GO condicionado**. El fixture completo compila 44/44 atributos,
cubre ocho variants, produce 30/30 hashes iguales y queda en 2,049 B CSS gzip / 3,550 B HTML+CSS frente a
2,327 B / 3,833 B de Tailwind no-preflight. Ver [Gate B](./docs/benchmarks/pliego-gate-b.md).

## Semana 4 — PliegoRS

### Entregables

- Integración `pc!` + `view!`.
- `StyleId` en el árbol DOM.
- SSR y resumability deterministas.
- CSS por ruta/island mediante particiones explícitas y asset plan generado; partición automática
  pendiente.
- Interoperabilidad con CSS estándar.

### Tareas

- [x] Integrar `Style`/`StyleId` con el seam de clase de `pliego-dom` mediante `Display`/`String`.
- [ ] Conectar el registro de estilos al árbol de vistas.
- [x] Emitir links de stylesheet mediante el `Head` de SSG.
- [x] Añadir preload CSS explícito y validado en `Head`; el fixture selecciona únicamente el bundle
  compartido con tema y Chromium confirma una sola descarga, sin afirmar mejora de latencia.
- [x] Verificar en Chromium que el runtime reusa el mismo
  document/island/button/bound-text element SSR,
  preserva clases e ID y actualiza estado 15→20 mediante el evento delegado.
- [x] Derivar reachability desde el registro validado de componentes/rutas/islands de PliegoRS,
  capturando el source de cada componente en su declaración y fallando ante macros sin ownership.
- [x] Probar cobertura CSS-source contra el grafo Cargo exacto de `site-lib`, `site-ssg` y
  `browser-client`: rustc dep-info aporta 12 units, se revalida tras ambos builds y cualquier macro
  compilado sin registro falla. Features/targets no construidos no se incluyen en el claim.
- [x] Derivar automáticamente particiones `shared`/route/island/unreachable, probar SSG determinista y
  verificar el asset ledger.
- [x] Generar `pliego.assets.json` schema 1 mediante `bundle --asset-plan` desde manifest 4/5 +
  reachability, con hashes exactos y mappings route/island separados.
- [x] Generar `pliego.index.json` schema 1 mediante `bundle --project-index` y consumirlo en el smoke
  SSG PliegoRS con verificación de fuentes, sites, backlinks y output físico calificado por bundle.
- [x] Probar el smoke SSG con manifest 5/graph 2 + pruning: `/` selecciona `shared` + `route-home`;
  `/visit` agrega `shared` + `route-visit` y su island aporta `island-visit-counter`;
  `unreachable` permanece en el inventario del plan pero no se despliega.
- [x] Probar markup SSR y asset/link aislado del runtime de una isla resumible.
- [x] Compilar, publicar y ejecutar un cliente WASM real en el fixture SSG, registrando 31,423 B raw
  / 12,584 B gzip-9 de WASM y 39,962 B raw / 15,186 B gzip-9 para sus cuatro recursos browser.
- [ ] Medir CSS y WASM de una app real.
- [ ] Escribir Getting Started completo.
- [ ] Documentar SSR, resumability y CSS externo desde proyecto vacío.

### Gate semanal

Una aplicación PliegoRS completa renderiza y resume eventos sin diferencias, sin runtime de estilos y sin cargar CSS inalcanzable.

El fixture actual prueba esa selección con reachability y bundle plan derivados de un registro de
producto y un Asset Plan consumido por el SSG; el gate CDP reproduce además el evento resumible en
Chromium y preserva la identidad de los nodos SSR. La cobertura Cargo está probada solo para los tres
targets/configuración construidos. No prueba política universal de performance/preload, matriz
hosted/multi-browser ni una aplicación productiva completa; la medición WASM actual pertenece al
fixture acotado.

## Semana 5 — developer experience

### Entregables

- `pliego-cssc`.
- Watch incremental.
- Recarga live full-page.
- Cache del IR.
- Formatter/linter.
- Autocompletado inicial.

### Tareas

- [x] Implementar CLI y archivo de configuración tipado.
- [x] Implementar rebuild por polling de input, árboles Rust y tema.
- [x] Certificar el loop local `pliego-cssc watch` + `pliego dev` con navegador real, generación
  SSE, CSS/clase final, p50/p95 y conservación del último artifact válido. Es reload full-page;
  CSS-only HMR queda diferido.
- [x] Evitar relectura y reparseo completo con snapshots exactos y caché sintáctica por archivo.
- [x] Evitar lowering de unidades sin cambios con caché de IR semántico invalidada por tema.
- [ ] Evitar emisión global cuando cambie un subconjunto mediante fragmentos CSS incrementales.
- [x] Crear comandos `check`, `build`, `watch` e `inspect`.
- [x] Exponer manifest, catálogo JSON para completion y `explain` JSON para hover (versiones
  iniciales cerradas; contratos actuales manifest 3 por defecto, 4 con graph 1 semántico y 5 con
  graph 2/traza física fail-closed, reachability 1, catálogo 3 y explain 2).
- [x] Exponer Project Index schema 1 como modelo portable compartido para CLI, adapters y futuro
  LSP. El contrato plain-HTML no-Pliego ya consume manifest schema 3 con bindings exactos y browser
  real; el transporte LSP y un adapter tipado Vite/Astro que produzca topología/Project Index siguen
  pendientes.
- [ ] Implementar transporte LSP y clientes de editor.
- [x] Añadir formatter determinista y `--check` para utilidades explícitas y archivos line-oriented.
- [x] Añadir diagnósticos y reemplazos read-only sobre literales Rust.
- [x] Añadir envelope JSON estable para diagnósticos de comandos one-shot.
- [x] Hacer estrictamente single-use todas las opciones CLI de valor único y comprobar que los
  duplicados fallen cerrados; solo las opciones documentadas como repeatables admiten repetición.
- [x] Añadir un smoke downstream local con Rust 1.85 para `Style`/`StyleId`, `pc!`, `pcx!`, el bridge
  `theme!`, los documentos CLI versionados y la igualdad de identidad Rust/CLI desde un CWD externo.
- [ ] Añadir aplicación opt-in de fixes seguros desde tooling de editor.
- [x] Completar documentación inicial de CLI, configuración y troubleshooting.

### Gate semanal

Una edición estilística aparece en el navegador con latencia inmediata y los errores apuntan al lugar exacto en Rust.

El gate local limpio de 20 muestras está cerrado con CSS p50 232.4 ms / p95 284.5 ms y convergencia
site/SSE p50 2.008 s / p95 2.458 s; Chromium confirma el reload resultante por separado. La matriz
hosted/multi-browser permanece como gate de release.

## Semana 6 — optimización y robustez

### Entregables

- Grafo de reachability semántico y trace físico fail-closed (implementados como manifest 4/5 y
  graph 1/2).
- Partición automática shared/ruta/island (pendiente; el asset plan mapea bundles declarados).
- Evaluación crash-atomic vs agrupada rollback-capable vs híbrida.
- CSS crítico.
- Benchmarks reproducibles.
- Fuzzing sostenido.

### Tareas

- [x] Eliminar reglas de StyleId inalcanzables mediante `--prune-unreachable` explícito sobre
  reachability schema 1, sin derivar ownership ni particiones.
- [x] Generar el asset load plan framework-neutral schema 1 con selección
  `all-compiled|reachable-style-ids`, hashes exactos, tema global y roots route/island separados.
- [x] Consumir ese plan en el smoke PliegoRS schema 5 + pruning y excluir del deployment el bundle
  muerto no referenciado, sin borrarlo del inventario de build.
- [x] Documentar el contrato completo y la decisión en asset-plan schema 1 y ADR-0010.
- [x] Documentar y consumir Project Index schema 1 para unir fuentes, sites, semántica, aplicación y
  declaraciones CSS físicas sin reconstruir ownership por heurística.
- [x] Publicar `pliego.usage.json` sobre el universo pre-pruning con estados independientes de
  reachability, observación, uso y remoción; `--observations` liga universo/reachability exactos,
  ausencia sampled nunca prueba deadness y la contradicción observed/unreachable falla cerrada.
- [x] Implementar `--retention` como excepción explícita bundle-qualified: conserva StyleIds dead
  completos sin convertirlos en reachable/observed y versiona la selección de Usage Analysis,
  Asset Plan, Project Index y ownership.
- [ ] Eliminar variables/tokens inalcanzables; el bloque emitido por `--theme` permanece global.
- [x] Fusionar media queries adyacentes con AST exactamente igual sin cruzar límites de cascada;
  el benchmark dirigido ahorra 437 B raw / 13 B gzip con tema y deja Gate A/B neutrales.
- [x] Implementar `why included` semántico como ruta/island → componente → declaración → token.
- [x] Extender `why included` hasta declaración/regla CSS postprocesada mediante manifest schema 5.
- [ ] Comparar estrategias de output por coste gzip.
- [ ] Adoptar híbrido únicamente si gana de forma medible.
- [x] Ejecutar property tests transversales y fuzzing reproducible con corpus/diccionario
  versionados; el soak semanal queda configurado y su evidencia hosted permanece pendiente de
  acumularse y revisarse.
- [x] Probar determinismo con 40 workers distribuidos en cinco cohortes de ocho simultáneos, más dos
  referencias secuenciales y un probe de lock rechazado de forma determinista, incluyendo
  publicación fail-fast sobre el mismo par CSS/manifest y graph schema 2.
- [ ] Publicar metodología y resultados.

### Gate semanal

Toda optimización tiene benchmark; ninguna complejidad entra solamente por intuición.

## Producto `0.1.0` — gate R0 rebased

### Entregables

- `pliegocss audit` útil sobre CSS estándar y repos no-Pliego.
- Policy engine R0: compatibilidad, budgets, token graph y a11y básica.
- Manifest/receipt unificado y reportes humano/JSON/SARIF equivalentes.
- Evidencia de corpus, precisión diagnóstica, determinismo y rendimiento.
- Bridges de inventario Sass/Tailwind/CSS Modules, o ADR explícito que calendarice el Should R0.8.
- Portal/manual completo con límites y no-claims verificables.
- Release candidate `0.1.0`.

### Tareas

- [x] Congelar schema de diagnóstico y diseñar un nuevo schema de manifest/receipt sin sobrecargar
  manifests 3–5.
- [ ] Implementar ingestión y auditoría de CSS estándar mediante el backend probado. La rebanada
  read-only ya parsea CSS normal, congela backend/hash/inventario, exige target explícito y aplica el
  clasificador de features de reglas; faltan cobertura completa de declarations/selectors y las
  policies E2 antes de cerrar la tarea.
- [x] Implementar la base estática acotada de S3/R1: `explain-cascade` schema 1 sobre un stylesheet
  de autor, elemento HTML simple y longhand cerrado, con spans exactos, shorthand origin,
  layers/importance/specificity/source-order, escalamiento y `browser-required` fail-closed. La
  cascada completa multi-sheet/origin, selectors/scopes/logical properties, herencia, graph joins y
  continuación browser permanece abierta y no se contabiliza como R1 cerrado.
- [ ] Cerrar el loop agent R1. La rebanada acotada ya implementa proposal/plan/dry-run schema
  1.0.0: acepta únicamente edits exactos ligados a findings verified, sin excepción y suggestions
  low-risk; aplica containment, prerequisitos, no-overlap, UTF-8 y change budgets; liga el
  FindingDocument, snapshots before/after, patch determinista y plan hash; y demuestra estados
  completos `ready|already-applied`. Apply explícito ya exige el token exacto del plan, revalida bajo
  lock persistente y publica sources + Change Receipt 1.0.0 como grupo con rollback; el recibo de
  cambio mantiene `checks-pending`, checks `not-run` y browser evidence `not-collected`. La
  verificación separada ya implementa policy/receipt 1.4.0 (lectura canónica
  1.0.0/1.1.0/1.2.0/1.3.0), los checks en proceso
  `standard-css-audit`/`token-graph-integrity`/`css-budget-audit`, y `test-suite-evidence` ligada al
  runner explícito de perfil Cargo fijo, Change Receipt y root manifest/lockfile, más
  `browser-evidence` ligada al runner PliegoRS/Chromium fijo, ocho profile inputs y observaciones
  CDP. No existe command/script/URL/selector surface en policy/plan; cada check publica su
  after-FindingDocument canónico adyacente con receipt-last y rollback. Faltan matriz
  Firefox/WebKit y hosted/signed runner para cerrar. El harness de corpus agent ya fija 16 casos
  sintéticos de conformidad del boundary y su hash, pero aún faltan trials same-model sobre el corpus
  revisado y los 20 incidentes reales; no se reclama reducción de turnos.
- [x] Integrar datos oficiales web-features/versionados y registrar fecha/vector de targets:
  policy schema 2 / policy 7 fija `web-features@3.32.0`,
  `baseline-browser-mapping@2.10.43`, SRI/hashes, query con fecha y decisiones auditables.
- [ ] Implementar budgets por ruta/paquete/layer para bytes, reglas, selectores, especificidad y
  duplicación semántica, con delta y excepciones. La rebanada schema 1 ya cierra un artifact exacto,
  ownership explícito, layers detectados y findings fail-closed. El core público ownership schema 1
  liga bytes/hash exactos del plan, exige bundle→package total/exclusivo y route→islands total,
  produce bytes canónicos para adapters y resuelve package groups más routes compuestas con
  deduplicación. Su integración `audit --ownership`, ledger y budgets tipados están verdes localmente
  en Debian WSL2, y las routes superpuestas permanecen fuera de las particiones Control Manifest.
  Usage analysis schema 1 ya conserva el universo pre-pruning, separa reachability/observación/uso,
  deriva `observed|unobserved|dead|unknown`, liga evidencia exacta y registra tombstones/removal para
  StyleIds completos. Schema 2 y el sidecar de retención ya preservan excepciones exactas sin
  relabeling y comparten selección con Asset Plan, Project Index y ownership. Faltan granularidad
  CSS/token genérica y evidencia hosted/release para cerrar R0.4.
- [ ] Cerrar el gate de producto del token graph. El core canónico ya implementa aliases, derived
  values, deprecations, procedencia, rechazo de ciclos, themes/permutations y cobertura transitiva;
  el bridge de formato y Resolver DTCG 2025.10 están implementados, y los grupos controlados publican
  `pliego.tokens.json`. La superficie compile/build/check/inspect/watch ya implementa
  `--tokens FILE`, `--token-input modifier=context`, selección fail-closed, grafo completo y binding
  canónico en `configHash`; sus gates locales de suite, watch, paquete y vector directo congelado
  Windows/Linux están verdes.
  Bundle-plan schema 2 conserva schema 1 seed/config, añade
  `dtcg-resolver`/`path`/`inputs` sin flags nuevos, registra bytes exactos como `token-resolver`,
  publica el grafo completo y liga la selección mediante los bytes exactos del plan. Sus gates E2E
  Windows/WSL, workspace, Rust 1.85 y paquete están verdes. El Cargo build macro ya acepta
  `theme!(tokens = FILE, inputs = { modifier => context })`, conserva la forma TOML, selecciona
  defaults con inputs vacíos, rechaza duplicados y escribe sólo la registry activa. La suite
  completa, Clippy, MSRV y API pública pasan en Debian WSL2, y el paquete dirty Windows ejecuta
  consumidores DTCG/TOML idénticos. El commit `9714b09` pasa el packaging limpio Debian WSL2 con la
  misma salida exacta. La relación de contraste declarada se implementa mediante la policy de
  accesibilidad schema 1 y un TokenGraph opcional; evidencia hosted sigue pendiente antes de cerrar
  el conjunto de gates de R0.5.
- [x] Implementar policy schema 1 / policy 1 para contraste declarado y checks configurables de
  motion, focus visibility, forced colors e input modality, con estados verified/unverified/manual,
  excepciones deterministas y enforcement `fail|warn` por clase de resultado. Funciona sobre CSS
  directo y Asset Plans verificados, registra policy/TokenGraph exactos en control artifacts y
  publica el conteo real de pares. El gate local pasa 30 tests de control, nueve tests CLI de
  `audit`, un test Asset Plan, Clippy estricto y el package gate dirty. No certifica WCAG: faltan
  precision/recall del corpus, browser/manual, hosted y evidencia nativa macOS/ARM64.
- [x] Emitir reportes humano, JSON y SARIF 2.1.0 semánticamente equivalentes desde el mismo
  `FindingDocument`; cada resultado SARIF conserva el finding canónico completo.
- [ ] Implementar manifest/receipt determinista con source/config/output hashes, backend, targets,
  data date, decisiones, violaciones, excepciones y checks. El crate `pliego-css-control` ya cierra
  el wire contract schema 1; `audit --control-dir` ya lo alimenta desde analizadores reales para CSS
  directo o Asset Plan, publica findings/manifest/receipt como grupo rollback-capable y soporta
  `--check` sin escribir. También conserva estados `unavailable`/`unknown` en vez de inventar
  métricas. `bundle --control` ya enlaza plan/config/reachability/fuentes y publica CSS, maps,
  manifests, un TokenGraph compartido, Asset Plan, Project Index, findings, manifest y receipt como
  un solo grupo. Los vectores CSS directo y auditoría independiente de Asset Plan tienen identidad
  byte-exacta local en Windows x64 y Linux x64 bajo Debian WSL2; los vectores graph-bearing de
  compile, watch y bundle-build están re-frozen y verdes localmente en Windows y Linux x64.
  Compile/watch publican desde un snapshot exacto y conservan el último grupo válido. Los grupos
  generados ya emiten Source Map v3 determinista por CSS, lo enlazan por bytes/hash en
  manifest/receipt y publican
  `pliegocss-token-graph/1` con hash canónico y cobertura transitiva real. TOML/seed todavía proyecta
  un tema literal; la selección DTCG del CLI publica el Resolver completo y enlaza la selección
  canónica con gates locales verdes. Bundle schema 2 aplica ese contrato desde el plan exacto con
  gates locales verdes. El Cargo build macro ya selecciona la misma registry DTCG para expansión,
  con workspace/MSRV/API local y packaging limpio del commit `9714b09` verdes. Atribución completa,
  policies de accesibilidad y evidencia hosted/macOS/ARM64 siguen pendientes antes de cerrar
  R0.2/R0.5.
- [ ] Crear inventarios read-only Sass, Tailwind y CSS Modules sin prometer migración perfecta. La
  primera rebanada Rust schema 1 y `pliego-cssc migration-inventory` ya cubren un archivo explícito
  por familia, hashes/spans, dynamic/unsupported, Preflight y un contrato stdout no-mutante. El core
  ya confirma snapshots declarados de hasta 4.096 fuentes mediante doble inventario canónico y
  deriva edges conservadores Sass/CSS/CSS Modules: targets locales exactos fallan si no están
  declarados con el tipo esperado; external/unresolved/local/dynamic quedan explícitos sin emular el
  toolchain. El mismo source set puede persistirse como declaración JSON schema 1 cerrada y acotada.
  El CLI read-only ya carga esa declaración con límites/no-follow y emite el snapshot canónico solo
  por stdout. Un fixture versionado cruza las tres familias y congela resolved/local/external/
  unresolved más el seam de plugin unsupported. El core ya retiene `@config`, `@plugin` y `@source`
  como observaciones external/unresolved/dynamic sin ejecutarlas. Configs, plugins y templates
  explícitos ya se fijan por bytes/hash y enlazan seams relativos del tipo exacto; no se ejecuta JS
  ni template code. Templates declarados ya clasifican candidatos literales exactos y atributos
  con expresión como dynamic. Faltan crawling de proyecto, resolución específica Sass/bundlers e
  inspección semántica profunda de configs/plugins y dialectos de template; keys/APIs comunes ya
  quedan como seams lexicales unsupported. Consumidores
  CSS Modules JS/TS declarados ya enlazan ESM, TypeScript import-equals y CommonJS simple, y
  clasifican usos static/dynamic; faltan destructuring/aliases avanzados y un corpus de proyectos
  reales para cerrar. Un harness versionado ya
  congela tres casos authored-contract y verifica seams/resúmenes/inmutabilidad vía CLI; no cuenta
  como evidencia real de precision/recall.
- [ ] Entrevistar 10–15 developers/equipos y reunir 20 incidentes reales anonimizados.
- [ ] Medir precision/recall, falsos positivos, excepciones y tiempo de resolución por categoría.
- [ ] Ejecutar la matriz hosted Windows/Linux/macOS, evidencia nativa ARM64 y el pipeline Cloudflare.
- [x] Configurar Windows/Linux/macOS con Rust 1.85/1.96, Node 22.13 y un vector de bundles
  byte-exacto; falta evidencia hosted y el pipeline Cloudflare real.
- [ ] Completar tutorial, referencia, cookbook y troubleshooting.
- [ ] Ejecutar todos los snippets en CI.
- [ ] Probar onboarding con developers sin contexto.
- [ ] Corregir bloqueos encontrados en la prueba documental.
- [x] Seleccionar y verificar por máquina la API pública candidata mínima de aplicación/build y los
  contratos one-shot, sin activar todavía una promesa SemVer.
- [ ] Activar la API seleccionada como contrato SemVer al cortar un RC aprobado.
- [x] Crear changelog, política de compatibilidad, smoke downstream y gate local de paquetes; la
  versión RC/final y la publicación real siguen bloqueadas por los gates restantes.
- [x] Ejecutar y congelar el benchmark local final Gate A/Gate B contra Tailwind v4; falta evidencia
  hosted/multiplataforma.
- [ ] Comparar el workflow completo contra Lightning CSS, Tailwind, UnoCSS/PostCSS y los adapters de
  Sass/CSS Modules sin atribuir a PliegoCSS el rendimiento del backend.
- [ ] Probar onboarding audit-only con developers sin contexto y medir tiempo a primer valor.
- [ ] Publicar `0.1.0` solo si R0.1–R0.7 y R0.9 están verdes y R0.8 está cerrado o movido por ADR.

## Contingencia de R0, no alcance adicional

Se reservan exclusivamente para:

- problemas de macros o tiempos de `cargo check`;
- recarga live full-page inestable;
- seams faltantes en PliegoRS;
- incompatibilidades SSR/resumability;
- fallos de fuzzing o determinismo;
- precisión insuficiente o fatiga de diagnósticos en el corpus;
- correcciones derivadas de pruebas con developers.

No se usarán para agregar animaciones avanzadas, plugins, compatibilidad total con Tailwind ni nuevas superficies fuera del MVP.

## Ritmo de trabajo

Cada milestone debe cerrar en este orden:

1. contrato y test del gate;
2. implementación mínima;
3. pruebas unitarias, snapshots y fuzzing proporcional;
4. benchmark cuando aplique;
5. documentación y ejemplos compilables;
6. revisión de output y diagnósticos;
7. commit sin `Co-Authored-By`;
8. siguiente milestone.

No se acumularán varias fases sin commits ni gates intermedios.

## Reporte de progreso

Al cerrar cada semana se entregará:

- tareas cerradas y pendientes;
- tests y gates;
- build cold/incremental;
- tamaño CSS raw/gzip;
- tamaño WASM;
- escapes a CSS normal;
- riesgos descubiertos;
- desviación de ETA;
- decisión explícita de continuar, corregir o reducir alcance.

## Fuera de alcance de `0.1.0`

- Cobertura completa de Tailwind.
- Motor de layout o pintura.
- Parser/minificador CSS propio.
- Runtime WASM de estilos.
- Sistema de plugins abierto.
- IA dentro del build.
- Animaciones avanzadas.
- Integraciones profundas con frameworks distintos de PliegoRS; `audit` genérico y los bridges de
  inventario siguen siendo parte de R0.
