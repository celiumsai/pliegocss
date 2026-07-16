# PliegoCSS — plan de documentación

Fecha: 2026-07-12

## Regla del proyecto

Una funcionalidad no está terminada hasta que tenga:

1. API y comportamiento implementados;
2. pruebas automatizadas;
3. rustdoc en toda API pública;
4. ejemplo mínimo compilable;
5. referencia actualizada;
6. mensaje de error y troubleshooting cuando aplique.
7. schema y ejemplo machine-readable cuando exponga un contrato para CI/agentes;
8. límites, evidencia no verificada y no-claims explícitos.

La documentación es parte del Definition of Done, no una fase cosmética posterior.

## Audiencias

### Developer nuevo

Quiere instalar PliegoCSS, auditar CSS existente sin migrarlo y comprender un hallazgo antes de
activar gates o compilación.

### Developer de producto

Necesita compatibilidad, cascade, responsive, estados, temas, CSS externo, presupuestos y debugging.

### Autor de design systems

Necesita definir tokens, aliases, temas, contraste, componentes base, excepciones y políticas.

### Integrador de agentes y CI

Necesita códigos estables, spans exactos, JSON/SARIF, planes acotados, dry-runs, idempotencia y
receipts verificables sin depender del texto humano.

### Integrador o autor de plugins

Necesita comprender IR, catálogo, pipeline, manifests, estabilidad y APIs de extensión.

### Contribuidor del framework

Necesita arquitectura, invariantes, ADRs, tests, benchmarks y proceso de releases.

## Arquitectura de información

```text
docs/
├── index.md
├── product/
│   ├── strategic-product-contract-2026.md
│   └── research-requirements.md
├── getting-started/
│   ├── installation.md
│   ├── first-audit.md
│   ├── first-component.md
│   ├── project-structure.md
│   └── editor-setup.md
├── learn/
│   ├── mental-model.md
│   ├── utilities.md
│   ├── responsive.md
│   ├── states-and-variants.md
│   ├── themes-and-tokens.md
│   ├── composition.md
│   ├── dynamic-values.md
│   ├── ssr-and-hydration.md
│   └── css-interop.md
├── how-to/
│   ├── build-a-button.md
│   ├── build-a-form.md
│   ├── build-a-navbar.md
│   ├── build-a-dashboard.md
│   ├── dark-mode.md
│   ├── custom-breakpoints.md
│   ├── use-external-css.md
│   ├── configure-budgets.md
│   ├── configure-accessibility-policies.md
│   ├── inspect-a-receipt.md
│   ├── debug-generated-css.md
│   └── migrate-from-tailwind.md
├── reference/
│   ├── utilities/
│   ├── variants.md
│   ├── directives.md
│   ├── macros.md
│   ├── theme-schema.md
│   ├── dtcg-bridge.md
│   ├── configuration.md
│   ├── cli.md
│   ├── diagnostic-schema.md
│   ├── sarif.md
│   ├── manifest-and-receipt.md
│   ├── policy-schema.md
│   └── browser-support.md
├── concepts/
│   ├── compiler-pipeline.md
│   ├── typed-ir.md
│   ├── conflict-model.md
│   ├── determinism.md
│   ├── policy-engine.md
│   ├── agent-trust-loop.md
│   ├── verified-unverified-manual.md
│   ├── reachability.md
│   └── provenance.md
├── cookbook/
│   ├── layouts.md
│   ├── typography.md
│   ├── forms.md
│   ├── navigation.md
│   ├── data-display.md
│   └── animation.md
├── migration/
│   ├── from-tailwind.md
│   ├── from-css-modules.md
│   └── from-plain-css.md
├── contributing/
│   ├── architecture.md
│   ├── repository.md
│   ├── add-a-utility.md
│   ├── add-a-variant.md
│   ├── testing.md
│   ├── benchmarks.md
│   ├── release-process.md
│   └── compatibility-policy.md
└── adr/
    └── README.md
```

## Tipos de documentación

Se aplicará la separación de cuatro necesidades:

- **Tutorial:** aprendizaje guiado de principio a fin.
- **How-to:** resolver una tarea concreta sin explicar toda la arquitectura.
- **Referencia:** catálogo exacto y completo de la API vigente.
- **Explicación:** razones, modelo mental, tradeoffs e invariantes.

