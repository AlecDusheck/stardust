// CPAL owns the context; resume it inside a browser-authorized gesture.
export function enableAudio(window) {
  if (!window.AudioContext) return;
  window.AudioContext = class extends window.AudioContext {
    constructor(options) {
      super(options);
      const resume = () => {
        if (this.state === "suspended" || this.state === "interrupted") {
          this.resume().catch(error => console.warn("Could not resume audio", error));
        }
      };
      for (const event of ["pointerdown", "pointerup", "keydown"]) {
        window.addEventListener(event, resume, { capture: true });
      }
    }
  };
}

export function bindControls(controls, canvas) {
  const document = controls.ownerDocument;
  const window = document.defaultView;
  const held = new Map();

  function key(button, down) {
    const code = button.dataset.code;
    button.classList.toggle("pressed", down);
    button.setAttribute("aria-pressed", String(down));
    canvas.dispatchEvent(new window.KeyboardEvent(down ? "keydown" : "keyup", {
      code, key: code === "Space" ? " " : code === "KeyR" ? "r" : code,
      bubbles: true,
    }));
  }

  function change(id, button) {
    const previous = held.get(id);
    if (previous === button) return;
    held.delete(id);
    if (previous && ![...held.values()].includes(previous)) key(previous, false);
    if (button && ![...held.values()].includes(button)) key(button, true);
    held.set(id, button);
  }

  function release(event) {
    if (!held.has(event.pointerId)) return;
    change(event.pointerId, null);
    held.delete(event.pointerId);
  }

  controls.addEventListener("pointerdown", event => {
    const button = event.target.closest("button[data-code]");
    if (!button) return;
    event.preventDefault();
    canvas.focus({ preventScroll: true });
    controls.setPointerCapture(event.pointerId);
    change(event.pointerId, button);
  });
  controls.addEventListener("pointermove", event => {
    if (!held.has(event.pointerId)) return;
    const button = document.elementFromPoint(event.clientX, event.clientY)
      ?.closest("button[data-code]");
    change(event.pointerId, button && controls.contains(button) ? button : null);
  });
  for (const event of ["pointerup", "pointercancel", "lostpointercapture"]) {
    controls.addEventListener(event, release);
  }
  function releaseAll() {
    for (const pointerId of held.keys()) release({ pointerId });
  }
  window.addEventListener("blur", releaseAll);
  document.addEventListener("visibilitychange", () => {
    if (document.hidden) releaseAll();
  });
}
