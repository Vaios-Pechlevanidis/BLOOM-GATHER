# Lawn model

`lawn.glb` is the meadow's lawn: one square tile of short grass blades (model
units are metres). The game lays it over the meadow around the player (off
paths, beds, water, trunks, rocks and buildings), each tile turned at random
so no pattern shows, swaying in the wind. Further away the terrain carries
on; the region's ground colours in `game_data.ron` are matched to the lawn.
Without the model the meadow gets the old procedural grass tufts.

The game varies the tiles as it lays them (`src/rendering/lawn.rs`): patches
of longer and shorter, lusher and sun-dried grass from smooth noise, a few
tiny daisies, buttercups and clover heads, and a soft edge where the lawn
thins out blade by blade into the terrain.

Models are embedded into the game at build time: rebuild after changing one.

`scripts/lawn_tile.py <model.glb>` makes `lawn.glb` from a model of a lawn
patch (lying flat, +Y up). Such a model is one fused mesh of hundreds of
thousands of triangles, which simplifying melts into blobs, so the script
measures it instead (blade density, blade heights, colours from root to tip)
and grows a tile of separate, crisp blades just like them (`--side`,
`--density`).
