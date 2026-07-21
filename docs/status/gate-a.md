# Gate A — decisión de viabilidad

Updated: 2026-07-13

Status: **GO condicionado con evidencia limpia local; release hosted y multi-browser pendiente**

Gate A autoriza continuar el desarrollo del núcleo y comenzar F4. No es una aprobación de release
ni afirma que F2/F3 estén completos.

## Evidencia cerrada

- [x] El matched core conserva el DOM de los cinco fixtures y compila 44 atributos de clase.
- [x] El CSS es determinista en 30 procesos fresh y no cambia al emitir el manifest.
- [x] CSS gzip queda en 1,594 B frente a 1,838 B de Tailwind No-preflight Gate-A core.
- [x] HTML transformado + CSS gzip queda en 3,038 B frente a 3,148 B.
- [x] La mediana fresh-process queda en 34.934 ms frente a 198.967 ms.
- [x] `cargo check` pareado centra en +24.316 ms no-op y +28.228 ms changed; los outliers observados
  mientras Defender/WDAC estaban activos quedan registrados y el cold overhead de +9.783 s sigue
  como deuda.
- [x] Diez mutaciones inválidas fallan cerradas con su código `PCS` esperado.
- [x] Valores arbitrarios, custom properties y una propiedad arbitraria llegan al CSS emitido.
- [x] Computed-style smoke en Chromium para core, tres breakpoints, focus, disabled, placeholder y dark.
- [x] Micro-fixture WASM pareado: 424 B raw en control y `pc!`; sin aumento gzip.

## Límites de la decisión

- [x] `theme!` y tokens configurables.
- [x] Targets explícitos, lowering y prefijos de browser bajo el contrato `modern`.
- [ ] Matriz visual y de comportamiento multi-browser automatizada.
- [x] Watch incremental de PliegoCSS para input line-oriented y cambios de config.
- [ ] Medición pareada de una aplicación PliegoRS completa; el micro-fixture `pc!` ya pasa.
- [ ] Contrato full con reset/preflight o reset externo documentado.
- [x] Benchmark PliegoCSS repetido con las 30 muestras de la metodología.
- [x] Snapshots locales reproducibles desde el commit limpio `d16fe5d`, con inputs ligados a blobs Git.
- [ ] Reproducibilidad hosted/paralela y multiplataforma.
- [ ] Interoperabilidad general con CSS normal.

La comparación primaria usa Tailwind No-preflight core. La columna Full incluye Preflight, que
PliegoCSS no tiene; no se usa para afirmar superioridad bajo un contrato equivalente. El reporte
completo, incluyendo el coste gzip de las clases hash y las deudas de diagnóstico, está en
[`pliego-gate-a.md`](../benchmarks/pliego-gate-a.md).
El smoke de navegador está en [`browser-validation.md`](../benchmarks/browser-validation.md).

## Decisión operativa

Se continúa sin reducir el modelo semántico. F4 puede trabajar variantes y composición mientras las
deudas anteriores siguen bloqueando cualquier afirmación de producción o `0.1.0`.
