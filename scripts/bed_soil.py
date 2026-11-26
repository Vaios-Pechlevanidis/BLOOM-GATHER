#!/usr/bin/env python3
"""Turn a model of a garden bed (soil framed by stones, any size) into the
pieces the game builds its flower beds from, and save them as
assets/beds/soil.glb:

  * cut across into a start, a middle and an end piece, the cuts where the
    stones along the sides leave a gap, so the middle piece repeats along a
    bed of any length without breaking a stone
  * each piece twice: "near" (fine, for beds close to the player) and "far"
  * the base-colour texture baked into vertex colours (see flower_model.py)

Usage: scripts/bed_soil.py <model.glb> [--near-triangles 6000] [--far-triangles 700]
Example: scripts/bed_soil.py ~/Downloads/garden_soil_plot.glb

The model must lie flat (+Y up) with its long side, if any, along Z. The
budgets are for the whole model. Each mesh is named "<piece> <detail>
length=<metres> width=<metres>": the length the piece covers along Z (its
triangles may overlap the cut a little) and the model's width. The soil
surface is at height 0. Requires numpy, Pillow and fast_simplification.
"""

import argparse
import pathlib

import numpy as np

import flower_model as fm


def find_cuts(positions, colours):
    """Where to cut across: in the lower and upper middle of the model, at
    the Z where the fewest stone (light) vertices lie along the sides."""
    lo, hi = positions.min(0), positions.max(0)
    half_w = (hi[0] - lo[0]) / 2.0
    depth = hi[2] - lo[2]
    stone = colours[:, 1] > 0.25  # light beige, not dark soil
    side = stone & (np.abs(positions[:, 0]) > half_w * 0.6)
    zs = positions[side, 2]
    band = depth * 0.02

    def best(a, b, prefer):
        candidates = np.linspace(a, b, 120)
        scores = np.array([np.count_nonzero(np.abs(zs - z) < band) for z in candidates])
        ties = candidates[scores == scores.min()]
        return ties[np.argmin(np.abs(ties - prefer))]

    z1 = best(lo[2] + depth * 0.2, lo[2] + depth * 0.4, lo[2])
    z2 = best(hi[2] - depth * 0.4, hi[2] - depth * 0.2, hi[2])
    return z1, z2


def piece(positions, colours, triangles, z_from, z_to, shift):
    """Triangles whose middle lies in z_from..z_to, moved by -shift along Z."""
    middle = positions[triangles].mean(axis=1)[:, 2]
    keep = triangles[(middle >= z_from) & (middle < z_to)]
    used, inverse = np.unique(keep, return_inverse=True)
    p = positions[used].copy()
    p[:, 2] -= shift
    t = inverse.reshape(-1, 3)
    return p, fm.smooth_normals(p, t), colours[used], t


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("model", type=pathlib.Path)
    ap.add_argument("--near-triangles", type=int, default=6000)
    ap.add_argument("--far-triangles", type=int, default=700)
    args = ap.parse_args()

    positions, colours, triangles = fm.gather(fm.Glb(args.model))
    positions, colours, triangles = fm.weld(positions, colours, triangles)
    # Centre it, the soil surface (middle, not stones) at height 0.
    lo, hi = positions.min(0), positions.max(0)
    positions[:, [0, 2]] -= (lo[[0, 2]] + hi[[0, 2]]) / 2.0
    lo, hi = positions.min(0), positions.max(0)
    inner = (np.abs(positions[:, 0]) < hi[0] * 0.4) & (np.abs(positions[:, 2]) < hi[2] * 0.4)
    positions[:, 1] -= np.median(positions[inner, 1])
    lo, hi = positions.min(0), positions.max(0)
    z1, z2 = find_cuts(positions, colours)
    width = hi[0] - lo[0]
    print(f"model {width:.3f} x {hi[2] - lo[2]:.3f}, cuts at z {z1:.3f} and {z2:.3f}")

    meshes = []
    for detail, budget in [("near", args.near_triangles), ("far", args.far_triangles)]:
        p, c, t = fm.simplify(positions, colours, triangles, budget)
        inf = np.inf
        for name, z_from, z_to, shift, length in [
            ("start", -inf, z1, lo[2], z1 - lo[2]),
            ("middle", z1, z2, z1, z2 - z1),
            ("end", z2, inf, z2, hi[2] - z2),
        ]:
            pp, nn, cc, tt = piece(p, c, t, z_from, z_to, shift)
            meshes.append((f"{name} {detail} length={length:.5f} width={width:.5f}", pp, nn, cc, tt))
            print(f"  {name:6} {detail}: {len(tt)} triangles, {length:.3f} long")

    out = pathlib.Path(__file__).resolve().parent.parent / "assets" / "beds" / "soil.glb"
    out.parent.mkdir(exist_ok=True)
    fm.write_glb_meshes(out, meshes)
    print(f"{out} ({out.stat().st_size // 1024} KB)")


if __name__ == "__main__":
    main()
