// Whether the user can currently see and is using the window. Continuous
// animations pause otherwise, so a backgrounded Aether costs ~0 CPU.
// A plain module (not React state) so render loops can read it per frame.

type Listener = (active: boolean) => void;

const listeners = new Set<Listener>();
const compute = () => document.visibilityState === "visible" && document.hasFocus();
let active = compute();

function update() {
  const next = compute();
  if (next === active) return;
  active = next;
  listeners.forEach((l) => l(active));
}

window.addEventListener("focus", update);
window.addEventListener("blur", update);
document.addEventListener("visibilitychange", update);

export const isWindowActive = () => active;

export function onWindowActiveChange(listener: Listener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
