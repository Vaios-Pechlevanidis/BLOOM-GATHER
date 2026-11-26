# Flower bed models

`soil.glb` is the ground of the bulb field beds: a stone-framed garden bed,
cut across into a start, a middle and an end piece. The game builds every
bed from the start piece, the middle piece as often as the bed is long
(stretched a little to fit exactly) and the end piece, scaled to the bed's
width and laid over the terrain, so the flowers stand on its soil. Near the
player it uses the fine pieces, further away the coarse ones.

Models are embedded into the game at build time: rebuild after changing one.

`scripts/bed_soil.py <model.glb>` makes `soil.glb` from a model of a garden
bed (any size, lying flat, +Y up): it bakes the base-colour texture into
vertex colours, cuts where the stones along the sides leave a gap (so the
middle piece repeats without breaking a stone) and simplifies each piece
twice (`--near-triangles`, `--far-triangles`, for the whole model).

Without `soil.glb` the beds are plain brown terrain ridges.
