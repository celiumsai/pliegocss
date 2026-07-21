// SPDX-License-Identifier: Apache-2.0

import gsap from "gsap";
import { ScrollTrigger } from "gsap/ScrollTrigger";
import Lenis from "lenis";

gsap.registerPlugin(ScrollTrigger);

const reducedMotion = matchMedia("(prefers-reduced-motion: reduce)");
const saveData = navigator.connection?.saveData === true;
const disposers = [];
const cache = new Map();
const spanish = document.documentElement.lang === "es";
let messages = {};

const clientSpanish = {
  ACCEPTED: "ACEPTADO",
  COPIED: "COPIADO",
  ERROR: "ERROR",
  SELECT: "SELECCIONAR",
  "Base padding applies until the md condition becomes true.":
    "El padding base se aplica hasta que la condición md sea verdadera.",
  "Build topology": "Topología de compilación",
  "Catalog unavailable": "Catálogo no disponible",
  "Close the field note": "Cerrar la nota de campo",
  "Compiler corpus unavailable": "Corpus del compilador no disponible",
  "Corpus unavailable": "Corpus no disponible",
  "Cycle proof": "Recorrer prueba",
  "Diagnostic corpus unavailable": "Corpus de diagnósticos no disponible",
  "Each condition owns one value. The responsive state is explicit.":
    "Cada condición posee un valor. El estado responsive es explícito.",
  "Evidence replaying…": "Reproduciendo evidencia…",
  "Explanation corpus unavailable": "Corpus de explicaciones no disponible",
  "Folio added ✓": "Folio agregado ✓",
  "LATEST BUILDS": "COMPILACIONES RECIENTES",
  "No exact surface found.": "No se encontró una superficie exacta.",
  "STATE SPACE EXPLICIT": "ESPACIO DE ESTADOS EXPLÍCITO",
  Utilities: "Utilidades",
  "secured in the order.": "asegurado en el pedido.",
  "selected.": "seleccionado.",
};

function tr(value) {
  return spanish && typeof value === "string"
    ? messages[value] ?? clientSpanish[value] ?? value
    : value;
}

