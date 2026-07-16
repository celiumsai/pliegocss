# PliegoRS

PliegoCSS mantiene la integración delgada: `PliegoRS` recibe una clase estándar y no depende del compilador CSS ni de su IR.

## `view!`

`Style` implementa `Display`, por lo que una expresión de atributo puede recibir directamente el resultado de `pc!` o `pcx!`:

```rust
use pliego_css::pc;
use pliego_macros::view;

let page = view! {
    <main class={pc!("grid gap-4 p-6")}>
        "Contenido"
    </main>
};
```

El valor enviado al DOM es una clase determinista `pc_<id>` para la versión de compilador y
`ThemeId` activos. El formato todavía no es un contrato persistente entre versiones publicadas.
`Style::EMPTY` se convierte en una cadena vacía.

## Builder DOM

El builder de PliegoRS acepta `String`, así que la conversión debe ser explícita:

```rust
use pliego_css::pc;

let page = el("main").class(String::from(pc!("grid gap-4 p-6")));
```

## Convivencia con una clase existente

PliegoRS todavía no fusiona dos llamadas a `.class(...)` de forma equivalente entre SSR y DOM. Durante una migración componga un único atributo:

```rust
let style = pc!("grid gap-4 p-6");
let page = el("main").class(format!("legacy-panel {style}"));
```

No emita dos atributos `class` sobre el mismo elemento.

## Hoja generada y SSG verificado

La clase solo identifica la regla. El CSS compilado debe distribuirse como un asset y enlazarse desde el `Head`:

```rust
let head = Head::new("Mi página")
    .stylesheet("/assets/pliego.css")
    .preload_stylesheet("/assets/pliego.css");
let css = Asset::new("assets/pliego.css", include_bytes!("../dist/pliego.css"));
```

`preload_stylesheet(...)` es opt-in y no selecciona assets por sí solo. `Head` rechaza URLs unsafe,
preloads duplicados y cualquier preload que no tenga un `stylesheet(...)` idéntico; los links
aceptados se emiten antes de scripts y hojas aplicadas.

El gate cruzado ya verifica esta costura con `pliego-ssg`. El adapter genera un
[plan declarativo schema 1](../reference/bundle-plan.md) con cinco particiones:
`shared`, `route-home`, `route-visit`, `island-visit-counter` y un control negativo `unreachable`.
El build usa manifest schema 5, poda de StyleIds inalcanzables y el
[asset load plan schema 1](../reference/asset-plan-schema.md), acompañado por
[Project Index schema 1](../reference/project-index-schema.md):

```console
pliego-cssc bundle \
  --plan integration-tests/pliegors-smoke/.pliegocss-bundles.generated.toml \
  --output-dir integration-tests/pliegors-smoke/runtime/bundles \
  --manifest-version 5 \
  --reachability target/pliego.reachability.json \
  --prune-unreachable \
  --asset-plan \
  --project-index
```

`ProductRegistry` es la fuente explícita de componentes, rutas, islands y ocurrencia route→island.
Cada componente usa `product_component!` para capturar el `file!()` normalizado de su declaración;
esto evita duplicar el path, pero no descubre componentes no registrados. El adapter transforma un
snapshot validado en `ApplicationTopology`, inventaría los Rust sources y genera el sidecar neutral
de [reachability schema 1](../reference/reachability-schema.md). También agrupa cada source por su
conjunto exacto de raíces route/island y genera el bundle plan; ya no hay JSON ni TOML de ownership
mantenido a mano. El collector rechaza una invocación visible `pc!`/`pcx!` sin dueño, un site exacto
stale, paths inseguros, symlinks y límites defensivos. No deriva componentes ni rutas desde
filenames. PliegoCSS valida y ordena ese grafo, poda únicamente StyleIds completos y genera
`pliego.assets.json` dentro del mismo grupo de publicación recuperable que los CSS/manifests.
`pliego.index.json` agrega los snapshots de fuente y el join source site → StyleId → declaraciones →
tokens → componentes → declaraciones CSS físicas, sin importar tipos de PliegoRS ni inferir
ownership desde paths.

El consumidor SSG parsea ambos contratos cerrados. Primero verifica el hash del asset plan fijado
por el Project Index, los cinco documentos fuente con sites, todos los backlinks de sites y bundles,
y cada referencia física calificada por bundle. Luego verifica bytes y SHA-256 de cada CSS y
manifest, selecciona la raíz y agrega por separado las islas renderizadas. `/` recibe `shared` +
`route-home`; `/visit` combina `shared` + `route-visit` con `visit-counter`, deduplica `shared` y
termina con `shared` + `island-visit-counter` + `route-visit`. `unreachable` queda podado a un LF,
permanece en ambos inventarios y no cruza al sitio publicado.

