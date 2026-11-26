# Tree model

`tree.glb` replaces every procedural tree: on the farm, the scenery behind
the fence and the windbreaks far away. The game stands it on the ground,
scales it to each tree's height (6 m tall as made, see `TREE_MODEL_HEIGHT`)
and lets the crown sway in the wind; a flat ground disc under the model, if
it has one, is left out (the terrain is the ground).

Models are embedded into the game at build time: rebuild after changing one.

`scripts/flower_model.py <model.glb> tree --out assets/trees/tree.glb
--triangles 100000` makes it from any tree model (+Y up): it bakes the
base-colour texture into vertex colours (and simplifies only if the model has
more triangles than asked for).
