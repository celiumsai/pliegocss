import {
  decorateClassNodes,
  extractStaticClassGroups,
  rewriteStaticClassGroups,
} from "./adapter-coexistence-v1.mjs";

function expectFailure(action, fragment) {
  try {
    action();
  } catch (error) {
    if (error.message.includes(fragment)) return;
    throw error;
  }
  throw new Error(`expected failure containing ${JSON.stringify(fragment)}`);
}

const source = '<main class="flex gap-4"><button class="px-4 py-2">Save</button></main>';
const inventory = extractStaticClassGroups(source, "contract-test");
if (
  JSON.stringify(inventory.groups) !== JSON.stringify(["flex gap-4", "px-4 py-2"]) ||
  !decorateClassNodes(source).includes('data-g6-node="g6-node-001"')
) {
  throw new Error("static class extraction or decoration drifted");
}
const manifest = {
  styles: [
    { className: "pc_one", origins: [{ source: "flex gap-4" }] },
    { className: "pc_two", origins: [{ source: "px-4 py-2" }] },
  ],
};
const rewritten = rewriteStaticClassGroups(source, manifest, "contract-test");
if (
  rewritten.replacements !== 2 ||
  rewritten.document !== '<main class="pc_one"><button class="pc_two">Save</button></main>'
) {
  throw new Error("static manifest-authorized rewrite drifted");
}

for (const dynamic of [
  '<div className="flex"></div>',
  "<div class='flex'></div>",
  '<div class = \'flex\'></div>',
  '<div :class="state"></div>',
  '<div v-bind:class="state"></div>',
  '<div class:list="state"></div>',
  '<div class="${state}"></div>',
  '<div class="{{ state }}"></div>',
  '<script>const value = "class=\\"flex\\""</script>',
  '<SCRIPT>const value = "class=\\"flex\\""</SCRIPT>',
]) {
  expectFailure(() => extractStaticClassGroups(dynamic, "dynamic-test"), "unsupported or dynamic");
}
expectFailure(
  () => rewriteStaticClassGroups(source, { styles: manifest.styles.slice(0, 1) }, "missing-test"),
  "manifest has no generated class",
);
expectFailure(
  () =>
    rewriteStaticClassGroups(
      source,
      {
        styles: [
          ...manifest.styles,
          { className: "pc_other", origins: [{ source: "flex gap-4" }] },
        ],
      },
      "ambiguous-test",
    ),
  "multiple generated classes",
);

process.stdout.write("adapter coexistence static protocol contract: pass\n");