Mezclarlas en una sola página produce documentación difícil de navegar.

## Documentación generada desde código

La información que pueda divergir no se mantendrá manualmente.

El catálogo interno de utilidades será la fuente de verdad para generar:

- nombre y aliases;
- propiedad CSS emitida;
- valores aceptados;
- tokens compatibles;
- variantes permitidas;
- browser support;
- ejemplos;
- versión de introducción y deprecación.

La misma fuente alimentará parser, autocompletado, referencia web y mensajes de error.

## Contrato de ejemplos

Todo snippet marcado como ejecutable debe:

- compilar en CI;
- usar la versión actual de la API;
- mostrar imports completos cuando sea tutorial;
- indicar output CSS cuando sea relevante;
- incluir accesibilidad básica;
- evitar APIs ficticias o futuras sin una etiqueta visible.

Los ejemplos grandes vivirán como proyectos reales bajo `examples/`, y las páginas incluirán su fuente para evitar duplicación.

## Documentación por fase

### F0

- Glosario.
- Principios de diseño.
- ADRs iniciales.
- Contrato de sintaxis.
- Página de estado: experimental y no apto para producción.

### F1–F2

- Gramática de utilidades.
- Tokens y temas.
- Errores y conflictos.
- Valores arbitrarios.
- Referencia generada del catálogo implementado.

### F3–F4

- Pipeline CSS.
- Variantes y responsive.
- Precedencia.
- Composición y estados condicionales.
- Inspección del output.

### F5

- Tutorial completo PliegoRS + PliegoCSS.
- SSR e hidratación.
- Rutas e islands.
- CSS externo.
- Contrato plain HTML con input line-oriented, manifest y cero runtime de estilos.

### F6–F7

- CLI y configuración.
- Editor y hot reload.
- Troubleshooting.
- Optimización, reachability y benchmarks.

### E0–E1 / R0 audit

- Contrato estratégico y matriz de traceability.
- Primer audit sin migración.
- Schema de diagnóstico congelado y guía de códigos/spans/evidencia.
- CSS estándar, backend, targets y compatibilidad-data date.
- Limitaciones del análisis estático y delegación a browser real.

### E2 / R0 guard

- Budgets por ruta/paquete/layer y deltas.
- Token graph, aliases, ciclos, themes, DTCG y blast radius.
- Contraste, motion, focus y forced-colors con cobertura verified/unverified/manual.
- Excepciones, expiración y troubleshooting de falsos positivos.

### E3–E4 / R0 agent y release

- Paridad humana/JSON/SARIF.
- `explain-cascade` estático acotado: precedencia, schema, spans exactos, blocker codes,
  `browser-required` y límites frente a la cascada/computed style reales.
- Proposal/plan/dry-run schema 1.0.0: autoridad low-risk, exact byte edits, FindingDocument/source
  binding, budgets, patch/hash, idempotencia `ready|already-applied` y seguridad de paths.
- Apply/Change Receipt schema 1.0.0: token externo como selección exacta, no autenticación; lock,
  staging/rollback, colisiones, re-ejecución no-op y non-claims explícitos de `checks-pending`,
  checks `not-run`, browser evidence `not-collected` y ausencia de verificación final.
- Check policy/Verification Receipt schema 1.4.0 con lectura canónica
  1.0.0/1.1.0/1.2.0/1.3.0: registro
  cerrado sin shell/argv en policy/plan, auditoría CSS, integridad token-graph, budget CSS
  identity-bound, `test-suite-evidence` producida por un runner Cargo explícito de perfil fijo y
  `browser-evidence` producida por el runner PliegoRS/Chromium fijo. Debe documentar binding al
  Change Receipt/root manifest/lockfile y a los ocho browser profile inputs, cobertura source-specific
  aditiva, `passed|failed|blocked`, códigos de salida, FindingDocuments adyacentes
  create-if-absent, límites de no-hermeticidad y estados browser
  `not-required|required-not-collected|required-passed|required-failed`.