function localizedHref(href) {
  if (
    !spanish ||
    typeof href !== "string" ||
    !href.startsWith("/") ||
    href.startsWith("/assets/") ||
    href.startsWith("/media/") ||
    (href.startsWith("/brand/") && href !== "/brand/") ||
    href.startsWith("/fonts/") ||
    href.startsWith("/es/")
  ) {
    return href;
  }
  const match = href.match(/^([^?#]*)(.*)$/u);
  const path = match?.[1] ?? href;
  const suffix = match?.[2] ?? "";
  return `${path === "/" ? "/es/" : `/es${path}`}${suffix}`;
}

function addDisposable(dispose) {
  disposers.push(dispose);
}

async function fetchJson(path) {
  if (!cache.has(path)) {
    cache.set(
      path,
      fetch(path, { cache: "force-cache" }).then((response) => {
        if (!response.ok) throw new Error(`${path}: HTTP ${response.status}`);
        return response.json();
      }),
    );
  }
  return cache.get(path);
}

async function setupLocale() {
  if (!spanish) return;
  try {
    messages = await fetchJson("/assets/i18n-es.json");
  } catch (error) {
    console.warn("PliegoCSS Spanish client catalog unavailable", error);
  }
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

function setupScroll() {
  const updateProgress = () => {
    const scrollable = document.documentElement.scrollHeight - innerHeight;
    const progress = scrollable > 0 ? (scrollY / scrollable) * 100 : 0;
    document.documentElement.style.setProperty("--scroll-progress", `${progress}%`);
  };
  addEventListener("scroll", updateProgress, { passive: true });
  addEventListener("resize", updateProgress, { passive: true });
  updateProgress();
  addDisposable(() => {
    removeEventListener("scroll", updateProgress);
    removeEventListener("resize", updateProgress);
  });

  if (reducedMotion.matches || saveData) return;
  const lenis = new Lenis({
    duration: 1,
    smoothWheel: true,
    syncTouch: false,
    wheelMultiplier: 0.92,
  });
  lenis.on("scroll", ScrollTrigger.update);
  const tick = (time) => lenis.raf(time * 1000);
  gsap.ticker.add(tick);
  gsap.ticker.lagSmoothing(0);
  addDisposable(() => {
    gsap.ticker.remove(tick);
    lenis.destroy();
  });
}

function setupMotion() {
  if (reducedMotion.matches) return;
  const context = gsap.context(() => {
    if (
      document.querySelector("[data-hero]") &&
      matchMedia("(min-width: 701px)").matches
    ) {
      gsap
        .timeline({ defaults: { ease: "power3.out" } })
        .from(".site-header", { y: -24, opacity: 0, duration: 0.65 })
        .from(".hero .kicker", { y: 18, opacity: 0, duration: 0.5 }, "-=0.2")
        .from(".hero h1 span", { yPercent: 95, opacity: 0, stagger: 0.1, duration: 1.05 }, "-=0.2")
        .from(".hero-lede", { y: 30, opacity: 0, duration: 0.72 }, "-=0.55")
        .from(".hero-actions", { y: 22, opacity: 0, duration: 0.58 }, "-=0.5")
        .from(".hero-terminal", { x: 72, opacity: 0, duration: 0.82 }, "-=0.62");
    }

    const proofStrip = document.querySelector(".proof-marquee > div");
    if (proofStrip) {
      gsap.fromTo(
        proofStrip,
        { xPercent: 0 },
        {
          xPercent: -24,
          ease: "none",
          scrollTrigger: {
            trigger: proofStrip.parentElement,
            start: "top bottom",
            end: "bottom top",
            scrub: true,
          },
        },
      );
    }

    ScrollTrigger.batch("[data-reveal]", {
      start: "top 90%",
      once: true,
      onEnter: (elements) =>
        gsap.from(elements, {
          y: 44,
          opacity: 0,
          duration: 0.82,
          stagger: 0.06,
          ease: "power3.out",
          clearProps: "transform,opacity",
        }),
    });

    const theatre = document.querySelector("[data-theatre]");
    if (theatre && matchMedia("(min-width: 701px)").matches) {
      gsap.fromTo(
        ".theatre-workbench",
        {
          y: 54,
          scale: 0.985,
          opacity: 0.72,
        },
        {
          y: 0,
          scale: 1,
          opacity: 1,
          duration: 0.72,
          ease: "power3.out",
          clearProps: "transform,opacity",
          scrollTrigger: {
            trigger: ".theatre-workbench",
            start: "top 94%",
            once: true,
          },
        },
      );
    }

    const conflict = document.querySelector("[data-conflict-lab]");
    if (conflict) {
      gsap.from(".diagnostic-console", {
        clipPath: "inset(0 100% 0 0)",
        ease: "none",
        scrollTrigger: {
          trigger: conflict,
          start: "top 70%",
          end: "top 22%",
          scrub: true,
        },
      });
    }

    const story = document.querySelector("[data-cascade-story]");
    if (story) {
      gsap.from("[data-cascade-step]", {
        x: -34,
        opacity: 0.18,
        stagger: 0.12,
        scrollTrigger: {
          trigger: story,
          start: "top 64%",
          end: "bottom 72%",
          scrub: 0.7,
        },
      });
      gsap.from(".rail-source", {
        scaleX: 0.03,
        scrollTrigger: { trigger: story, start: "top 75%", end: "center 45%", scrub: true },
      });
      gsap.from(".rail-output", {
        scaleX: 0.02,
        scrollTrigger: { trigger: story, start: "top 58%", end: "bottom 55%", scrub: true },
      });
      gsap.fromTo(
        ".cascade-code",
        { y: 56, opacity: 0.55 },
        {
          y: 0,
          opacity: 1,
          ease: "power2.out",
          clearProps: "transform,opacity",
          scrollTrigger: {
            trigger: ".cascade-code",
            start: "top 92%",
            once: true,
          },
        },
      );
    }

    ScrollTrigger.batch(".utility-card", {
      start: "top 94%",
      once: true,
      batchMax: 8,
      onEnter: (cards) =>
        gsap.from(cards, {
          y: 28,
          opacity: 0,
          duration: 0.55,
          stagger: 0.035,
          clearProps: "transform,opacity",
        }),
    });
  });
  addDisposable(() => context.revert());
}

async function setupHeroCanvas() {
  const canvas = document.querySelector("[data-hero-canvas]");
  const host = canvas?.parentElement;
  if (!canvas || !host || saveData || reducedMotion.matches) {
    if (canvas) document.documentElement.classList.add("no-webgl");
    return;
  }
  const {
    BoxGeometry,
    CatmullRomCurve3,
    DoubleSide,
    Group,
    Mesh,
    MeshBasicMaterial,
    PerspectiveCamera,
    PlaneGeometry,
    Scene,
    TubeGeometry,
    Vector3,
    WebGLRenderer,
  } = await import("./three-runtime.js");

  let renderer;
  try {
    renderer = new WebGLRenderer({
      canvas,
      alpha: true,
      antialias: devicePixelRatio <= 1.5,
      powerPreference: "high-performance",
    });
  } catch {
    document.documentElement.classList.add("no-webgl");
    return;
  }

  const scene = new Scene();
  const camera = new PerspectiveCamera(36, 1, 0.1, 100);
  camera.position.set(0, 0, 10);
  const group = new Group();
  group.position.set(2.8, 0.15, -0.5);
  group.rotation.set(-0.2, -0.54, -0.08);
  scene.add(group);

  const planeGeometry = new PlaneGeometry(5.4, 3.3);
  const colors = [0x11151a, 0x283039, 0x2855ff, 0x65d9ff];
  const planes = colors.map((color, index) => {
    const material = new MeshBasicMaterial({
      color,
      transparent: true,
      opacity: index === 3 ? 0.16 : 0.22 + index * 0.08,
      side: DoubleSide,
      wireframe: index === 3,
    });
    const plane = new Mesh(planeGeometry, material);
    plane.position.set(index * 0.32, -index * 0.18, -index * 0.56);
    plane.rotation.z = index * 0.035;
    group.add(plane);
    return { plane, material };
  });

  const railPoints = [
    new Vector3(-3.6, 1.25, 1.1),
    new Vector3(-1.25, 1.25, 0.8),
    new Vector3(-1.25, 0, 0.2),
    new Vector3(0.8, 0, -0.25),
    new Vector3(0.8, -1.2, -0.65),
    new Vector3(3.4, -1.2, -0.95),
  ];
  const curve = new CatmullRomCurve3(railPoints, false, "catmullrom", 0.03);
  const railGeometry = new TubeGeometry(curve, 128, 0.065, 8, false);
  const railMaterial = new MeshBasicMaterial({ color: 0xf4f7f8 });
  const rail = new Mesh(railGeometry, railMaterial);
  group.add(rail);

  const traceCurve = new CatmullRomCurve3(
    railPoints.map((point, index) => point.clone().add(new Vector3(0.35, -0.5, -0.4 - index * 0.02))),
    false,
    "catmullrom",
    0.03,
  );
  const traceGeometry = new TubeGeometry(traceCurve, 128, 0.08, 8, false);
  const traceMaterial = new MeshBasicMaterial({ color: 0x2855ff });
  const trace = new Mesh(traceGeometry, traceMaterial);
  group.add(trace);

  const nodeGeometry = new BoxGeometry(0.16, 0.16, 0.16);
  const nodeMaterial = new MeshBasicMaterial({ color: 0x65d9ff });
  const nodes = railPoints.slice(1, -1).map((point, index) => {
    const node = new Mesh(nodeGeometry, nodeMaterial);
    node.position.copy(point).add(new Vector3(0.35, -0.5, -0.4 - (index + 1) * 0.02));
    group.add(node);
    return node;
  });

  let pointerX = 0;
  let pointerY = 0;
  let running = true;
  let frame = 0;
  let last = performance.now();
  const resize = () => {
    const bounds = host.getBoundingClientRect();
    renderer.setPixelRatio(Math.min(devicePixelRatio, 1.5));
    renderer.setSize(Math.max(1, bounds.width), Math.max(1, bounds.height), false);
    camera.aspect = bounds.width / Math.max(1, bounds.height);
    camera.updateProjectionMatrix();
  };
  const onPointer = (event) => {
    pointerX = event.clientX / innerWidth - 0.5;
    pointerY = event.clientY / innerHeight - 0.5;
  };
  const render = (time) => {
    if (!running) return;
    const delta = Math.min(0.05, (time - last) / 1000);
    last = time;
    const targetY = reducedMotion.matches ? -0.54 : -0.54 + pointerX * 0.22;
    const targetX = reducedMotion.matches ? -0.2 : -0.2 + pointerY * 0.12;
    group.rotation.y += (targetY - group.rotation.y) * Math.min(1, delta * 3);
    group.rotation.x += (targetX - group.rotation.x) * Math.min(1, delta * 3);
    renderer.render(scene, camera);
    frame = requestAnimationFrame(render);
  };
  const onVisibility = () => {
    running = !document.hidden;
    cancelAnimationFrame(frame);
    if (running) {
      last = performance.now();
      frame = requestAnimationFrame(render);
    }
  };
  resize();
  addEventListener("resize", resize, { passive: true });
  addEventListener("pointermove", onPointer, { passive: true });
  document.addEventListener("visibilitychange", onVisibility);
  frame = requestAnimationFrame(render);

  addDisposable(() => {
    running = false;
    cancelAnimationFrame(frame);
    removeEventListener("resize", resize);
    removeEventListener("pointermove", onPointer);
    document.removeEventListener("visibilitychange", onVisibility);
    planeGeometry.dispose();
    planes.forEach(({ material }) => material.dispose());
    railGeometry.dispose();
    railMaterial.dispose();
    traceGeometry.dispose();
    traceMaterial.dispose();
    nodeGeometry.dispose();
    nodeMaterial.dispose();
    nodes.length = 0;
    renderer.dispose();
  });
}

async function setupLayerCanvas(onSelect) {
  const canvas = document.querySelector("[data-layer-canvas]");
  const host = canvas?.parentElement;
  if (!canvas || !host || saveData || reducedMotion.matches) return () => {};
  const {
    BoxGeometry,
    Group,
    Mesh,
    MeshBasicMaterial,
    PerspectiveCamera,
    Raycaster,
    Scene,
    Vector2,
    WebGLRenderer,
  } = await import("./three-runtime.js");

  let renderer;
  try {
    renderer = new WebGLRenderer({
      canvas,
      alpha: true,
      antialias: devicePixelRatio <= 1.5,
      powerPreference: "high-performance",
    });
  } catch {
    return () => {};
  }

  const scene = new Scene();
  const camera = new PerspectiveCamera(34, 1, 0.1, 100);
  camera.position.set(0, 0.2, 8);
  const group = new Group();
  group.rotation.set(-0.26, -0.5, 0.08);
  scene.add(group);
  const geometry = new BoxGeometry(3.8, 2.3, 0.12);
  const colors = [0xf4f7f8, 0x59636d, 0x2855ff, 0x65d9ff];
  const layers = colors.map((color, index) => {
    const material = new MeshBasicMaterial({
      color,
      transparent: true,
      opacity: index === 3 ? 0.5 : 0.72,
    });
    const mesh = new Mesh(geometry, material);
    mesh.position.set(index * 0.42 - 0.6, index * -0.26 + 0.4, index * -0.74);
    mesh.userData.index = index;
    group.add(mesh);
    return { mesh, material };
  });
  const raycaster = new Raycaster();
  const pointer = new Vector2();
  let selected = 0;
  let frame = 0;
  let running = true;

  const resize = () => {
    const bounds = host.getBoundingClientRect();
    renderer.setPixelRatio(Math.min(devicePixelRatio, 1.5));
    renderer.setSize(Math.max(1, bounds.width), Math.max(1, bounds.height), false);
    camera.aspect = bounds.width / Math.max(1, bounds.height);
    camera.updateProjectionMatrix();
  };
  const select = (index) => {
    selected = index;
    layers.forEach(({ mesh, material }, itemIndex) => {
      material.opacity = itemIndex === selected ? 1 : itemIndex === 3 ? 0.34 : 0.52;
      mesh.scale.setScalar(itemIndex === selected ? 1.04 : 1);
    });
    onSelect?.(selected);
  };
  const onPointer = (event) => {
    const bounds = canvas.getBoundingClientRect();
    pointer.x = ((event.clientX - bounds.left) / bounds.width) * 2 - 1;
    pointer.y = -((event.clientY - bounds.top) / bounds.height) * 2 + 1;
    raycaster.setFromCamera(pointer, camera);
    const hit = raycaster.intersectObjects(layers.map(({ mesh }) => mesh))[0];
    canvas.style.cursor = hit ? "pointer" : "default";
    if (event.type === "click" && hit) select(hit.object.userData.index);
  };
  const render = (time) => {
    if (!running) return;
    group.rotation.y = -0.5 + Math.sin(time * 0.00032) * 0.06;
    renderer.render(scene, camera);
    frame = requestAnimationFrame(render);
  };
  const onVisibility = () => {
    running = !document.hidden;
    cancelAnimationFrame(frame);
    if (running) frame = requestAnimationFrame(render);
  };
  resize();
  select(0);
  addEventListener("resize", resize, { passive: true });
  canvas.addEventListener("pointermove", onPointer);
  canvas.addEventListener("click", onPointer);
  document.addEventListener("visibilitychange", onVisibility);
  frame = requestAnimationFrame(render);

  return () => {
    running = false;
    cancelAnimationFrame(frame);
    removeEventListener("resize", resize);
    canvas.removeEventListener("pointermove", onPointer);
    canvas.removeEventListener("click", onPointer);
    document.removeEventListener("visibilitychange", onVisibility);
    geometry.dispose();
    layers.forEach(({ material }) => material.dispose());
    renderer.dispose();
  };
}

function setupMotionPreference() {
  const onChange = () => {
    if (reducedMotion.matches) {
      ScrollTrigger.getAll().forEach((trigger) => trigger.kill());
      document.documentElement.classList.add("reduced-motion");
    } else {
      document.documentElement.classList.remove("reduced-motion");
      ScrollTrigger.refresh();
    }
  };
  onChange();
  reducedMotion.addEventListener?.("change", onChange);
  addDisposable(() => reducedMotion.removeEventListener?.("change", onChange));
}

async function setupLaboratory() {
  const roots = [...document.querySelectorAll("[data-theatre]")];
  if (!roots.length) return;
  try {
    const document = await fetchJson("/assets/laboratory.json");
    for (const root of roots) {
      const controls = [...root.querySelectorAll("[data-lab-control]")];
      const preview = root.querySelector("[data-lab-preview]");
      const input = root.querySelector("[data-lab-input]");
      const css = root.querySelector("[data-lab-css]");
      const proofValues = [...root.querySelectorAll(".proof-panel dd")];
      const tabs = [...root.querySelectorAll("[data-output-tab]")];
      const panels = [...root.querySelectorAll("[data-lab-panel]")];
      const viewportButtons = [...root.querySelectorAll("[data-viewport]")];
      const stage = root.querySelector(".theatre-stage");
      const defaults = {
        layout: "stack",
        gap: "balanced",
        padding: "roomy",
        radius: "soft",
        depth: "raised",
        tone: "paper",
      };
      const state = { ...defaults };
      const findRecipe = () =>
        document.recipes.find((candidate) =>
          Object.entries(state).every(([key, value]) => candidate.selections[key] === value),
        );
      const render = () => {
        const recipe = findRecipe();
        if (!recipe) return;
        // The preview consumes the exact generated class from laboratory.css.
        // No browser-side utility compiler or handwritten style translation exists.
        preview.className = `preview-card ${recipe.className}`;
        preview.dataset.tone = state.tone;
        input.textContent = `pc!("${recipe.input}")`;
        css.textContent = `${recipe.css}\n\n/* bound corpus */\n${document.cssBytes} bytes\nsha256:${document.cssSha256}`;
        const values = [
          recipe.className,
          recipe.styleId,
          document.cssSha256,
        ];
        proofValues.forEach((node, index) => {
          node.textContent = values[index] ?? "—";
        });
        controls.forEach((control) => {
          const dimension = control.dataset.labControl;
          for (const button of control.querySelectorAll("[data-lab-option]")) {
            const active = button.dataset.labOption === state[dimension];
            button.classList.toggle("is-selected", active);
            button.setAttribute("aria-pressed", String(active));
          }
        });
      };
      controls.forEach((control) => {
        const dimension = control.dataset.labControl;
        for (const button of control.querySelectorAll("[data-lab-option]")) {
          const onClick = () => {
            state[dimension] = button.dataset.labOption;
            render();
          };
          button.addEventListener("click", onClick);
          addDisposable(() => button.removeEventListener("click", onClick));
        }
      });
      const reset = root.querySelector("[data-lab-reset]");
      if (reset) {
        const onReset = () => {
          Object.assign(state, defaults);
          render();
        };
        reset.addEventListener("click", onReset);
        addDisposable(() => reset.removeEventListener("click", onReset));
      }
      tabs.forEach((tab) => {
        const selectTab = () => {
          const id = tab.dataset.outputTab;
          tabs.forEach((item) => {
            const selected = item === tab;
            item.setAttribute("aria-selected", String(selected));
            item.tabIndex = selected ? 0 : -1;
          });
          panels.forEach((panel) => {
            panel.hidden = panel.dataset.labPanel !== id;
          });
        };
        const onClick = () => selectTab();
        const onKey = (event) => {
          if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
          event.preventDefault();
          const index = tabs.indexOf(tab);
          const nextIndex =
            event.key === "Home"
              ? 0
              : event.key === "End"
                ? tabs.length - 1
                : (index + (event.key === "ArrowRight" ? 1 : -1) + tabs.length) %
                  tabs.length;
          tabs[nextIndex].focus();
          tabs[nextIndex].click();
        };
        tab.addEventListener("click", onClick);
        tab.addEventListener("keydown", onKey);
        addDisposable(() => {
          tab.removeEventListener("click", onClick);
          tab.removeEventListener("keydown", onKey);
        });
      });
      viewportButtons.forEach((button) => {
        const onClick = () => {
          viewportButtons.forEach((item) =>
            item.setAttribute("aria-pressed", String(item === button)),
          );
          stage.dataset.stageWidth = button.dataset.viewport;
        };
        button.addEventListener("click", onClick);
        addDisposable(() => button.removeEventListener("click", onClick));
      });
      render();
    }
  } catch (error) {
    roots.forEach((root) => {
      const input = root.querySelector("[data-lab-input]");
      if (input) input.textContent = `${tr("Compiler corpus unavailable")}: ${error}`;
    });
  }
}

async function setupConflicts() {
  const roots = [...document.querySelectorAll("[data-conflict-lab]")];
  if (!roots.length) return;
  try {
    const document = await fetchJson("/assets/laboratory.json");
    const conflicts = new Map(document.conflicts.map((item) => [item.id, item]));
    for (const root of roots) {
      const buttons = [...root.querySelectorAll("[data-conflict]")];
      const status = root.querySelector("[data-conflict-status]");
      const code = root.querySelector("[data-conflict-code]");
      const input = root.querySelector("[data-conflict-input]");
      const message = root.querySelector("[data-conflict-message]");
      const suggestion = root.querySelector("[data-conflict-suggestion]");
      const select = (id) => {
        const conflict = conflicts.get(id);
        if (!conflict) return;
        const diagnostic = conflict.diagnostics[0];
        root.dataset.accepted = String(conflict.accepted);
        status.textContent = conflict.accepted ? tr("ACCEPTED") : tr("REJECTED");
        code.textContent = conflict.accepted
          ? tr("STATE SPACE EXPLICIT")
          : diagnostic?.code ?? tr("ERROR");
        input.textContent = conflict.input;
        message.textContent = conflict.accepted
          ? tr("Each condition owns one value. The responsive state is explicit.")
          : tr(diagnostic?.message ?? "Compiler rejected the style.");
        suggestion.textContent = conflict.accepted
          ? tr("Base padding applies until the md condition becomes true.")
          : tr(diagnostic?.suggestion ?? "Inspect the compiler diagnostic.");
        buttons.forEach((button) =>
          {
            const selected = button.dataset.conflict === id;
            button.setAttribute("aria-selected", String(selected));
            button.tabIndex = selected ? 0 : -1;
          },
        );
      };
      buttons.forEach((button) => {
        const onClick = () => select(button.dataset.conflict);
        const onKey = (event) => {
          if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
          event.preventDefault();
          const index = buttons.indexOf(button);
          const nextIndex =
            event.key === "Home"
              ? 0
              : event.key === "End"
                ? buttons.length - 1
                : (index + (event.key === "ArrowRight" ? 1 : -1) + buttons.length) %
                  buttons.length;
          buttons[nextIndex].focus();
          buttons[nextIndex].click();
        };
        button.addEventListener("click", onClick);
        button.addEventListener("keydown", onKey);
        addDisposable(() => {
          button.removeEventListener("click", onClick);
          button.removeEventListener("keydown", onKey);
        });
      });
      select("padding");
    }
  } catch (error) {
    roots.forEach((root) => {
      const message = root.querySelector("[data-conflict-message]");
      if (message) message.textContent = `${tr("Diagnostic corpus unavailable")}: ${error}`;
    });
  }
}

async function setupExplanations() {
  const root = document.querySelector("[data-explanation]");
  if (!root) return;
  const buttons = [...root.querySelectorAll("[data-explanation-id]")];
  const source = root.querySelector("[data-explanation-source]");
  const css = root.querySelector("[data-explanation-css]");
  const utilityList = root.querySelector("[data-explanation-utilities]");
  try {
    const document = await fetchJson("/assets/laboratory.json");
    const entries = new Map(document.explanations.map((entry) => [entry.id, entry]));
    let currentUtilities = [];
    const highlight = (index) => {
      [...utilityList.children].forEach((item, itemIndex) => {
        item.style.opacity = itemIndex === index ? "1" : "0.42";
        item.style.transform = itemIndex === index ? "translateY(-4px)" : "none";
      });
    };
    const disposeCanvas = await setupLayerCanvas((index) => {
      if (currentUtilities.length) highlight(index % currentUtilities.length);
    });
    addDisposable(disposeCanvas);
    const select = (id) => {
      const entry = entries.get(id);
      if (!entry) return;
      const explanation = entry.explanation;
      currentUtilities = explanation.utilities ?? [];
      source.textContent = `pc!("${entry.input}")\n\nstyle ${explanation.styleId}\nclass ${explanation.className}`;
      css.textContent = explanation.css;
      utilityList.innerHTML = currentUtilities
        .map(
          (utility) =>
            `<li><strong>${escapeHtml(utility.source)}</strong><span>${escapeHtml(
              tr(utility.summary),
            )}</span><span>${escapeHtml(utility.pattern)} · bytes ${utility.byteStart}–${utility.byteEnd}</span></li>`,
        )
        .join("");
      buttons.forEach((button) =>
        button.setAttribute("aria-selected", String(button.dataset.explanationId === id)),
      );
      highlight(0);
    };
    buttons.forEach((button) => {
      const onClick = () => select(button.dataset.explanationId);
      button.addEventListener("click", onClick);
      addDisposable(() => button.removeEventListener("click", onClick));
    });
    select("composed-effect");
  } catch (error) {
    source.textContent = `${tr("Explanation corpus unavailable")}: ${error}`;
  }
}

async function setupCatalog() {
  const roots = [...document.querySelectorAll("[data-catalog-explorer]")];
  if (!roots.length) return;
  try {
    const document = await fetchJson("/assets/catalog.json");
    for (const root of roots) {
      const search = root.querySelector("[data-catalog-search]");
      const filters = [...root.querySelectorAll("[data-catalog-filter]")];
      const results = root.querySelector("[data-catalog-results]");
      let activeFilter = "all";
      const utilities = document.utilities;
      results.innerHTML = utilities
        .map((utility) => {
          const capabilities = Object.entries(utility.capabilities ?? {})
            .filter(([, enabled]) => enabled)
            .map(([name]) => `<span>${escapeHtml(name)}</span>`)
            .join("");
          return `<article class="utility-card" data-utility-form="${escapeHtml(
            utility.form,
          )}" data-utility-search="${escapeHtml(
             `${utility.pattern} ${utility.matchName} ${utility.domain} ${utility.summary} ${tr(utility.summary)} ${utility.example}`.toLowerCase(),
          )}">
            <div class="utility-card-header">
              <h3>${escapeHtml(utility.pattern)}</h3>
              <span class="utility-form">${escapeHtml(utility.form)}</span>
            </div>
            <p>${escapeHtml(tr(utility.summary))}</p>
            <code class="utility-example">${escapeHtml(utility.example)}</code>
            <div class="utility-meta"><span>${escapeHtml(utility.domain)}</span>${capabilities}</div>
          </article>`;
        })
        .join("");
      const cards = [...results.querySelectorAll(".utility-card")];
      const apply = () => {
        const query = search.value.trim().toLocaleLowerCase();
        cards.forEach((card) => {
          const matchesQuery = !query || card.dataset.utilitySearch.includes(query);
          const matchesFilter =
            activeFilter === "all" || card.dataset.utilityForm === activeFilter;
          card.hidden = !(matchesQuery && matchesFilter);
        });
      };
      search.addEventListener("input", apply);
      addDisposable(() => search.removeEventListener("input", apply));
      filters.forEach((filter) => {
        const onClick = () => {
          activeFilter = filter.dataset.catalogFilter;
          filters.forEach((item) =>
            item.setAttribute("aria-pressed", String(item === filter)),
          );
          apply();
        };
        filter.addEventListener("click", onClick);
        addDisposable(() => filter.removeEventListener("click", onClick));
      });
    }
  } catch (error) {
    roots.forEach((root) => {
      root.querySelector("[data-catalog-results]").innerHTML =
        `<p class="catalog-loading">${escapeHtml(tr("Catalog unavailable"))}: ${escapeHtml(error)}</p>`;
    });
  }
}

async function setupHeroProof() {
  const cycle = document.querySelector("[data-hero-cycle]");
  const output = document.querySelector("[data-hero-recipe]");
  if (!cycle || !output) return;
  try {
    const document = await fetchJson("/assets/laboratory.json");
    const entries = document.explanations;
    let index = 0;
    const render = () => {
      const entry = entries[index % entries.length];
      output.textContent = `"${entry.input}"`;
      cycle.textContent = `${tr("Cycle proof")} / ${tr(entry.label)}`;
    };
    const onClick = () => {
      index += 1;
      render();
    };
    cycle.addEventListener("click", onClick);
    addDisposable(() => cycle.removeEventListener("click", onClick));
    render();
  } catch {
    cycle.disabled = true;
  }
}

async function setupExamples() {
  const root = document.querySelector("[data-example-lab]");
  const selectors = [...document.querySelectorAll("[data-example-select]")];
  const panels = [...document.querySelectorAll("[data-example-panel]")];
  if (!root || !selectors.length || !panels.length) return;

  let active = "commerce";
  let variant = "carbon";
  let view = "front";
  let opsPulse = 0;
  let corpus;
  try {
    corpus = await fetchJson("/assets/laboratory.json");
  } catch (error) {
    root.querySelector("[data-example-css]").textContent = `${tr("Corpus unavailable")}: ${error}`;
    return;
  }

  const stateRecipes = {
    commerce: {
      default: {
        layout: "split",
        gap: "open",
        padding: "editorial",
        radius: "soft",
        depth: "raised",
        tone: "paper",
      },
      added: {
        layout: "split",
        gap: "balanced",
        padding: "roomy",
        radius: "soft",
        depth: "raised",
        tone: "compiled",
      },
    },
    operations: {
      default: {
        layout: "split",
        gap: "tight",
        padding: "compact",
        radius: "precise",
        depth: "quiet",
        tone: "compiled",
      },
      pulse: {
        layout: "split",
        gap: "balanced",
        padding: "compact",
        radius: "precise",
        depth: "raised",
        tone: "compiled",
      },
    },
    editorial: {
      default: {
        layout: "split",
        gap: "open",
        padding: "editorial",
        radius: "precise",
        depth: "quiet",
        tone: "paper",
      },
      open: {
        layout: "stack",
        gap: "balanced",
        padding: "editorial",
        radius: "precise",
        depth: "raised",
        tone: "paper",
      },
    },
  };

  const inspector = {
    state: root.querySelector("[data-example-state]"),
    recipe: root.querySelector("[data-example-recipe]"),
    css: root.querySelector("[data-example-css]"),
    values: [...root.querySelectorAll(".example-inspector dl dd")],
  };
  const recipeFor = (selection) =>
    corpus.recipes.find((recipe) =>
      Object.entries(selection).every(([key, value]) => recipe.selections[key] === value),
    );
  const currentState = () => {
    if (active === "commerce") {
      return root.querySelector("[data-commerce-add]")?.dataset.added === "true"
        ? "added"
        : "default";
    }
    if (active === "operations") return opsPulse % 2 ? "pulse" : "default";
    return root.querySelector("[data-editorial-expand]")?.getAttribute("aria-expanded") === "true"
      ? "open"
      : "default";
  };
  const updateInspector = () => {
    const state = currentState();
    const recipe = recipeFor(stateRecipes[active][state]);
    if (!recipe) return;
    inspector.state.textContent = `${active} / ${state}`;
    inspector.recipe.textContent = `pc!("${recipe.input}")`;
    inspector.css.textContent = recipe.css;
    [recipe.className, recipe.styleId, corpus.cssSha256.slice(0, 16)].forEach(
      (value, index) => {
        if (inspector.values[index]) inspector.values[index].textContent = value;
      },
    );
    root.style.setProperty("--example-signal", `#${corpus.cssSha256.slice(0, 6)}`);
  };
  const show = (id) => {
    active = id;
    selectors.forEach((button) =>
      button.setAttribute("aria-pressed", String(button.dataset.exampleSelect === id)),
    );
    panels.forEach((panel) => {
      const selected = panel.dataset.examplePanel === id;
      panel.hidden = !selected;
      panel.classList.toggle("is-active", selected);
    });
    updateInspector();
  };
  selectors.forEach((button) => {
    const onClick = () => show(button.dataset.exampleSelect);
    button.addEventListener("click", onClick);
    addDisposable(() => button.removeEventListener("click", onClick));
  });

  root.querySelectorAll("[data-commerce-variant]").forEach((button) => {
    const onClick = () => {
      variant = button.dataset.commerceVariant;
      root.querySelector(".example-commerce").dataset.variant = variant;
      root.querySelectorAll("[data-commerce-variant]").forEach((item) =>
        item.setAttribute("aria-pressed", String(item === button)),
      );
      updateInspector();
    };
    button.addEventListener("click", onClick);
    addDisposable(() => button.removeEventListener("click", onClick));
  });
  root.querySelectorAll("[data-commerce-view]").forEach((button) => {
    const onClick = () => {
      view = button.dataset.commerceView;
      root.querySelector(".example-commerce").dataset.view = view;
      root.querySelectorAll("[data-commerce-view]").forEach((item) =>
        item.setAttribute("aria-pressed", String(item === button)),
      );
      updateInspector();
    };
    button.addEventListener("click", onClick);
    addDisposable(() => button.removeEventListener("click", onClick));
  });
  const add = root.querySelector("[data-commerce-add]");
  if (add) {
    const onClick = () => {
      const added = add.dataset.added !== "true";
      add.dataset.added = String(added);
      add.textContent = added ? tr("Folio added ✓") : tr("Add folio");
      root.querySelector("[data-commerce-status]").textContent = added
        ? `${variant} / ${view} ${tr("secured in the order.")}`
        : tr("Ready to compile the order.");
      updateInspector();
    };
    add.addEventListener("click", onClick);
    addDisposable(() => add.removeEventListener("click", onClick));
  }
  const pulse = root.querySelector("[data-ops-pulse]");
  if (pulse) {
    const onClick = () => {
      opsPulse += 1;
      root.querySelector(".example-operations").classList.toggle("is-replaying");
      pulse.textContent = opsPulse % 2 ? tr("Evidence replaying…") : tr("Replay evidence");
      updateInspector();
    };
    pulse.addEventListener("click", onClick);
    addDisposable(() => pulse.removeEventListener("click", onClick));
  }
  const opsViews = [...root.querySelectorAll("[data-ops-view]")];
  const opsCopy = {
    overview: {
      headings: ["Build authority", "SEVEN-DAY ARTIFACT TRACE", "LATEST EVIDENCE"],
      metrics: [
        ["CSS", "64.8", "KiB corpus"],
        ["Styles", "144", "reachable"],
        ["Drift", "0", "bytes"],
        ["Gate", "PASS", "local"],
      ],
      events: [
        ["14:32:08", "manifest", "hash matched"],
        ["14:32:06", "browser", "route replayed"],
        ["14:31:59", "policy", "0 findings"],
      ],
    },
    builds: {
      headings: ["Build topology", "REACHABLE OUTPUT TRACE", "LATEST BUILDS"],
      metrics: [
        ["Routes", "39", "authored"],
        ["Files", "62", "receipt"],
        ["Drift", "0", "bytes"],
        ["Mode", "SSG", "PliegoRS"],
      ],
      events: [
        ["14:32:10", "routes", "39 emitted"],
        ["14:32:09", "assets", "ledger bound"],
        ["14:32:08", "sitemap", "graph matched"],
      ],
    },
    evidence: {
      headings: ["Evidence ledger", "HASH-BOUND RECEIPT TRACE", "LATEST RECEIPTS"],
      metrics: [
        ["Recipes", "144", "compiled"],
        ["Explain", "3", "lineages"],
        ["Conflict", "3", "scenarios"],
        ["Corpus", "BOUND", "sha256"],
      ],
      events: [
        ["14:32:08", "style ID", "128-bit bound"],
        ["14:32:07", "selector", "CSS verified"],
        ["14:32:06", "receipt", "written last"],
      ],
    },
    policies: {
      headings: ["Policy control", "BUDGET AND COMPATIBILITY TRACE", "LATEST POLICIES"],
      metrics: [
        ["Audit", "0", "advisories"],
        ["Media", "PASS", "frozen"],
        ["Pruning", "PASS", "frozen"],
        ["Release", "PREVIEW", "public"],
      ],
      events: [
        ["14:31:59", "licenses", "allowlist passed"],
        ["14:31:58", "bans", "duplicates clear"],
        ["14:31:57", "release", "not published"],
      ],
    },
  };
  opsViews.forEach((button) => {
    const onClick = () => {
      const id = button.dataset.opsView;
      const copy = opsCopy[id];
      if (!copy) return;
      opsViews.forEach((item) => {
        const selected = item === button;
        item.classList.toggle("is-active", selected);
        item.setAttribute("aria-pressed", String(selected));
      });
      root.querySelector("[data-ops-title]").textContent = tr(copy.headings[0]);
      root.querySelector("[data-ops-trace-label]").textContent = tr(copy.headings[1]);
      root.querySelector("[data-ops-feed-label]").textContent = tr(copy.headings[2]);
      [...root.querySelectorAll(".ops-metrics > div")].forEach((metric, index) => {
        const values = copy.metrics[index];
        if (!values) return;
        metric.querySelector("span").textContent = tr(values[0]);
        metric.querySelector("strong").textContent = values[1];
        metric.querySelector("small").textContent = tr(values[2]);
      });
      [...root.querySelectorAll(".ops-feed > div")].forEach((event, index) => {
        const values = copy.events[index];
        if (!values) return;
        event.querySelector("time").textContent = values[0];
        event.querySelector("strong").textContent = tr(values[1]);
        event.querySelector("span").textContent = tr(values[2]);
      });
      root.querySelector("[data-ops-announcer]").textContent = `${button.textContent} ${tr("selected.")}`;
      root.querySelector(".example-operations").dataset.opsView = id;
    };
    button.addEventListener("click", onClick);
    addDisposable(() => button.removeEventListener("click", onClick));
  });
  const expand = root.querySelector("[data-editorial-expand]");
  if (expand) {
    const onClick = () => {
      const open = expand.getAttribute("aria-expanded") !== "true";
      expand.setAttribute("aria-expanded", String(open));
      expand.textContent = open ? tr("Close the field note") : tr("Open the field note");
      root.querySelector("[data-editorial-excerpt]").hidden = !open;
      updateInspector();
    };
    expand.addEventListener("click", onClick);
    addDisposable(() => expand.removeEventListener("click", onClick));
  }

  root.querySelector(".example-commerce").dataset.variant = variant;
  root.querySelector(".example-commerce").dataset.view = view;
  show(active);
}

function setupDocs() {
  const search = document.querySelector("[data-doc-search]");
  const items = [...document.querySelectorAll("[data-doc-search-item]")];
  if (search && items.length) {
    const onInput = () => {
      const query = search.value.trim().toLocaleLowerCase();
      items.forEach((item) => {
        item.hidden = query !== "" && !item.dataset.searchText.toLocaleLowerCase().includes(query);
      });
      document.querySelectorAll("[data-doc-search-group]").forEach((group) => {
        group.hidden = ![...group.querySelectorAll("[data-doc-search-item]")].some(
          (item) => !item.hidden,
        );
      });
    };
    search.addEventListener("input", onInput);
    addDisposable(() => search.removeEventListener("input", onInput));
  }

  for (const button of document.querySelectorAll("[data-copy]")) {
    const onClick = async () => {
      const code = button.parentElement?.querySelector("code")?.textContent ?? "";
      try {
        await navigator.clipboard.writeText(code);
        button.textContent = tr("COPIED");
        setTimeout(() => (button.textContent = tr("COPY")), 1200);
      } catch {
        button.textContent = tr("SELECT");
      }
    };
    button.addEventListener("click", onClick);
    addDisposable(() => button.removeEventListener("click", onClick));
  }
}

async function setupCommandPalette() {
  const dialog = document.querySelector("[data-command-palette]");
  const input = dialog?.querySelector("[data-command-search]");
  const results = dialog?.querySelector("[data-command-results]");
  const openers = [...document.querySelectorAll("[data-command-open]")];
  const closer = dialog?.querySelector("[data-command-close]");
  if (!dialog || !input || !results) return;

  const staticItems = [
    ["Product", "Home", "CSS you can prove.", "/"],
    ["Product", "Cascade Laboratory", "Real compiler-backed interactions.", "/playground/"],
    ["Product", "Interactive examples", "Commerce, operations, and editorial product surfaces.", "/examples/"],
    ["Docs", "Getting started", "Audit and compile your first surface.", "/docs/getting-started/"],
    ["Docs", "Typed styles", "pc!, pcx!, identity, and lineage.", "/docs/typed-styles/"],
    ["Docs", "Audit and transform", "Standard CSS analysis and policy.", "/docs/audit-and-transform/"],
    ["Docs", "Evidence", "Manifests, receipts, and authority.", "/docs/evidence/"],
    ["Docs", "Migration", "Inventory, plans, apply, and rollback.", "/docs/migration/"],
    ["Docs", "PliegoRS", "Native integration contract.", "/docs/integrations/pliegors/"],
    ["Reference", "Utility catalog", "Generated utility surface.", "/docs/utilities/"],
    ["Evidence", "Benchmarks", "Measured, inherited, pending, uncertain.", "/benchmarks/"],
    ["Project", "Brand", "Identity, assets, and usage.", "/brand/"],
    ["Project", "Security", "Security policy and reporting.", "/security/"],
    ["Project", "Accessibility", "Keyboard, language, focus, and reduced motion.", "/accessibility/"],
    ["Legal", "Legal register", "Current terms, privacy, cookies, and acceptable use.", "/legal/"],
    ["Legal", "Terms", "Current website and public-preview terms.", "/legal/terms/"],
    ["Legal", "Privacy", "Browser data, correspondence, and infrastructure.", "/legal/privacy/"],
    ["Legal", "Cookies", "Current cookie and browser-storage posture.", "/legal/cookies/"],
    ["Legal", "Acceptable use", "Security, artifacts, identity, and reports.", "/legal/acceptable-use/"],
  ].map(([group, title, summary, href]) => ({
    group: tr(group),
    title: tr(title),
    summary: tr(summary),
    href: localizedHref(href),
  }));
  staticItems.push(
    ...[
      ["Start", "Installation", "CLI-only and typed Rust adoption.", "/docs/installation/"],
      ["Start", "First audit", "Read CSS before changing it.", "/docs/first-audit/"],
      ["Start", "Editor setup", "LSP diagnostics and completion.", "/docs/editor-setup/"],
      ["Core", "Utility syntax", "Typed values and visible edges.", "/docs/syntax/"],
      ["Core", "Composition", "Conflict-free semantic composition.", "/docs/composition/"],
      ["Core", "Variants", "Responsive, state, attribute, and layer conditions.", "/docs/variants/"],
      ["Core", "Themes and tokens", "TOML and DTCG Resolver inputs.", "/docs/themes/"],
      ["Standard CSS", "Policies", "Compatibility, accessibility, and budgets.", "/docs/standard-css/policies/"],
      ["Standard CSS", "Cascade inspection", "Explain the winning declaration.", "/docs/cascade/"],
      ["Evidence", "Style manifests", "Semantic and physical lineage.", "/docs/manifests/"],
      ["Evidence", "Control artifacts", "Receipt-last publication.", "/docs/control-artifacts/"],
      ["Evidence", "Critical CSS", "Browser evidence with bounded authority.", "/docs/critical-css/"],
      ["Migration", "Tailwind", "Inventory and hash-bound plans.", "/docs/migration/tailwind/"],
      ["Migration", "Sass", "Separate syntax conversion and adoption.", "/docs/migration/sass/"],
      ["Migration", "CSS Modules", "Preserve component ownership.", "/docs/migration/css-modules/"],
      ["Tooling", "CLI reference", "Explicit modes and strict arguments.", "/docs/tooling/cli/"],
      ["Tooling", "Language server", "Bounded editor feedback.", "/docs/tooling/lsp/"],
      ["Tooling", "Watch mode", "Keep the last complete truth.", "/docs/tooling/watch/"],
      ["Tooling", "Repair agent", "Hash-bound authorization and staging.", "/docs/tooling/repair/"],
      ["Integrations", "Plain HTML", "Framework-free audit and transform.", "/docs/integrations/plain-html/"],
      ["Integrations", "Generic build tools", "File-first build integration.", "/docs/integrations/build-tools/"],
      ["Reference", "Configuration", "Inputs that participate in identity.", "/docs/reference/configuration/"],
      ["Reference", "Diagnostics", "Stable codes and precise ranges.", "/docs/reference/diagnostics/"],
    ].map(([group, title, summary, href]) => ({
      group: tr(group),
      title: tr(title),
      summary: tr(summary),
      href: localizedHref(href),
    })),
  );
  let items = [...staticItems];
  let activeIndex = 0;

  try {
    const catalog = await fetchJson("/assets/catalog.json");
    items.push(
      ...catalog.utilities.map((utility) => ({
        group: tr("Utilities"),
        title: utility.pattern,
        summary: tr(utility.summary),
        href: localizedHref(`/docs/utilities/?q=${encodeURIComponent(utility.pattern)}`),
        code: utility.form,
      })),
    );
  } catch {
    // Static navigation remains useful without the generated catalog.
  }

  const filtered = () => {
    const query = input.value.trim().toLocaleLowerCase();
    return items
      .filter((item) =>
        `${item.group} ${item.title} ${item.summary}`.toLocaleLowerCase().includes(query),
      )
      .slice(0, 16);
  };
  const render = () => {
    const matches = filtered();
    activeIndex = Math.max(0, Math.min(activeIndex, Math.max(0, matches.length - 1)));
    if (!matches.length) {
      results.innerHTML = `<p class="command-empty">${escapeHtml(tr("No exact surface found."))}</p>`;
      return;
    }
    let lastGroup = "";
    results.innerHTML = matches
      .map((item, index) => {
        const group =
          item.group === lastGroup
            ? ""
            : `<p class="command-section-label">${escapeHtml(item.group)}</p>`;
        lastGroup = item.group;
        return `${group}<a class="command-result ${index === activeIndex ? "is-active" : ""}"
          href="${escapeHtml(item.href)}" data-command-index="${index}">
          <span><strong>${escapeHtml(item.title)}</strong><small>${escapeHtml(
            item.summary,
          )}</small></span>
          <code>${escapeHtml(item.code ?? "↗")}</code>
        </a>`;
      })
      .join("");
  };
  const open = () => {
    if (!dialog.open) dialog.showModal();
    input.value = "";
    activeIndex = 0;
    render();
    requestAnimationFrame(() => input.focus());
  };
  const close = () => dialog.open && dialog.close();
  const onKey = (event) => {
    if ((event.metaKey || event.ctrlKey) && event.key.toLocaleLowerCase() === "k") {
      event.preventDefault();
      dialog.open ? close() : open();
      return;
    }
    if (!dialog.open) return;
    const matches = filtered();
    if (event.key === "ArrowDown") {
      event.preventDefault();
      activeIndex = Math.min(activeIndex + 1, matches.length - 1);
      render();
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      activeIndex = Math.max(activeIndex - 1, 0);
      render();
    } else if (event.key === "Enter" && matches[activeIndex]) {
      event.preventDefault();
      location.href = matches[activeIndex].href;
    }
  };
  const onInput = () => {
    activeIndex = 0;
    render();
  };
  openers.forEach((opener) => opener.addEventListener("click", open));
  closer?.addEventListener("click", close);
  input.addEventListener("input", onInput);
  addEventListener("keydown", onKey);
  addDisposable(() => {
    openers.forEach((opener) => opener.removeEventListener("click", open));
    closer?.removeEventListener("click", close);
    input.removeEventListener("input", onInput);
    removeEventListener("keydown", onKey);
  });
}

function hydrateCatalogQuery() {
  const query = new URLSearchParams(location.search).get("q");
  const search = document.querySelector("[data-catalog-search]");
  if (query && search) {
    search.value = query;
    search.dispatchEvent(new Event("input"));
  }
}

async function boot() {
  document.documentElement.classList.add("js");
  await setupLocale();
  setupMotionPreference();
  setupScroll();
  const heroCanvasReady = setupHeroCanvas();
  setupDocs();
  // Start finite entrance motion immediately. Interactive hydration may fetch
  // evidence, but essential content must never wait on that network boundary.
  setupMotion();
  await Promise.all([
    heroCanvasReady,
    setupLaboratory(),
    setupConflicts(),
    setupExplanations(),
    setupCatalog(),
    setupHeroProof(),
    setupExamples(),
    setupCommandPalette(),
  ]);
  hydrateCatalogQuery();
  ScrollTrigger.refresh();
}

boot().catch((error) => {
  console.error("PliegoCSS site boot failed", error);
  document.documentElement.classList.add("client-error");
});

addEventListener(
  "pagehide",
  () => {
    while (disposers.length) disposers.pop()?.();
    ScrollTrigger.getAll().forEach((trigger) => trigger.kill());
  },
  { once: true },
);
