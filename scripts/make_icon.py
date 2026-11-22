#!/usr/bin/env python3
"""Draw the Bloom & Gather app icon (a five-petal flower on a soft sage
tile) and write every size the packages need into packaging/icon/:

  icon-1024.png      master image (Linux, docs)
  icon-256.png       Linux desktop icon
  AppIcon.icns       macOS (via iconutil)
  icon.ico           Windows (PNG-compressed ICO, written by hand)
  icon-64.rgba       raw 64x64 RGBA for the window icon (Windows/Linux)

Requires Pillow. The outputs are committed, so building packages does not
need Python.
"""
import io
import math
import os
import shutil
import struct
import subprocess
import tempfile

from PIL import Image, ImageDraw, ImageFilter

ROOT = os.path.join(os.path.dirname(__file__), "..", "packaging", "icon")
S = 1024  # master size
SS = 4    # supersampling


def lerp(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(len(a)))


def master():
    n = S * SS
    img = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    # Rounded tile with a soft vertical gradient (cream to sage).
    tile = Image.new("RGBA", (n, n))
    top, bottom = (247, 241, 224, 255), (178, 198, 158, 255)
    px = tile.load()
    for y in range(n):
        c = lerp(top, bottom, (y / n) ** 1.3)
        for x in range(n):
            px[x, y] = c
    mask = Image.new("L", (n, n), 0)
    margin = int(n * 0.06)
    ImageDraw.Draw(mask).rounded_rectangle(
        (margin, margin, n - margin, n - margin), radius=int(n * 0.2), fill=255
    )
    img.paste(tile, (0, 0), mask)
    d = ImageDraw.Draw(img)
    cx, cy = n * 0.5, n * 0.47
    # Stem and a leaf.
    stem = (104, 146, 86, 255)
    d.line([(cx, cy), (cx - n * 0.02, n * 0.86)], fill=stem, width=int(n * 0.035))
    d.ellipse(
        (cx - n * 0.2, n * 0.62, cx - n * 0.02, n * 0.72), fill=(126, 168, 104, 255)
    )
    # Five petals: soft pink with a lighter edge.
    petal_r = n * 0.16
    for k in range(5):
        a = -math.pi / 2 + k * 2 * math.pi / 5
        px_, py_ = cx + math.cos(a) * n * 0.15, cy + math.sin(a) * n * 0.15
        d.ellipse(
            (px_ - petal_r, py_ - petal_r, px_ + petal_r, py_ + petal_r),
            fill=(238, 170, 182, 255),
            outline=(250, 214, 220, 255),
            width=int(n * 0.012),
        )
    # Centre.
    r = n * 0.085
    d.ellipse((cx - r, cy - r, cx + r, cy + r), fill=(240, 200, 90, 255))
    r2 = n * 0.04
    d.ellipse(
        (cx - r2 - n * 0.02, cy - r2 - n * 0.02, cx + r2 - n * 0.02, cy + r2 - n * 0.02),
        fill=(250, 226, 140, 255),
    )
    img = img.filter(ImageFilter.GaussianBlur(radius=SS * 0.5))
    return img.resize((S, S), Image.LANCZOS)


def write_ico(img, path):
    sizes = [16, 24, 32, 48, 64, 128, 256]
    blobs = []
    for s in sizes:
        buf = io.BytesIO()
        img.resize((s, s), Image.LANCZOS).save(buf, "PNG")
        blobs.append(buf.getvalue())
    header = struct.pack("<HHH", 0, 1, len(sizes))
    offset = 6 + 16 * len(sizes)
    entries = b""
    for s, blob in zip(sizes, blobs):
        dim = 0 if s >= 256 else s
        entries += struct.pack("<BBBBHHII", dim, dim, 0, 0, 1, 32, len(blob), offset)
        offset += len(blob)
    with open(path, "wb") as f:
        f.write(header + entries + b"".join(blobs))


def main():
    os.makedirs(ROOT, exist_ok=True)
    img = master()
    img.save(os.path.join(ROOT, "icon-1024.png"))
    img.resize((256, 256), Image.LANCZOS).save(os.path.join(ROOT, "icon-256.png"))
    with open(os.path.join(ROOT, "icon-64.rgba"), "wb") as f:
        f.write(img.resize((64, 64), Image.LANCZOS).tobytes())
    write_ico(img, os.path.join(ROOT, "icon.ico"))
    if shutil.which("iconutil"):
        with tempfile.TemporaryDirectory() as tmp:
            iconset = os.path.join(tmp, "AppIcon.iconset")
            os.makedirs(iconset)
            for s in [16, 32, 128, 256, 512]:
                img.resize((s, s), Image.LANCZOS).save(os.path.join(iconset, f"icon_{s}x{s}.png"))
                img.resize((s * 2, s * 2), Image.LANCZOS).save(
                    os.path.join(iconset, f"icon_{s}x{s}@2x.png")
                )
            subprocess.run(
                ["iconutil", "-c", "icns", iconset, "-o", os.path.join(ROOT, "AppIcon.icns")],
                check=True,
            )
    print("Icons written to", os.path.normpath(ROOT))


if __name__ == "__main__":
    main()
