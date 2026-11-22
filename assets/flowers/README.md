# Flower pictures

A picture named after a species id (`daisy.png`, `sunflower.png`, ... — the ids
are in `assets/data/game_data.ron`) replaces that species' procedural model.
The game draws it on an upright card that always turns to face the player
and sways in the wind, so the plant looks like the picture from every side.
Species without a picture keep their procedural model.

Pictures are embedded into the game at build time: rebuild after adding or
changing one.

## Format

| | |
|---|---|
| File | PNG with alpha, `<species id>.png` |
| Size | powers of two: 1024x1024, or 512x1024 for slender plants |
| Background | fully transparent; no ground, pot or cast shadow |
| Content | one whole plant, side view, upright, nothing cropped |
| Position | stem base at the bottom centre; little room above the bloom |
| Light | even and soft; no strong highlights (the game lights the plant) |

The game measures each picture: the plant's top is placed at the species'
`height`, the stem base on the ground, whatever the margins. Edge colours are
spread into the transparent surroundings, and the mip chain keeps petals from
thinning out in the distance.

`scripts/flower_picture.py <picture> <species id>` turns a picture with a
transparent background (PNG, WebP, ...) into this format.

## Night flowers

`<species id>_glow.png` (same size, black except the parts that glow) makes
those parts glow in the species' petal colour. Without it, a pictured night
flower does not glow.
