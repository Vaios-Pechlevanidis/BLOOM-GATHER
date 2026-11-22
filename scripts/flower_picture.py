#!/usr/bin/env python3
"""Turn a flower picture (PNG, WebP, ... with a transparent background) into
the game's format and save it as assets/flowers/<species id>.png:

  * cropped to the plant, the stem base at the bottom centre
  * a power-of-two canvas: 1024x1024, or 512x1024 for slender plants
  * PNG, RGBA

Usage: scripts/flower_picture.py <picture> <species id> [--size 1024]
Example: scripts/flower_picture.py ~/Downloads/daisy.webp daisy

The game measures the picture itself (where the plant starts and ends) and
spreads edge colours into the transparent surroundings, so this only has to
get the canvas right. Requires Pillow. Rebuild the game afterwards; the
pictures are embedded at build time.
"""

import argparse
import pathlib

from PIL import Image

CUTOFF = 128  # alpha that counts as "plant"


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("picture", type=pathlib.Path)
    ap.add_argument("species", help="species id from game_data.ron, e.g. daisy")
    ap.add_argument("--size", type=int, default=1024, help="canvas height (power of two)")
    args = ap.parse_args()

    im = Image.open(args.picture).convert("RGBA")
    alpha = im.getchannel("A")
    if alpha.getextrema()[0] == 255:
        raise SystemExit("The picture has no transparent background. Remove it first.")
    box = alpha.point(lambda a: 255 if a >= CUTOFF else 0).getbbox()
    if box is None:
        raise SystemExit("The picture is empty.")
    left, top, right, bottom = box

    # The stem: the middle of what is solid in the lowest rows.
    rows = max(1, im.height // 50)
    xs = [
        x
        for y in range(bottom - rows, bottom)
        for x in range(left, right)
        if alpha.getpixel((x, y)) >= CUTOFF
    ]
    stem = sum(xs) / len(xs) + 0.5
    half = max(stem - left, right - stem)
    plant = im.crop((round(stem - half), top, round(stem + half), bottom))

    size = args.size
    width = size // 2 if plant.width <= plant.height * 0.48 else size
    scale = min(size * 0.97 / plant.height, width / plant.width)
    plant = plant.resize(
        (max(1, round(plant.width * scale)), max(1, round(plant.height * scale))),
        Image.LANCZOS,
    )
    canvas = Image.new("RGBA", (width, size), (0, 0, 0, 0))
    canvas.alpha_composite(plant, ((width - plant.width) // 2, size - plant.height))

    out = pathlib.Path(__file__).resolve().parent.parent / "assets/flowers" / f"{args.species}.png"
    out.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(out, optimize=True)
    print(f"{out}: {width}x{size}")


if __name__ == "__main__":
    main()
