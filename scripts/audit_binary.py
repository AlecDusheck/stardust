# /// script
# requires-python = ">=3.11"
# dependencies = ["unicorn==2.1.4", "capstone==5.0.7"]
# ///
"""Execute Stardust 1.1's gameplay routines; intercept only OS/presentation calls.

Run from the repository root with: uv run scripts/audit_binary.py
The resource dump must already exist (see README.md, Tests and binary fidelity).
This is a CPU-level trace harness, not a complete Macintosh emulator.
"""
from pathlib import Path
import argparse
import hashlib
import json
import re
import struct

from capstone import Cs, CS_ARCH_M68K, CS_MODE_BIG_ENDIAN, CS_MODE_M68K_000
from unicorn import Uc, UC_ARCH_M68K, UC_MODE_BIG_ENDIAN, UC_HOOK_CODE
from unicorn.m68k_const import (
    UC_CPU_M68K_M68000, UC_M68K_REG_A5, UC_M68K_REG_A7,
    UC_M68K_REG_D0, UC_M68K_REG_D5, UC_M68K_REG_D6, UC_M68K_REG_PC,
)

OUT = Path("target/binary-audit")
CODE = OUT / "CODE-1.bin"
CODE_SHA = "6a1495f0ded01362361b4672011ee76ecc60c1704e34047ef743bbd1b20065ed"
BASE, GLOBAL, STACK, STOP = 0x10000, 0x30000, 0x50000, 0x80000
# Port legend -> original runtime codes, from CODE 1 $3dc2 and $4d34.
TYPES = {".": 0, "*": 1, "#": 2, "I": 3, "O": 4, "b": 5, "g": 6,
         "U": 7, "R": 8, "+": 9, "~": 10, "<": 11, ">": 12, "^": 13,
         "|": 14, "%": 15, "(": 16, ")": 0, "/": 9, '"': 9, "&": 9}
KEYS = {"L": 123, "R": 124, "D": 125, "U": 126, "M": 49}


