#!/usr/bin/env python3
"""Download the original Stardust archive and extract vendor resources and assets."""

import hashlib
from pathlib import Path
import subprocess
from urllib.request import urlopen

URL = "https://archive.org/download/stardust-mac-en_202609/Stardust_Mac_EN.sit"
SHA256 = "078c8d53a15d67a21304770017fa1704f015a5660ebd0311168da57e1ffc0c2c"


def extract(root):
    archive = root / "archive/Stardust_Mac_EN.sit"
    if archive.exists():
        data = archive.read_bytes()
    else:
        print(f"Downloading {URL}", flush=True)
        with urlopen(URL, timeout=60) as response:
            data = response.read()
    if hashlib.sha256(data).hexdigest() != SHA256:
        raise RuntimeError("Stardust archive does not match the original binary used by this project")
    if not archive.exists():
        archive.parent.mkdir(parents=True, exist_ok=True)
        archive.write_bytes(data)
    subprocess.run(
        ["cargo", "run", "--locked", "-p", "stardust-extract", "--", str(archive)],
        cwd=root, check=True,
    )


if __name__ == "__main__":
    extract(Path(__file__).resolve().parents[1])
