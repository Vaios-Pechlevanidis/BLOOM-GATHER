#!/usr/bin/env python3
"""Make the tile the game lays its meadows with from a model of a lawn patch
(a sod of grass blades), and save it as assets/lawn/lawn.glb.

A lawn model is one fused mesh of hundreds of thousands of triangles:
simplifying it melts the blades into blobs. So this measures the model
instead (how densely the blades stand, how tall they grow, their colours
from root to tip) and grows a tile of separate, crisp blades just like them:
each a tapered, slightly leaning blade of three triangles, darker at the
root and lighter at the tip, with a light and a shaded side.

Usage: scripts/lawn_tile.py <model.glb> [--side 0.5] [--density 1.4] [--seed 7]
Example: scripts/lawn_tile.py ~/Downloads/grass_lawn_patch.glb

`--side` is the tile's size in model units (taken as metres), `--density`
scales the measured number of blades. The mesh is named "tile side=<units>",
centred on the origin, the ground at height 0; the colours are the model's.
Requires numpy, Pillow and fast_simplification.
"""

import argparse
import pathlib

import numpy as np

import flower_model as fm


def crop(positions, colours, triangles, centre, side):
    """The triangles whose middle lies in the square, re-indexed, centred."""
    middle = positions[triangles].mean(axis=1)
    inside = (np.abs(middle[:, 0] - centre[0]) < side / 2) & (np.abs(middle[:, 2] - centre[1]) < side / 2)
    t = triangles[inside]
    used, inverse = np.unique(t, return_inverse=True)
    p = positions[used].copy()
    p[:, 0] -= centre[0]
    p[:, 2] -= centre[1]
    return p, colours[used], inverse.reshape(-1, 3)


def blade_tips(positions, triangles, height):
    """Heights of the separate blade tips poking above `height`."""
    tt = triangles[positions[triangles][:, :, 1].min(axis=1) > height]
    parent = np.arange(len(positions))

    def find(x):
        root = x
        while parent[root] != root:
            root = parent[root]
        while parent[x] != root:
            parent[x], x = root, parent[x]
        return root

    for a, b in np.vstack([tt[:, [0, 1]], tt[:, [1, 2]]]):
        ra, rb = find(a), find(b)
        if ra != rb:
            parent[ra] = rb
    verts = np.unique(tt)
    roots = np.array([find(v) for v in verts])
    return np.array([positions[verts[roots == r], 1].max() for r in np.unique(roots)])


def measure(model: pathlib.Path, side: float):
    positions, colours, triangles = fm.gather(fm.Glb(model))
    lo, hi = positions.min(0), positions.max(0)
    centre = ((lo[0] + hi[0]) / 2, (lo[2] + hi[2]) / 2)
    p, c, t = crop(positions, colours, triangles, centre, side)
    del positions, colours, triangles
    p, c, t = fm.weld(p, c, t)
    low = p[:, 1].min()
    t = t[p[t][:, :, 1].max(axis=1) > low + 0.02]  # not the sod's underside
    p[:, 1] -= np.percentile(p[np.unique(t), 1], 8)  # the ground between the blades
    top = p[:, 1].max()
    tips = blade_tips(p, t, top * 0.5)
    # Blades below the cut were not counted; the short ones fill in.
    density = len(tips) / side**2 * 1.4
    bands = []
    for a, b in [(0.0, 0.25), (0.25, 0.6), (0.6, 1.01)]:
        m = (p[:, 1] >= a * top) & (p[:, 1] < b * top)
        bands.append(c[m].mean(axis=0) if m.any() else c.mean(axis=0))
    return density, np.percentile(tips, [5, 50, 95]), bands


def grow(side, density, heights, bands, rng):
    """A tile of blades: positions, normals, colours, triangles."""
    count = int(density * side * side)
    root, middle, tip = bands
    p, n, c, t = [], [], [], []
    for _ in range(count):
        at = rng.uniform(-side / 2, side / 2, 2)
        # Mostly blades as tall as the model's, some short ones between.
        if rng.random() < 0.75:
            h = rng.uniform(heights[0], heights[2])
        else:
            h = rng.uniform(heights[0] * 0.45, heights[0])
        w = rng.uniform(0.013, 0.022)
        yaw = rng.uniform(0, 2 * np.pi)
        across = np.array([np.cos(yaw), 0.0, np.sin(yaw)])
        face = np.array([-np.sin(yaw), 0.0, np.cos(yaw)])
        lean = face * rng.uniform(-0.35, 0.35) + across * rng.uniform(-0.12, 0.12)
        base = np.array([at[0], 0.0, at[1]])
        mid = base + np.array([0, h * 0.55, 0]) + lean * h * 0.25
        top = base + np.array([0, h, 0]) + lean * h
        # The blade's own facing, tipped up a little: a lit and a shaded side.
        normal = face * np.sign(rng.uniform(-1, 1)) + np.array([0, 0.6, 0])
        normal /= np.linalg.norm(normal)
        shade = rng.uniform(0.88, 1.1)
        k = len(p)
        for v, col in [
            (base - across * w / 2, root * 0.85),
            (base + across * w / 2, root * 0.85),
            (mid - across * w * 0.36, middle),
            (mid + across * w * 0.36, middle),
            (top, tip * 1.18),
        ]:
            p.append(v)
            n.append(normal)
            c.append(np.clip(col * shade, 0, 1))
        t += [[k, k + 2, k + 1], [k + 1, k + 2, k + 3], [k + 2, k + 4, k + 3]]
    return np.array(p), np.array(n), np.array(c), np.array(t)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("model", type=pathlib.Path)
    ap.add_argument("--side", type=float, default=0.5)
    ap.add_argument("--density", type=float, default=1.4)
    ap.add_argument("--seed", type=int, default=7)
    args = ap.parse_args()

    density, heights, bands = measure(args.model, args.side)
    density *= args.density
    print(f"model: {density:.0f} blades per square unit, tips {heights.round(3)}")
    p, n, c, t = grow(args.side, density, heights, bands, np.random.default_rng(args.seed))
    out = pathlib.Path(__file__).resolve().parent.parent / "assets" / "lawn" / "lawn.glb"
    out.parent.mkdir(exist_ok=True)
    fm.write_glb_meshes(out, [(f"tile side={args.side:.5f}", p, n, c, t)])
    print(f"{len(t) // 3} blades, {len(t)} triangles: {out} ({out.stat().st_size // 1024} KB)")


if __name__ == "__main__":
    main()