- Corpus agent: distinguir el harness sintético de conformidad del corpus revisado de incidentes;
  documentar provenance, consent/redaction, hashes, mismo modelo/surface, turnos, receipts, checks y
  violations antes/después sin convertir casos sintéticos en evidencia de investigación.
- Migration inventory schema 1: semántica `static|dynamic|unsupported`, dependencia Preflight,
  bounds, paths/hashes/spans, API Rust y límites explícitos frente a ejecución, project graphs y
  promesas de migración perfecta.
- Manifest y receipt unificados, verificación e idempotencia.
- Bridges e inventarios Sass/Tailwind/CSS Modules.
- Migraciones, cookbook, guías de contribución, compatibilidad/versionado y portal navegable.

## Definition of Done documental por tarea

Cada PR o milestone debe responder:

- ¿Qué aprende el developer?
- ¿Dónde encuentra la referencia exacta?
- ¿Existe un ejemplo compilable?
- ¿Qué error verá si lo usa mal?
- ¿Está documentada la interoperabilidad con CSS?
- ¿Cambió una decisión arquitectónica que requiere ADR?
- ¿El contenido puede generarse desde el catálogo en lugar de duplicarse?
- ¿Human, JSON y SARIF expresan el mismo diagnóstico?
- ¿Se documentan causa, evidencia, riesgo, alcance, prerequisitos y excepción?
- ¿Distingue verified, unverified, manual, unobserved y dead sin sobreprometer?
- ¿El manifest/receipt permite demostrar exactamente qué inputs, policy y output se revisaron?

## Calidad y verificación

CI debe verificar:

- `cargo test --doc`;
- links internos y externos;
- snippets compilables;
- catálogo web sincronizado con el registro de utilidades;
- ausencia de APIs públicas sin rustdoc;
- ejemplos completos contra la versión actual;
- output CSS mostrado mediante snapshots;
- spelling de términos reservados y nombres de utilidades.
- paridad semántica de diagnósticos human/JSON/SARIF;
- schemas, ejemplos y receipts contra fixtures byte-exactos;
- links entre código estable, policy, evidencia, excepción y troubleshooting;
- ausencia de claims de WCAG automático, browser simulation, migración perfecta o compatibilidad no
  demostrada.

Antes de `0.1.0`, al menos dos developers sin contexto previo deben completar:

1. instalación;
2. primer `audit` sobre un repo CSS existente sin migración;
3. explicación y excepción justificada de un diagnóstico;
4. activación de un budget y lectura de su delta;
5. inspección/verificación de manifest y receipt;
6. consumo equivalente human/JSON/SARIF;
7. integración PliegoRS o uso de un bridge genérico.

Se registrarán tiempo, bloqueos y preguntas. Si necesitan explicación oral para completar el flujo, la documentación no pasa el gate.

## Portal

El portal debe ofrecer:

- búsqueda por utilidad, propiedad o concepto;
- ejemplos Rust y CSS generado lado a lado;
- indicador de estabilidad por API;
- compatibilidad por navegador;
- enlaces directos a diagnósticos;
- selector de versión;
- playground cuando el compilador sea estable;
- navegación rápida para usuarios provenientes de Tailwind.

La implementación visual del portal puede esperar hasta F6–F8. El contenido fuente comienza en F0.

## Estimación

Documentar en paralelo agrega aproximadamente 15–20% al esfuerzo de implementación, pero evita una fase tardía de reconstrucción y fuerza APIs explicables.

La estimación cerrada para ejecución con GPT-5.6 Ultra es:

- schemas y primer audit documentados: 1–2 semanas;
- R0 técnico y documental desde el estado actual: 6–10 semanas calendario;
- validación de producto: depende del acceso a 10–15 entrevistas y 20 incidentes reales;
- R1–R3 se estiman y documentan después de verificar R0.

## Entregable del MVP

PliegoCSS `0.1.0` no se publica hasta contar con:

- tutorial de inicio completo;
- referencia generada de todas las utilidades soportadas;
- documentación de macros, tema, configuración y CLI;
- guías de responsive, variantes, SSR y CSS externo;
- migración inicial desde Tailwind;
- ejemplos reales compilados en CI;
- arquitectura y contribución documentadas;
- troubleshooting de los diagnósticos principales.
