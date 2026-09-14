# Stardust

A Rust and [Bevy](https://bevy.org) port of *Stardust 1.1*, James Burton's 1995
Macintosh puzzle game (freeware, THINK Pascal, art in PixelPaint). Fifty levels
of walking, levitating and building stardust blocks to reach the exit portal.

## Layout

| Path | What |
| --- | --- |
| `src/` | The game and level editor (Bevy 0.19) |
| `crates/stardust-core` | Deterministic actions, animation frames and timing, no engine dependencies |
| `crates/stardust-level` | Level model, RON format, ASCII legend, importer for the original data, passwords |
| `crates/stardust-extract` | CLI that pulls art, sounds and levels out of the original archive |
| `crates/macrsrc` | Classic Mac resource fork, `PICT` and `snd ` decoders |
| `crates/stuffit` | StuffIt 5 archive reader with the Arsenic decompressor |
| `archive/` | The original `Stardust_Mac_EN.sit` |
| `vendor/` | Faithful exports of every original resource (generated) |
| `assets/` | Sprite sheets, sounds and levels the game loads (generated) |

## Extracting the original data

```sh
python3 scripts/extract.py
```

The script downloads the [original archive](https://archive.org/download/stardust-mac-en_202609/Stardust_Mac_EN.sit)
into `archive/` if needed, verifies its checksum, then runs the Rust extractor.
It writes original resources to `vendor/` and game-ready art, sounds and all
50 levels to `assets/`. Hero sheets retain their original pixel positions and masks.

## Playing

Generate the assets with the extraction script above before building.

```sh
cargo run
```

| Key | Action |
| --- | --- |
| Arrows or WASD | Walk, up, crouch |
| Space / Return | Magic: build or destroy the block in front |
| Down + Magic | Build or destroy diagonally down (or the green block underfoot) |
| Up + Magic | Levitate on a green block, or destroy the green block above |
| Up in an exit portal | Finish the level |
| R | Reset the level |
| 1 / 2 / 3 | Fast / normal / slow animation |
| Esc | Back to the title |

Title screen: `Space` new game, `P` enter a password, `I` instructions, `S` the
story, `E` level editor.

## Level format

Levels are RON files with one string per row. The legend is mnemonic rather
than the original digits:

```
.  empty        *  star wall     #  gray wall      I  entrance     O  exit
U  warp pocket  R  red wall      +  star           ~  fall wall
<  one-way (only passable moving left)   >  one-way (only passable moving right)
^  elevator     |  tunnel        %  vertical warp   (  companion    )  companion mark
b  blue block   g  green block   /  "  &  pass-through decorations
```

`assets/levels/campaign.ron` lists the level files in play order. Each level
carries its name, optional password and which hero is drawn.

## Level editor

Press `E` on the title screen. Left-click paints the current brush, right-click
erases, the mouse wheel or `[` `]` change the brush, `1`–`9` and Page Up/Down
load campaign levels, `P` test-plays, `N` starts a new level and `Ctrl+S` saves
to `assets/levels/` (native builds only; the browser build logs the RON).

## Web build

Requires Node.js 22 or newer as well as Rust and Python 3.

```sh
python3 scripts/web.py
```

The script extracts missing assets, installs the WebAssembly target and matching
`wasm-bindgen` CLI, optimizes the build with Binaryen, copies assets, and opens
http://127.0.0.1:8080.
Press Ctrl+C to stop the server. The complete site is in `target/web/`.

Touchscreens show a D-pad, Magic, Reset and Menu. Hold a direction and Magic
together for combined actions. Browser audio starts after a tap or keypress.

To build without launching the server and deploy to Cloudflare:

```sh
python3 scripts/web.py --build-only
npx wrangler@4 deploy --config web/wrangler.jsonc
```

The configuration serves the site at `stardust.dusheck.com`. The `dusheck.com`
zone must be active in your Cloudflare account; Wrangler creates the custom domain
and its DNS record. Run `npx wrangler@4 login` once for local deployments.

## Tests and binary fidelity

The regression suite compares 939 recorded scenarios (313 at each speed) with
results from the original executable: position, facing, completion, runtime tiles,
ticks, presentation delays, background restoration, sprite rectangles and sound
requests. It covers input combinations, magic, crumbling, warps, death, respawn,
reunion and input sequences on all 50 levels. Other tests check extracted pixels,
passwords, transitions, audio channels and presentation at 30, 60 and 144 updates
per second.

```sh
python3 scripts/extract.py
cargo test --workspace --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
node --test tests/web_controls.test.mjs
```

Normal tests use checked-in fixtures and require no emulator. The fixture harness
executes original CPU instructions with mocked Macintosh drawing, audio and input
calls and controlled TickCount and Random. Campaign traces are input sequences,
not complete solutions. Audio-device latency is not reproduced; the original
unpaced reunion wipe runs in order at rendering cadence. Internal representations
such as password obfuscation need not match. QuickDraw scan conversion and random
behavior were checked against [Apple's original sources](https://github.com/computerhistory/Historical-Source-Code-Quick-Draw-Repository).

To regenerate fixtures, install `uv` and run:

```sh
cargo run -p stardust-extract --example dump_code -- archive/Stardust_Mac_EN.sit target/binary-audit
uv run scripts/audit_binary.py --write-fixtures
```

The harness verifies CODE 1 SHA-256
`6a1495f0ded01362361b4672011ee76ecc60c1704e34047ef743bbd1b20065ed`.
Disassembly and traces go to `target/binary-audit/`; gameplay and password fixtures
live under their crates' `tests/fixtures/`, and transition fixtures live in
`tests/fixtures/transitions.tsv`.

Useful CODE 1 offsets: input/actions `$4166–47d8`, animations `$232e–393e`,
restore/present/draw `$3952–3ce2`, crumble `$21fa–22ce`, loading `$4b06–4d34`,
exit wipe `$1e96`, reunion wipe `$1f86`, passwords `$1202–18aa`, sound `$314–440`.

## Development helpers

Native builds accept a scripted session, handy for screenshots and reviews:

```sh
STARDUST_KEYS="1:space;3:space;5:right*1.2" STARDUST_SHOTS="6:play.png" cargo run
```

`F12` saves a screenshot at any time.