Este join ocurre en el fixture, no dentro de `Head`: el asset plan contiene filenames portables, no
URLs, elementos `<link>` ni instrucciones de preload. El Project Index ya sirve como contrato único
para el consumidor PliegoRS de prueba. El collector genera reachability y plan de forma determinista
desde el registro, y el gate elimina ambos sidecars manuales. El adapter SSG selecciona exactamente
el único bundle de ruta con `emitsTheme`: `/assets/shared.css` (560 bytes), compartido por ambas
rutas. No preloada `route-home`, `island-visit-counter` ni `route-visit`. El Asset Plan permanece
neutral a URL/delivery policy. El gate Cargo-aware descrito abajo prueba todos los source units de
los tres targets exactos del fixture; no extiende la afirmación a features o targets que la
aplicación no compila.
Manifest schema 5 agrega graph schema 2 con rangos UTF-8, contribuciones many-to-many y cobertura
física verificada; véanse [manifest schema 4](../reference/manifest-schema-4.md) y
[manifest schema 5](../reference/manifest-schema-5.md).

## Isla resumible, no hidratación

La ruta `/visit` contiene una isla de `pliego-resume` con estado SSR, binding de texto y acción
delegada `increment`. Solo esa ruta enlaza `/assets/pliego-resume.js`; `/` no contiene scripts. El
gate SSG verifica el HTML, el asset de runtime y su aislamiento. El gate CDP adicional genera y
sirve ese sitio, ejecuta el click en Chromium y exige que sobrevivan exactamente los mismos objetos
`document`, island, button y bound-text element; el estado y texto avanzan de 15 a 20, las clases se
conservan, se emite un solo evento tipado y no hay errores de consola o servidor. El mismo replay
exige un solo preload de `shared.css`, la hoja aplicada correspondiente, una entrada Resource Timing
y una única petición al servidor; prueba reutilización, no una mejora de latencia.

PliegoRS no adopta SSR mediante una hidratación general. Su contrato actual resume la isla tocada
por un evento sin reconstruir la vista ni un grafo reactivo. Por eso esta evidencia se denomina
resumability; no debe presentarse como hidratación.

## Compatibilidad

PliegoCSS y PliegoRS declaran Rust 1.85 como versión mínima. La suite del workspace de PliegoCSS se verifica también con ese toolchain.

El gate cruzado vive en `integration-tests/pliegors-smoke` y presupone que `PliegoCSS` y `pliegors`
son directorios hermanos. Es un workspace separado con `Cargo.lock` versionado; usa `pliego-dom` y
`pliego-macros`, `pliego-resume` y `pliego-ssg` desde el checkout hermano. Verifica `view!`, el
builder DOM, SSR directo, ramas precompiladas de `pcx!` y el sitio de dos rutas:

```console
cargo +1.85 test --locked --manifest-path integration-tests/pliegors-smoke/Cargo.toml
```

La superficie fijada incluye también `Cargo.lock`, `crates/pliego-starters/**` y
`crates/pliego-cli/**`. El gate F6 separado materializa
`integration-tests/pliegors-dev-loop`, levanta `pliego-cssc watch` y `pliego dev`, alterna ediciones
válidas 20 veces, comprueba CSS servido contra artifact, padding semántico, ausencia de clase stale,
una sola generación SSE estable y conserva el grupo de publicación ante errores. Puede ejecutarse
con `pnpm integration:pliegors-dev`; la metodología y latencias están en el
[gate del development loop](../benchmarks/pliegors-dev-loop.md).

Este fixture no modifica ni incorpora PliegoRS al workspace de PliegoCSS.

El harness completo genera reachability dos veces desde 12 Rust units compiladas, 5 componentes, 2
rutas, 1 isla y 5 sites, exige bytes idénticos y ausencia total de invocaciones sin ownership.
Las 12 units no provienen de un glob: `cargo check --message-format=json` selecciona `site-lib`,
`site-ssg` y `browser-client`; el adapter resuelve el dep-info exacto emitido por rustc, normaliza sus
paths dentro del fixture y publica un inventario cerrado `rustc-dep-info`. El collector schema 2
escanea la unión completa, aunque solo cinco archivos posean sites de estilo. Un test negativo añade
un source unit compilado con macros no registrados y exige fallo cerrado.

Después de ambos builds, el harness regenera el inventario y compara target membership, source units
y bytes fuente con el snapshot inicial. El vector actual contiene 3 targets / 12 units y SHA-256
`32fc8f07e31adffb1c46aa6b08a333e3f83d33996d7542c4dbd64f883d7cb59d`. Los macros bajo `cfg`
dentro de una unit compilada se escanean conservadoramente aunque la rama esté inactiva; esto puede
rechazar de más, pero no convierte búsqueda textual en certeza. Features y targets no construidos no
forman parte de la prueba.

El harness después
renderiza HTML SSR, demuestra primero que una compilación `all-compiled`
descubre el estilo de `dead.rs`, recompila con schema 5 + pruning + asset plan + Project Index,
construye el sitio dos veces y comprueba clases, fuentes, sites, assets, hashes, selección separada
por ruta/isla, exclusión del bundle muerto, el contrato SSR de la isla y un cliente Rust/WASM real:

```console
pnpm integration:pliegors
pnpm integration:pliegors-browser
```

