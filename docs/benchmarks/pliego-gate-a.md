# PliegoCSS Gate A benchmark

Date: 2026-07-13

Status: **GO condicionado para continuar el desarrollo**

Gate A demuestra que el núcleo semántico es viable en el subconjunto core de los cinco fixtures.
Incluye un smoke posterior de estilos computados en Chromium, pero no demuestra todavía
compatibilidad con una matriz de browsers ni impacto nulo en una aplicación PliegoRS/WASM.

## Contrato medido

La medición usa el mismo DOM core para button, card, navbar, form y dashboard: 44 atributos
`class`, 28 listas de estilo únicas, 257 apariciones de utilidades y 65 utilidades únicas. Se excluye
el corpus avanzado de variantes, que pertenece a Gate B.

PliegoCSS compila cada valor completo de `class` como un estilo semántico, agrupa sus declaraciones
bajo una clase `pc_*`, incluye el tema seed y pasa el artefacto por Lightning CSS. La compilación del
binario release queda fuera del cronómetro; cada muestra lanza un proceso nuevo.

La comparación primaria es **Tailwind No-preflight Gate-A core**, porque PliegoCSS aún no emite
reset/preflight. Tailwind Full Gate-A core se muestra como contexto, no como una victoria bajo el
mismo contrato. Incluso en la comparación no-preflight los payloads de tema no son idénticos:
PliegoCSS incluye un seed pequeño de colores y fuentes, mientras Tailwind conserva su propia capa de
tema. La equivalencia funcional de esos valores se congeló para el fixture, pero los bytes de
infraestructura no son los mismos.

## Resultado principal

| Métrica | PliegoCSS matched core | Tailwind Full core | Tailwind No-preflight core |
|---|---:|---:|---:|
| Fresh-process minificado, mediana | 34.934 ms | 202.605 ms | 198.967 ms |
| Fresh-process minificado, mínimo | 31.959 ms | 187.682 ms | 182.907 ms |
| Fresh-process minificado, p95 | 248.502 ms | 265.907 ms | 269.550 ms |
| CSS raw | 6,546 B | 10,379 B | 6,620 B |
| CSS gzip | 1,594 B | 2,879 B | 1,838 B |
| HTML generado gzip | 1,444 B | 1,310 B | 1,310 B |
| HTML generado + CSS gzip | 3,038 B | 4,189 B | 3,148 B |

Contra Tailwind No-preflight core, PliegoCSS muestra una mediana fresh-process 82.4% menor
(5.70x), CSS raw 1.1% menor, CSS gzip 13.3% menor y transferencia operativa HTML+CSS 3.5% menor.
Contra Tailwind Full core, las diferencias son 82.8%, 36.9%, 44.6% y 27.5% menores,
respectivamente, pero esa columna incluye un reset que PliegoCSS no implementa.

Veintiocho muestras quedaron entre 31.959 y 38.614 ms; dos arranques altos midieron 248.502 y
267.676 ms. Por eso la media de 49.903 ms y la desviación estándar de 55.713 ms
quedan por encima del camino habitual. La mediana convencional y su MAD de 0.708 ms resisten esos
outliers. Tanto PliegoCSS como el baseline Tailwind congelado usan cinco warmups y 30 muestras.

## Coste de clases en HTML

Reemplazar las listas utility-first por hashes `pc_*` reduce el HTML raw de 5,007 B a 3,916 B, pero
empeora su gzip de 1,310 B a 1,444 B: los nombres hash repetidos comprimen peor que el vocabulario
repetitivo de utilidades. La cifra operativa de PliegoCSS es por ello 3,038 B, no 2,904 B.

Los 2,904 B son un control útil — HTML original con utility strings más el CSS de PliegoCSS — pero
no representan el contrato actual de render. Aun pagando los 134 B extra de HTML gzip, el total
queda 110 B por debajo de Tailwind No-preflight core.

## Determinismo

Los 30 procesos medidos produjeron exactamente el mismo CSS:

```text
aebdbff60a76abc1347cc34818bd31d54502c63505fb4962b1b6b3af46f5bf8e
```

The measured release executable SHA-256 was
`4c8f1964fda895e7f29213af2456bd79905b7b2e79b6ecadcc81124c3e444a73`.

El harness también comprueba que generar el manifest no cambia el CSS, que las 28 identidades no
colisionan y que el HTML transformado conserva todos los bytes externos a los valores de `class`.
Esto prueba reproducibilidad fresh-process para este ejecutable, fixture y entorno; no cubre todavía
builds clean/incrementales/paralelos, otros targets ni estabilidad entre versiones del compilador.

## `cargo check` pareado

| Escenario | Pares | Mediana plain | Mediana styled | Mediana delta pareado | Mediana delta % | MAD del delta |
|---|---:|---:|---:|---:|---:|---:|
| Target frío | 6 | 378.025 ms | 10,160.012 ms | +9,782.699 ms | +2,848.833% | 254.188 ms |
| Check no-op | 30 | 65.811 ms | 90.002 ms | +24.316 ms | +37.407% | 2.403 ms |
| Literal de estilo cambiado | 30 | 129.825 ms | 159.112 ms | +28.228 ms | +21.614% | 138.672 ms |

