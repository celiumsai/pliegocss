export function Card({ active }) {
  return <article className="rounded p-4 shadow" data-state={active} class={active ? "safe" : "hidden"} />;
}