class Original:
    def __init__(self, rows=None, speed=2, hero=0):
        self.u = Uc(UC_ARCH_M68K, UC_MODE_BIG_ENDIAN)
        self.u.ctl_set_cpu_model(UC_CPU_M68K_M68000)
        self.u.mem_map(0, 0x100000)
        self.u.mem_write(BASE, CODE.read_bytes())
        self.u.reg_write(UC_M68K_REG_A5, GLOBAL)
        self.u.reg_write(UC_M68K_REG_A7, STACK)
        self.u.hook_add(UC_HOOK_CODE, self.hook)
        self.now, self.heap = 100, 0x60000
        self.keys, self.log = [], []
        self.w(-0x3c6, 5)
        self.w(-0x3c4, 5)
        self.b(-0x3c1, 1)  # Controlled initial facing, not the game's random spawn.
        self.b(-0x3ca, speed)
        self.b(-0x3bd, hero)
        # Actual loader uses solid top/sides and an open bottom ($4bc2).
        for x in range(18):
            for y in range(14):
                self.tile(x, y, 2 if x in (0, 17) or y == 0 else 0)
        if rows:
            for y, row in enumerate(rows, 1):
                for x, c in enumerate(row, 1):
                    self.tile(x, y, TYPES[c])
                    if c == "I":
                        self.w(-0x3c6, x)
                        self.w(-0x3c4, y)

    def read(self, address, size=2):
        return int.from_bytes(self.u.mem_read(address, size), "big")

    def write(self, address, value, size=2):
        self.u.mem_write(address, int(value % (1 << (size * 8))).to_bytes(size, "big"))

    def w(self, offset, value):
        self.write(GLOBAL + offset, value)

    def b(self, offset, value):
        self.write(GLOBAL + offset, value, 1)

    def tile(self, x, y, value):
        self.b(-0x3bc + x * 14 + y, value)

    def ret(self, argument_bytes):
        sp = self.u.reg_read(UC_M68K_REG_A7)
        pc = self.read(sp, 4)
        self.u.reg_write(UC_M68K_REG_A7, sp + 4 + argument_bytes)
        self.u.reg_write(UC_M68K_REG_PC, pc)

    def rect(self, address):
        return list(struct.unpack(">hhhh", self.u.mem_read(address, 8)))

    def hook(self, u, address, _size, _data):
        offset = address - BASE
        sp = u.reg_read(UC_M68K_REG_A7)
        if address == STOP:
            u.emu_stop()
            return
        if offset == 0x3ae:  # Cursor UI helper; no game-state effects.
            self.ret(0)
            return
        if offset == 0x140:  # Pascal allocation helper, for metadata initialization.
            self.write(self.read(sp + 8, 4), self.heap, 4)
            self.heap += self.read(sp + 4, 4)
            self.ret(8)
            return
        if offset == 0x3952:  # Restore cell pixels; grid is left untouched.
            x, y = self.read(sp + 6), self.read(sp + 4)
            self.log.append(["restore", x - 1, y - 1,
                             self.read(GLOBAL - 0x3bc + x * 14 + y, 1), f"{self.read(sp, 4) - BASE:04x}"])
            self.ret(4)
            return
        if offset in (0x39ba, 0x3ce2):
            self.log.append(["hero" if offset == 0x39ba else "tile",
                             self.rect(self.read(sp + 8, 4)), self.rect(self.read(sp + 4, 4)), self.now, f"{self.read(sp, 4) - BASE:04x}"])
            self.ret(8)
            return
        if offset == 0x3a32:
            self.log.append(["present", self.read(sp + 4), self.now])
            # Execute the original wait, deadline update, and crumble callbacks.
        if offset == 0x3d6:
            self.log.append(["sound", self.read(sp + 4), self.now])
            self.ret(2)
            return
        if offset == 0x3d32:
            self.log.append(["victory_art", self.now])
            self.ret(0)
            return
        if offset == 0x3a44:
            self.now = max(self.now, self.read(GLOBAL - 0x20a, 4))
        if offset == 0x1f00:
            self.now = max(self.now, u.reg_read(UC_M68K_REG_D6))
        if offset == 0x371a:
            self.now = max(self.now, u.reg_read(UC_M68K_REG_D5))
        word = self.read(address)
        pop = 0
        if word == 0xa976:  # GetKeys
            buf = bytearray(16)
            for key in self.keys:
                buf[key // 8] |= 1 << (key % 8)
            u.mem_write(self.read(sp, 4), bytes(buf))
            pop = 4
        elif word == 0xa975:  # TickCount, deterministic clock advanced at wait loops.
            self.write(sp, self.now, 4)
        elif word == 0xa8a7:  # SetRect (Pascal argument order).
            bottom, right, top, left = struct.unpack(">hhhh", u.mem_read(sp, 8))
            u.mem_write(self.read(sp + 8, 4), struct.pack(">hhhh", top, left, bottom, right))
            pop = 12
        elif word == 0xa8a8:  # OffsetRect
            dy, dx = struct.unpack(">hh", u.mem_read(sp, 4))
            ptr = self.read(sp + 4, 4)
            top, left, bottom, right = self.rect(ptr)
            u.mem_write(ptr, struct.pack(">hhhh", top + dy, left + dx, bottom + dy, right + dx))
            pop = 8
        elif word == 0xab1d:  # Only SetGWorld is needed in these routines.
            if u.reg_read(UC_M68K_REG_D0) != 0x80006:
                raise RuntimeError("Unexpected graphics selector")
            pop = 8
        elif word == 0xa8ec:  # CopyBits: retain source/destination rectangles as evidence.
            self.log.append(["copy", self.rect(self.read(sp + 10, 4)),
                             self.rect(self.read(sp + 6, 4)), self.now, f"{offset:04x}"])
            pop = 22
        elif word in (0xa862, 0xa863, 0xa87a, 0xa879):  # Fore/BackColor, Get/SetClip
            pop = 4
        elif word in (0xa8de, 0xa8e6):  # RectRgn, DiffRgn
            pop = 12
        elif word in (0xa8a2, 0xa8a4, 0xa8ba):
            self.log.append([{0xa8a2: 'black', 0xa8a4: 'invert', 0xa8ba: 'oval'}[word], self.rect(self.read(sp, 4)), self.now])
            pop = 4
        elif word == 0xa861:  # Controlled Random result; algorithm itself not emulated.
            self.write(sp, 1234)
        elif word & 0xf000 == 0xa000:
            raise RuntimeError(f"Unhandled trap {word:04x} at CODE 1 ${offset:04x}")
        else:
            return
        u.reg_write(UC_M68K_REG_A7, sp + pop)
        u.reg_write(UC_M68K_REG_PC, address + 2)

    def run(self, offset, args=b"", until=STOP):
        self.u.reg_write(UC_M68K_REG_A7, STACK)
        self.write(STACK, STOP, 4)
        self.u.mem_write(STACK + 4, args)
        self.u.emu_start(BASE + offset, until, count=200000)
        if self.u.reg_read(UC_M68K_REG_PC) != until:
            raise RuntimeError(f"Instruction limit reached at {self.u.reg_read(UC_M68K_REG_PC) - BASE:04x}")

    def step(self, keys=""):
        self.keys = [KEYS[k] for k in keys if k in KEYS]
        self.log = []
        start = self.now
        self.run(0x4a24)
        state = self.read(GLOBAL - 0x3c2, 1)
        return {
            "pos": [self.read(GLOBAL - 0x3c6) - 1, self.read(GLOBAL - 0x3c4) - 1],
            "state": state, "finished": bool(self.read(GLOBAL - 0x438, 1)),
            "facing": "R" if self.read(GLOBAL - 0x3c1, 1) else "L",
            "clock": [start, self.now],
            "requested_delays": [e[1] for e in self.log if e[0] == "present"],
            "grid": [[self.read(GLOBAL - 0x3bc + x * 14 + y, 1) for x in range(1, 17)] for y in range(1, 13)],
            "draws": self.log.copy(),
        }


def scenarios():
    def case(name, steps, changes=(), floor=5):
        grid = [["."] * 16 for _ in range(12)]
        grid[4][4] = "I"
        if floor is not None:
            grid[floor] = ["#"] * 16
        for x, y, c in changes:
            grid[y][x] = c
        return {"name": name, "steps": steps.split(","), "rows": ["".join(row) for row in grid]}
    result = [
        case("idle", "-,-,-"),
        case("walk", "R,R,R"),
        case("turn", "L,L,L"),
        case("up_holds_position", "UR,UR,UR"),
        case("crouch_holds_position", "DR,DR,DR"),
        case("face_and_cast", "LM"),
        case("blue_create", "M"),
        case("blue_over_hot", "M", [(5, 5, "R")]),
        case("star_destroy", "M", [(5, 4, "*")]),
        case("blue_destroy", "M", [(5, 4, "b")]),
        case("green_destroy", "M", [(5, 4, "g")]),
        case("warp_pocket_destroy", "M", [(5, 4, "U")]),
        case("magic_dud", "M", [(5, 4, "#")]),
        case("elevator_side_magic", "M", [(5, 4, "^")]),
        case("elevator_diagonal_magic", "D,DM", [(5, 5, "^")]),
        case("levitate", "R,U,UM"),
        case("levitate_into_tunnel", "R,U,UM", [(5, 3, "|")]),
        case("elevator_blocked", "R,-,-", [(5, 4, "^"), (5, 3, "#")]),
        case("elevator_overhead", "U,U,-", [(4, 3, "^")]),
        case("tunnel_holds_hero", "-,R,R", [(4, 5, "|")], floor=6),
        case("vertical_warp", "R,-,-", [(5, 4, "%"), (5, 8, "%")]),
        case("warp_wraps", "R,-,-", [(5, 4, "%"), (5, 1, "%")]),
        case("companion_adjacency", "-", [(3, 4, "(")]),
        case("companion_right_square", "R,R,-", [(5, 4, ")"), (4, 3, "(")]),
        case("fall", "-,-,-", floor=8),
        case("fall_offscreen", ",".join(["-"] * 11), floor=None),
        case("hot_wall", "-,-", [(4, 5, "R")]),
        case("warp_pocket", "-,-,-", [(4, 5, "U")], floor=6),
        case("crumble_idle", ",".join(["-"] * 12), [(4, 5, "~")]),
        case("crumble_during_magic", "-,M,-", [(4, 5, "~")]),
        case("exit", "R,U,U", [(5, 4, "O")]),
    ]
    for char in ['/', '"', '&']:
        result.append(case(f"decoration_{ord(char)}", "R,R", [(5, 4, char)]))
    for char in ["<", ">", "|", "^", "%"]:
        result.append(case(f"sideways_{ord(char)}", "R", [(5, 4, char)]))
    for char in '.#*ObgUR+~<>^|%()/"&':
        result.append(case(f"matrix_side_{ord(char)}", "M,M", [(5, 4, char)]))
        result.append(case(f"matrix_left_{ord(char)}", "L,-,M", [(3, 4, char)]))
        result.append(case(f"matrix_down_{ord(char)}", "D,DM,DM", [(5, 5, char)]))
        result.append(case(f"matrix_up_{ord(char)}", "R,U,UM,UM", [(5, 3, char)]))
    result.append(case("matrix_underfoot", "R,U,UM,D,D,DM,-"))
    for keys in ['UD', 'UDM', 'LR', 'LRM', 'UL', 'DL', 'UM', 'DM']:
        result.append(case(f"matrix_keys_{keys}", ','.join([keys] * 4)))
    mapping = dict(zip('012345678!@#$%()/^&', '.*#IOUR+~<>^|%()/"&'))
    for bits in range(32):
        keys = ''.join(key for i, key in enumerate('LRUDM') if bits & (1 << i)) or '-'
        for stance, prefix, floor in [('stand', '-', 5), ('up', 'U', 5), ('crouch', 'D', 5), ('fall', '-', 8)]:
            result.append(case(f"matrix_chord_{stance}_{bits}", f"{prefix},{keys},{keys},-", floor=floor))
    result.append(case("matrix_crumble_walk_back", "-,R,L,L,-,R,R", [(4, 5, '~'), (5, 5, '~')]))
    result.append(case("matrix_warp_cast_and_return", "R,-,M,U,UM,D,DM,L,L,R,R,-", [(5, 4, '%'), (5, 8, '%')]))
    result.append(case("matrix_crumble_falling", ",".join(['-'] * 14), [(4, 6, '~')], floor=8))
    for x,y in [(0,0), (15,0), (0,11), (15,11)]:
        boundary = case(f"matrix_edge_{x}_{y}", "U,UM,D,DM,L,L,M,R,R,M,-,-")
        rows = [list(row) for row in boundary['rows']]
        rows[4][4] = '.'
        rows[y][x] = 'I'
        boundary['rows'] = [''.join(row) for row in rows]
        result.append(boundary)
    for number in range(1, 51):
        raw = (OUT / f"TEXT-{number + 200}.bin").read_bytes().decode("mac_roman")
        rows = [''.join(mapping[c] for c in row) for row in raw.split('\r')]
        steps = ['-'] * 10 + ['R'] * 5 + ['U', 'UM', 'UM', '-', 'M', 'M', 'D', 'DM', '-'] + ['L'] * 5 + ['U', '-', 'M']
        result.append({"name": f"campaign_{number:02}", "rows": rows, "steps": steps, "hero": int(number > 25)})
    return result


def metadata():
    original = Original()
    original.run(0x1202, until=BASE + 0x18aa)
    tables = []
    for offset in [-0x2bc, -0x2b8]:
        ptr = original.read(GLOBAL + offset, 4)
        values = []
        for i in range(50):
            p = ptr + 256 * i
            raw = bytes(original.u.mem_read(p + 1, original.read(p, 1)))
            values.append(raw.decode("mac_roman") if offset == -0x2bc else "".join(chr(155 - x) for x in raw))
        tables.append(values)
    return [{"level": i + 1, "name": tables[0][i], "password": tables[1][i]} for i in range(50)]


def tile_mapping():
    entries = []
    for char in "12345678!@#$%(/^&":
        original = Original()
        original.u.mem_write(0x70000, struct.pack(">hhhh", 0, 0, 40, 40))
        original.run(0x3dc2, struct.pack(">IIH", 0x70000, 0x70100, ord(char)))
        draws = [e for e in original.log if e[0] == "copy" and e[4] == "4052"]
        entries.append({"code": char, "runtime_type": original.read(0x70100, 1), "source_rect": draws[0][1]})
    return entries


def campaign_checks(table):
    mapping = dict(zip('012345678!@#$%()/^&', '.*#IOUR+~<>^|%()/"&'))
    checks = []
    for entry in table:
        number = entry["level"]
        raw = (OUT / f"TEXT-{number + 200}.bin").read_bytes().decode("mac_roman")
        expected = ["".join(mapping[c] for c in row) for row in raw.split("\r")]
        text = Path(f"assets/levels/{number:02}.level.ron").read_text()
        row_section = text.split("rows: [", 1)[1].split("]", 1)[0]
        rows = [json.loads(m.group(0)) for m in re.finditer(r'"(?:[^"\\]|\\.)*"', row_section)]
        password_match = re.search(r'password: Some\("([^"\n]*)"\)', text)
        password = password_match.group(1) if password_match else ""
        checks.append({**entry, "rows_match": rows == expected,
                       "name_matches": f'name: "{entry["name"]}"' in text,
                       "password_in_port": password, "password_matches": password == entry["password"],
                       "hero_matches": f'hero: {"A" if number <= 25 else "B"}' in text})
    return checks


def frame_fixture(draws):
    frames, blits, sounds = [], [], []
    for draw in draws:
        if draw[0] in ('hero', 'tile') or (draw[0] == 'copy' and draw[4] == '38e6'):
            source, dest = draw[1:3]
            t,l,b,r = source
            dt,dl,db,dr = dest
            blit = ','.join(map(str, [dict(hero='h',tile='t',copy='c')[draw[0]],l,t,r-l,b-t,dl,dt,dr-dl,db-dt]))
            if draw[0] == 'tile' and draw[4] == '22a0': frames[-1][3].append(blit)
            else: blits.append(blit)
        elif draw[0] == 'restore' and 0 <= draw[1] < 16 and 0 <= draw[2] < 12:
            restore = ','.join(map(str, ['r', *draw[1:4]]))
            if draw[4] == '2274': frames[-1][3].append(restore)
            else: blits.append(restore)
        elif draw[0] == 'sound': sounds.append(str(draw[1]))
        elif draw[0] == 'present':
            frames.append([str(draw[1]), blits, sounds, [], []])
            blits, sounds = [], []
    if frames: frames[-1][4].extend(sounds)
    return '/'.join('@'.join([ticks, ';'.join(blits), ','.join(sounds), ';'.join(after), ','.join(tail)])
                    for ticks, blits, sounds, after, tail in frames)


def transitions():
    lines = []
    for name, offset in [('exit', 0x1e96), ('reunion', 0x1f86)]:
        original = Original()
        original.u.mem_write(GLOBAL - 0x42a, struct.pack('>hhhh', 0, 0, 480, 640))
        original.run(offset)
        lines.append('@' + name)
        for draw in original.log:
            if draw[0] in ('black', 'invert', 'oval'):
                lines.append('\t'.join(map(str, [draw[0], *draw[1], draw[2] - 100])))
    Path('tests/fixtures').mkdir(parents=True, exist_ok=True)
    Path('tests/fixtures/transitions.tsv').write_text('\n'.join(lines) + '\n')


def write_fixtures(output):
    lines = []
    for case in output["scenarios"] + output["speed_scenarios"]:
        lines.append("@" + case["name"] + "\t" + ",".join(case["rows"]) + "\t" + str(case.get("speed", 2)) + "\t" + str(case.get("hero", 0)))
        previous_start = case["traces"][0]["original"]["clock"][0]
        for trace in case["traces"]:
            ref = trace["original"]
            ticks = ref["clock"][0] - previous_start
            previous_start = ref["clock"][0]
            grid = bytes(t for row in ref["grid"] for t in row).hex()
            lines.append("\t".join([trace["keys"], str(ticks), ",".join(map(str, ref["pos"])),
                                    ref["facing"], str(ref["finished"]).lower(), grid, ",".join(map(str, ref["requested_delays"])), frame_fixture(ref["draws"]), str(ref["clock"][1] - ref["clock"][0])]))
            if ref["finished"]: break
    path = Path("crates/stardust-core/tests/fixtures")
    path.mkdir(parents=True, exist_ok=True)
    (path / "binary_scenarios.tsv").write_text("\n".join(lines) + "\n")
    path = Path("crates/stardust-level/tests/fixtures")
    path.mkdir(parents=True, exist_ok=True)
    (path / "original_passwords.txt").write_text("\n".join(e["password"] for e in output["campaign_checks"]) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write-fixtures", action="store_true", help="Regenerate checked-in original-binary test fixtures")
    args = parser.parse_args()
    if hashlib.sha256(CODE.read_bytes()).hexdigest() != CODE_SHA:
        raise RuntimeError("CODE resource differs from the audited Stardust 1.1 binary")
    dis = Cs(CS_ARCH_M68K, CS_MODE_BIG_ENDIAN | CS_MODE_M68K_000)
    dis.skipdata = True
    (OUT / "CODE-1.asm").write_text("\n".join(
        f"{i.address:04x}: {i.bytes.hex():20} {i.mnemonic:12} {i.op_str}" for i in dis.disasm(CODE.read_bytes(), 0)) + "\n")
    cases = scenarios()
    for case in cases:
        original = Original(case["rows"], hero=case.get("hero", 0))
        case["traces"] = [{"keys": keys, "original": original.step(keys)} for keys in case["steps"]]
    timings = []
    for speed, name in [(0, "fast"), (2, "normal"), (1, "slow")]:
        for case in cases[:]:
            if case["name"] not in {"idle", "walk", "fall", "blue_create", "star_destroy", "blue_destroy", "green_destroy", "magic_dud", "levitate", "exit", "crumble_idle"}:
                continue
            original = Original(case["rows"], speed, hero=case.get("hero", 0))
            timings.append({"speed": name, "scenario": case["name"],
                            "steps": [original.step(k) for k in case["steps"]]})
    speed_scenarios = []
    for speed in [0, 1]:
        for case in cases:
            original = Original(case["rows"], speed, hero=case.get("hero", 0))
            speed_scenarios.append({"name": f"speed_{speed}_{case['name']}", "speed": speed, "rows": case["rows"], "hero": case.get("hero", 0),
                "traces": [{"keys": k, "original": original.step(k)} for k in case["steps"]]})
    output = {"speed_scenarios": speed_scenarios, "code_sha256": CODE_SHA, "method": "Original CPU instructions with mocked OS drawing/audio and deterministic TickCount/Random; fixtures record complete action calls, presentation deadlines, restoration/blit rectangles, and sound requests.",
              "campaign_checks": campaign_checks(metadata()), "tile_mapping": tile_mapping(), "scenarios": cases, "timings": timings}
    (OUT / "traces.json").write_text(json.dumps(output, indent=2) + "\n")
    if args.write_fixtures:
        write_fixtures(output)
        transitions()
    for case in cases:
        ref = case["traces"][-1]["original"]
        print(f"{case['name']:28} position={ref['pos']} state={ref['state']} finished={ref['finished']}")
    print(f"Wrote {len(cases)} scenarios and {len(timings)} timing traces to {OUT / 'traces.json'}")


if __name__ == "__main__":
    main()