Las columnas plain/styled son resúmenes marginales y no se restan entre sí; el delta autoritativo se
calcula dentro de cada par adyacente y luego toma su mediana.

El centro incremental de esta máquina queda alrededor de 24–28 ms, aceptable para continuar. La
distribución conserva outliers positivos y negativos mientras Defender y Windows Application Control
estaban activos. No-op queda estrecho (MAD 2.403 ms); changed exhibe gran dispersión (MAD 138.672
ms), por lo que su mediana no es una cifra de precisión. El cold overhead de 9.78 s sí es deuda de
optimización explícita. Metodología, muestras y baseline
superseded están en [`rust-check-baseline.md`](./rust-check-baseline.md).

## Diagnósticos y libertad de CSS

`pnpm baseline:check-diagnostics` verificó que diez entradas inválidas fallan cerradas con el código
`PCS` esperado: utilidad y token desconocidos, dominio incorrecto, conflicto, variante incompleta,
breakpoint desconocido, valor arbitrario desbalanceado, important duplicado y condiciones
imposibles o redundantes. Los compile-fail tests conservan spans sobre el literal y ejemplos de
sugerencias como `flec` -> `flex`.

Esto ya es mejor que aceptar silenciosamente una clase desconocida, pero aún no existe una
puntuación head-to-head de cada explicación y sugerencia contra Tailwind. El harness de diez casos
comprueba el código y el fallo cerrado; no audita automáticamente que todas las sugerencias
esperadas compilen.

Los valores arbitrarios preservan el escape necesario para layouts y valores CSS reales:
`gap-[clamp(...)]`, `grid-cols-[...]`, variables como `gap-(--card-gap)` y una declaración
`[property:value]`. Parser, lowering y emisión tienen contratos black-box para estos casos. Las
restricciones visibles son intencionales por ahora: las propiedades arbitrarias se identifican por
nombre, los selectores arbitrarios usan el subconjunto validado `[&...]`, y la validación CSS completa
ocurre en el artefacto CLI, no en la macro aislada. Las hojas CSS normales siguen siendo la salida
explícita para la cola larga de la plataforma.

## Evaluación de los criterios

| Criterio de Gate A | Estado | Evidencia o límite |
|---|---|---|
| Velocidad de sintaxis cercana a Tailwind | Aceptado provisionalmente | El fixture conserva vocabulario utility-first familiar; falta estudio cronometrado de autoría. |
| Errores claramente mejores | Pasa con deuda | Validación semántica, spans, códigos y sugerencias; falta scoring comparativo completo. |
| CSS gzip igual o menor | Pasa | 1,594 B frente a 1,838 B no-preflight bajo el core matched. |
| `cargo check` incremental aceptable | Pasa con ruido host registrado | +24.316 ms no-op y +28.228 ms changed; outliers completos preservados. |
| Sin aumento de WASM | Pasa en micro-fixture | Control y `pc!` producen 424 B raw; falta la app PliegoRS pareada. |
| Valores arbitrarios suficientemente libres | Pasa con límites documentados | Valores, custom properties y una declaración arbitraria sobreviven hasta CSS. |

## Decisión

**GO condicionado.** Los datos no justifican abandonar ni reducir el núcleo tipado: la latencia
fresh-process, el CSS gzip, el centro incremental y el escape arbitrario están del lado correcto del
gate. El cold Rust build y la varianza del host permanecen deudas visibles. Se puede continuar sin
reabrir la arquitectura fundamental, pero no declarar listo un release.

Este GO no aprueba producción ni cierra F2/F3 completos. Las deudas que deben permanecer visibles
son:

- matriz automatizada Firefox/WebKit/Chromium y regresión visual;
- medición de WASM en una aplicación PliegoRS pareada;
- contrato full con reset/preflight o una política explícita de reset externo;
- verificación multiplataforma;
- reachability por ruta/island y procedencia a nivel de declaración/token.

Después de este snapshot se implementaron manifest schema 4 con reachability explícito y
declaraciones semánticas/token, schema 5 con trace físico fail-closed y poda opt-in de reglas
inalcanzables desde el sidecar. Siguen pendientes el collector automático, la poda de variables/token
y la validación end-to-end; el alcance histórico de Gate A no se reescribe por esos avances posteriores.

El micro-fixture WASM está documentado en [`wasm-overhead.md`](./wasm-overhead.md).

## Reproducción

```console
pnpm baseline:measure-pliego
pnpm baseline:measure-rust
pnpm baseline:check-diagnostics
```

El resultado máquina-local se escribe en `benchmarks/results/pliego-gate-a.local.json`. El snapshot
inmutable del commit limpio `d16fe5d` está en
[`pliego-gate-a-2026-07-21-d16fe5d.json`](../../benchmarks/evidence/pliego-gate-a-2026-07-21-d16fe5d.json).
La metodología y comparaciones están en
[`tailwind-v4-baseline.md`](./tailwind-v4-baseline.md) y
[`rust-check-baseline.md`](./rust-check-baseline.md).
La evidencia de estilos computados está en [`browser-validation.md`](./browser-validation.md).
