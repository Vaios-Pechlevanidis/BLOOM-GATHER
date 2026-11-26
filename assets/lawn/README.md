# Lawn model

`lawn.glb` is the meadow's lawn: one square tile of short grass blades (model
units are metres). The game lays it over the meadow around the player (off
paths, beds, water, trunks, rocks and buildings), each tile turned at random
so no pattern shows, swaying in the wind. Further away the terrain carries
on; the region's ground colours in `game_data.ron` are matched to the lawn.
Without the model the meadow gets the old procedural grass tufts.

Models are embedded into the game at build time: rebuild after changing one.

`scripts/lawn_tile.py <model.glb>` makes `lawn.glb` from a model of a lawn
patch (lying flat, +Y up). Such a model is one fused mesh of hundreds of
thousands of triangles, which simplifying melts into blobs, so the script
measures it instead (blade density, blade heights, colours from root to tip)
and grows a tile of separate, crisp blades just like them (`--side`,
`--density`).
