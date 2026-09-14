#!/usr/bin/env python3
"""Build Stardust for the web, serve it locally, and open the browser."""

from argparse import ArgumentParser
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import re
import shutil
import subprocess
import webbrowser

from extract import extract


def main():
    parser = ArgumentParser(description=__doc__)
    parser.add_argument("--build-only", action="store_true", help="Build without starting the server")
    args = parser.parse_args()
    npx = shutil.which("npx")
    if npx is None:
        parser.error("Install Node.js 22 or newer to build for the web")
    root = Path(__file__).resolve().parents[1]
    output = root / "target/web"

    if not (root / "assets/levels/campaign.ron").exists():
        extract(root)

    def run(*args):
        subprocess.run(args, cwd=root, check=True)

    run("rustup", "target", "add", "wasm32-unknown-unknown")
    version = re.search(r'name = "wasm-bindgen"\nversion = "([^"]+)"', (root / "Cargo.lock").read_text()).group(1)
    installed = subprocess.check_output(["wasm-bindgen", "--version"], text=True).strip() if shutil.which("wasm-bindgen") else ""
    if installed != f"wasm-bindgen {version}":
        run("cargo", "install", "wasm-bindgen-cli", "--version", version, "--locked")

    run("cargo", "build", "--locked", "--target", "wasm32-unknown-unknown", "--profile", "wasm-release", "--features", "bevy/web")
    run("wasm-bindgen", "--target", "web", "--remove-name-section", "--out-dir", str(output),
        "target/wasm32-unknown-unknown/wasm-release/stardust.wasm")
    wasm = str(output / "stardust_bg.wasm")
    run(npx, "--yes", "--package", "binaryen@132.0.0", "wasm-opt", wasm,
        "-Oz", "--strip-debug", "--strip-producers", "-o", wasm)
    for name in ("index.html", "controls.mjs"):
        shutil.copy2(root / "web" / name, output / name)
    shutil.copytree(root / "assets", output / "assets", dirs_exist_ok=True)

    if args.build_only:
        return

    handler = partial(SimpleHTTPRequestHandler, directory=str(output))
    with ThreadingHTTPServer(("127.0.0.1", 8080), handler) as server:
        print("Stardust: http://127.0.0.1:8080 — Ctrl+C to stop", flush=True)
        webbrowser.open("http://127.0.0.1:8080")
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass


if __name__ == "__main__":
    main()