El segundo comando ejecuta primero todo el gate SSG y luego el replay CDP. Exige que el cliente WASM
marque el documento como listo antes de probar el evento resumible. Use
`PLIEGOCSS_CHROME_PATH` si Chrome/Chromium no está en una ubicación estándar. La evidencia es local
Chromium; no sustituye la matriz hosted Chrome/Firefox/WebKit.

Para ligar ese replay a un Change Receipt, use el boundary explícito del agente después de aplicar
el cambio:

```console
pliego-css-agent run-browser \
  --change-receipt pliego.css.change-receipt.json \
  --source-root . \
  --check-id browser.pliegors \
  --evidence pliego.css.browser-evidence.json
```

El agente sostiene `.pliegocss-repair.lock`, fuerza build limpio, ejecuta únicamente el script fijo
en modo de reporte cerrado y relee sources/profile inputs. La evidencia canónica liga el pin de
PliegoRS y sus bytes cubiertos; `PLIEGORS_ROOT` solo cambia el checkout físico, no el contrato.

Antes de ejecutar Cargo, el harness compara el checkout de PliegoRS con
`integration-tests/pliegors-smoke/pliegors-contract.json`. Deben coincidir:

- la revisión resultante de `git rev-parse HEAD`;
- un SHA-256 determinista sobre rutas relativas y blobs Git del commit para cada superficie
  enumerada explícitamente en el contrato schema 2;
- ausencia de cambios staged, unstaged o untracked dentro de esas superficies.

El hash usa blobs confirmados, no bytes dependientes de conversión CRLF/LF del checkout. La
comprobación separada de estado detecta cambios locales que la revisión Git por sí sola no vería.
Cualquier drift falla antes de compilar y exige revisar la integración antes de actualizar el
contrato fijado. `PLIEGORS_ROOT` puede apuntar a un checkout limpio alternativo sin cambiar el pin.
Cuando difiere del sibling predeterminado, el harness crea un `.cargo/config.toml` temporal y
fail-closed que redirige tanto Cargo externo como el `cargo` hijo de `pliego build` al mismo checkout;
el config entra en el Build Receipt y se elimina al terminar. Un config preexistente no se reemplaza.
En WSL, `PLIEGOCSS_CARGO_TARGET_DIR` puede mover solo los binarios nativos a un filesystem Linux;
las fuentes, artefactos y comprobaciones del contrato permanecen en el checkout montado. Sin ese
override, el gate conserva `target/` del workspace para respetar Windows Application Control.
El path con cliente requiere el target `wasm32-unknown-unknown` para Rust 1.85 y
`wasm-bindgen 0.2.126`; el CLI host puede compilarse con un Rust más nuevo.

Además del smoke SSR original, el harness exige que `/` cargue solo `shared` + `route-home`, que
`/visit` cargue `shared` + `island-visit-counter` + `route-visit`, el runtime resumible y el bootstrap
del cliente, y que cada asset desplegado sea byte por byte idéntico al bundle validado por
`pliego.assets.json` o a su entrada del ledger SSG.
`unreachable.css` no puede existir en el
output SSG. El gate recalcula todas las entradas de `pliego.build.json` y exige dos builds SSG
idénticos. El consumidor SSG aplica el contrato cerrado schema 1, límites defensivos, filenames
derivados sin traversal, unicidad/orden, referencias válidas e identidad e integridad independientes
de cada manifest/CSS; el harness incluye un caso negativo de filename inseguro.
También rechaza un Project Index cuyo hash del asset plan fue alterado y comprueba que sus cinco
snapshots fuente sigan dentro del root confiable, sin symlinks finales ni traversal.

El replay Debian WSL2 de 2026-07-16 midió 31,423 B WASM raw / 12,584 B gzip-9. Resume runtime,
bootstrap, bindgen JS y WASM sumaron 39,962 B raw / 15,186 B gzip-9 al comprimir cada recurso por
separado. Chrome cargó cada recurso una vez, ejecutó el cliente y mantuvo el mismo document/island/
button/value durante el evento 15→20. Son números del fixture y toolchain fijados, no un presupuesto
universal ni evidencia de una aplicación productiva completa.

## Alcance pendiente

Este gate prueba registro de producto validado, collector determinista, partición adapter-derived,
SSR, SSG, assets, poda, selección framework-neutral de bundles por ruta/isla y markup resumible, no
el cierre completo de F5.
La recarga full-page de desarrollo y el evento resumible de producto ya están certificados localmente
en Chromium; falta repetirlos en la matriz hosted/multi-browser. Para F5 siguen pendientes integrar
el flujo como experiencia desde proyecto vacío y probar una aplicación PliegoRS productiva mayor.
La política de performance general/critical CSS
permanece en F7.

Las superficies `pliego-macros`, `pliego-resume` y `pliego-ssg` ya están versionadas en la revisión
fijada de PliegoRS. El contrato cubre sus bytes junto con `pliego-reactive` y `pliego-dom`, por lo que
un clon limpio de esa revisión puede reproducir la misma frontera auditada.

El workflow normal de CI de PliegoCSS no ejecuta este harness porque el checkout hermano de PliegoRS
no forma parte del repositorio. Debe ejecutarse explícitamente en un entorno que tenga ambos
directorios y el contrato de fuentes esperado.
