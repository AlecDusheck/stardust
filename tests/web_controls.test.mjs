import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtemp, readFile, writeFile, copyFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { bindControls, enableAudio } from "../web/controls.mjs";

function emit(target, type, properties = {}) {
  const event = new Event(type, { cancelable: true });
  for (const [name, value] of Object.entries(properties)) {
    Object.defineProperty(event, name, { value });
  }
  target.dispatchEvent(event);
}

test("audio resumes on mouse, touch release and keyboard gestures, including after interruption", async () => {
  const window = new EventTarget();
  window.AudioContext = class {
    state = "suspended";
    resumes = 0;
    constructor(options) { this.options = options; }
    async resume() { this.resumes++; this.state = "running"; }
  };
  enableAudio(window);
  const context = new window.AudioContext({ sampleRate: 44100 });
  assert.equal(context.options.sampleRate, 44100);
  assert.equal(context.resumes, 0);
  for (const gesture of ["pointerdown", "pointerup", "keydown"]) {
    context.state = "suspended";
    emit(window, gesture);
    assert.equal(context.state, "running");
  }
  emit(window, "keydown");
  assert.equal(context.resumes, 3);
  context.state = "interrupted";
  emit(window, "pointerup");
  assert.equal(context.resumes, 4);
  context.state = "closed";
  emit(window, "keydown");
  assert.equal(context.resumes, 4);
  assert.doesNotThrow(() => enableAudio({}));
});

function setup() {
  const window = new EventTarget();
  window.KeyboardEvent = class extends Event {
    constructor(type, options) { super(type, options); Object.assign(this, { code: options.code, key: options.key }); }
  };
  const document = Object.assign(new EventTarget(), { defaultView: window });
  const controls = Object.assign(new EventTarget(), { ownerDocument: document, setPointerCapture() {} });
  const canvas = Object.assign(new EventTarget(), { focus() {} });
  const events = [];
  for (const type of ["keydown", "keyup"]) {
    canvas.addEventListener(type, event => events.push([event.type, event.code, event.key]));
  }
  const buttons = Object.fromEntries(["ArrowUp", "ArrowRight", "Space", "KeyR", "Escape"].map(code => {
    const button = {
      dataset: { code },
      classList: { toggle(_name, pressed) { button.pressed = pressed; } },
      setAttribute(name, value) { button[name] = value; },
      closest() { return button; },
    };
    return [code, button];
  }));
  controls.contains = button => Object.values(buttons).includes(button);
  bindControls(controls, canvas);
  return {
    window, document, controls, canvas, events, buttons,
    pointer(type, pointerId, code) {
      document.elementFromPoint = () => buttons[code] ?? null;
      emit(controls, type, { pointerId, target: buttons[code] ?? controls });
    },
  };
}

test("the shipped page installs audio recovery before wasm-bindgen captures AudioContext", async () => {
  const directory = await mkdtemp(join(tmpdir(), "stardust-web-"));
  const { window, document, controls, canvas } = setup();
  window.AudioContext = class {
    state = "suspended";
    async resume() { this.state = "running"; }
  };
  document.querySelector = selector => selector === "#controls" ? controls : canvas;
  globalThis.window = window;
  globalThis.document = document;
  try {
    const html = await readFile(new URL("../web/index.html", import.meta.url), "utf8");
    const entry = html.match(/<script type="module">([\s\S]*?)<\/script>/)[1];
    await writeFile(join(directory, "entry.mjs"), entry);
    await writeFile(join(directory, "package.json"), '{"type":"module"}');
    await copyFile(new URL("../web/controls.mjs", import.meta.url), join(directory, "controls.mjs"));
    await writeFile(join(directory, "stardust.js"), `
      const AudioContext = window.AudioContext;
      export default function init() { window.engineAudio = new AudioContext(); }
    `);
    await import(pathToFileURL(join(directory, "entry.mjs")));
    assert.equal(window.engineAudio.state, "suspended");
    emit(window, "pointerup");
    assert.equal(window.engineAudio.state, "running");
  } finally {
    delete globalThis.window;
    delete globalThis.document;
    await rm(directory, { recursive: true, force: true });
  }
});

test("movement and Magic stay independent with two fingers", () => {
  const { pointer, events, buttons } = setup();
  pointer("pointerdown", 1, "ArrowUp");
  pointer("pointerdown", 2, "Space");
  pointer("pointerup", 2);
  assert.equal(buttons.ArrowUp.pressed, true);
  assert.equal(buttons.Space["aria-pressed"], "false");
  pointer("pointerup", 1);
  assert.deepEqual(events, [
    ["keydown", "ArrowUp", "ArrowUp"], ["keydown", "Space", " "],
    ["keyup", "Space", " "], ["keyup", "ArrowUp", "ArrowUp"],
  ]);
});

test("two fingers on one button release the key only when both lift", () => {
  const { pointer, events } = setup();
  pointer("pointerdown", 1, "Space");
  pointer("pointerdown", 2, "Space");
  pointer("pointerup", 1);
  assert.equal(events.length, 1);
  pointer("pointerup", 2);
  assert.deepEqual(events.map(event => event[0]), ["keydown", "keyup"]);
});

test("sliding between buttons or outside the controls releases the previous key", () => {
  const { pointer, events } = setup();
  pointer("pointermove", 9, "Space");
  pointer("pointerdown", 1, "ArrowUp");
  pointer("pointermove", 1, "ArrowRight");
  pointer("pointermove", 1);
  pointer("pointermove", 1, "Space");
  pointer("pointerup", 1);
  assert.deepEqual(events.map(event => event.slice(0, 2)), [
    ["keydown", "ArrowUp"], ["keyup", "ArrowUp"],
    ["keydown", "ArrowRight"], ["keyup", "ArrowRight"],
    ["keydown", "Space"], ["keyup", "Space"],
  ]);
});

for (const cause of ["pointercancel", "lostpointercapture", "blur", "visibilitychange"]) {
  test(`${cause} prevents stuck keys`, () => {
    const { pointer, window, document, events } = setup();
    pointer("pointerdown", 1, "KeyR");
    if (cause === "blur") emit(window, cause);
    else if (cause === "visibilitychange") {
      document.hidden = true;
      emit(document, cause);
    } else pointer(cause, 1);
    pointer("pointerup", 1);
    assert.deepEqual(events, [["keydown", "KeyR", "r"], ["keyup", "KeyR", "r"]]);
  });
}
