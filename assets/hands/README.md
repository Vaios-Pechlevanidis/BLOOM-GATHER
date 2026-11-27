# First-person hands

The player's hands, in a cream knitted sleeve with a sage cuff:

- `open.glb`: a relaxed open hand, shown while picking by hand.
- `grip.glb`: a loose grip round a short wooden handle. With the handle it
  holds the garden hand rake (the game adds the tines at its end); without
  it, the garden scissors.
- `fist.glb`: a firm fist round a pole: holds the flower cart's long rake
  (the game draws the long pole and the rake head through the fist).

Each file has a mesh `hand` and, for the grips, a mesh `stick` (the handle;
the game finds its axis). The models end a short way behind the cuff; the
game carries the sleeve on as a knitted forearm in the sleeve's own colour
(`extend_sleeve` in `src/viewmodel.rs`), so no open sleeve end shows when the
hand reaches forward. Models are embedded into the game at build time:
rebuild after changing one. Without a file the game draws its simple
procedural hand instead (`src/viewmodel.rs`).

`scripts/hand_model.py` makes them from Meshy's FBX export (texture PNG next
to it): it bakes the texture into vertex colours on a finer mesh, turns the
hand into view-model space (fingers ahead, back of the hand up, mirrored
into a right hand) and splits off the handle. The shipped ones were made
with:

```
scripts/hand_model.py Cozy_Hand_Clean_Fingers.fbx open --forward=0.064,-0.439,0.896 --up=-0.409,0.801,0.437 --mirror
scripts/hand_model.py Cozy_Hand_Loose_Grip.fbx grip --forward=0.691,-0.428,0.583 --up=0.681,0.708,-0.186 --mirror
scripts/hand_model.py Cozy_Hand_Firm_Fist.fbx fist --forward=0.952,-0.072,0.299 --up=0.299,0,-0.952 --mirror
```
