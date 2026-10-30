#!/usr/bin/env python3
"""Wrap a CommandSpace capture in a transparent rounded card.

    frame.py SHOT.png OUT.png

The canvas is transparent; only the rounded terminal card and its shadow are
opaque, so the image sits cleanly on both light and dark README backgrounds.
"""
import html
import os
import subprocess
import sys
from pathlib import Path

from PIL import Image

PAD = 35
RADIUS = 14
BORDER = "#3c3836"


def main() -> None:
    shot, out = sys.argv[1], sys.argv[2]
    width, height = Image.open(shot).size
    canvas_w, canvas_h = width + PAD * 2, height + PAD * 2

    out_dir = os.path.dirname(os.path.abspath(out))
    os.makedirs(out_dir, exist_ok=True)
    html_path = os.path.join(out_dir, ".frame.html")
    html_url = Path(html_path).as_uri()
    img_url = html.escape(Path(shot).resolve().as_uri())

    with open(html_path, "w", encoding="utf-8") as f:
        f.write(f"""<!doctype html>
<html><head><meta charset="utf-8"><style>
html, body {{ margin:0; padding:0; background:transparent; }}
body {{ width:{canvas_w}px; height:{canvas_h}px; padding:{PAD}px; box-sizing:border-box; }}
.card {{ width:{width}px; height:{height}px; border-radius:{RADIUS}px; overflow:hidden;
         background:#282828; outline:1px solid {BORDER}; outline-offset:-1px;
         box-shadow: 0 10px 30px rgba(0,0,0,.45); }}
img {{ display:block; width:{width}px; height:{height}px; }}
</style></head><body><div class="card"><img src="{img_url}"></div></body></html>
""")

    try:
        subprocess.run(
            [
                "chromium",
                "--headless=new",
                "--hide-scrollbars",
                "--force-device-scale-factor=1",
                "--default-background-color=00000000",
                f"--window-size={canvas_w},{canvas_h}",
                f"--screenshot={out}",
                html_url,
            ],
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
    finally:
        os.unlink(html_path)

    framed = Image.open(out)
    if framed.mode not in ("RGBA", "LA"):
        raise SystemExit(f"framed image lost its alpha channel (mode {framed.mode})")
    print(f"framed: {out} ({framed.width}x{framed.height})")


if __name__ == "__main__":
    main()
