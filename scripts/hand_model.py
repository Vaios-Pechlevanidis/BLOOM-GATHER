#!/usr/bin/env python3
"""Turn a first-person hand model (binary .fbx as Meshy and Blender export it,
with its base-colour texture next to it) into the game's format and save it
as assets/hands/<name>.glb:

  * the model's own placement applied (Blender's FBX: Z up, centimetres),
    turned +Y up, then into the game's hand space: fingers towards -Z, the
    back of the hand up (+Y), a right hand (`--mirror` a left one), its
    middle at the origin and `--length` metres long from fingertips to the
    end of the sleeve
  * split into its connected pieces: the largest is saved as the mesh
    "hand", the rest (a handle the hand holds) as "stick"
  * the colours of its texture baked into vertex colours (no texture to
    ship); a coarse model is first subdivided towards `--triangles` so the
    nails, knit and cuff stay crisp up close
  * positions, normals, colours and indices only

Usage: scripts/hand_model.py <model.fbx> <name> --forward=X,Y,Z --up=X,Y,Z [--mirror]
       [--length 0.3] [--texture <png>] [--triangles 4000]
Example (see assets/hands/README.md for the shipped hands):
  scripts/hand_model.py ~/Downloads/Cozy_Hand_Clean_Fingers.fbx open \
      --forward=0.064,-0.439,0.896 --up=-0.409,0.801,0.437 --mirror

--forward is the way the fingers point and --up the way the back of the hand
faces, both in the model's +Y-up space (the script prints the directions of
its pieces to help). The game places the hand on screen (`src/viewmodel.rs`).
Requires numpy and Pillow. Rebuild the game afterwards; the models are
embedded at build time.
"""

import argparse
import pathlib
import struct
import sys
import zlib

import numpy as np
from PIL import Image

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from flower_model import srgb_to_linear, write_glb_meshes  # noqa: E402


# --- Binary FBX ------------------------------------------------------------


def read_fbx(path: pathlib.Path):
    """The node tree of a binary FBX file as (name, properties, children)."""
    d = path.read_bytes()
    if not d.startswith(b"Kaydara FBX Binary"):
        raise SystemExit(f"{path} is not a binary FBX file.")
    version = struct.unpack_from("<I", d, 23)[0]
    wide = version >= 7500
    header = 25 if wide else 13

    def props(o: int, n: int):
        out = []
        for _ in range(n):
            t = chr(d[o])
            o += 1
            if t in "YCIFDL":
                fmt = {"Y": "<h", "C": "<?", "I": "<i", "F": "<f", "D": "<d", "L": "<q"}[t]
                out.append(struct.unpack_from(fmt, d, o)[0])
                o += struct.calcsize(fmt)
            elif t in "fdlib":
                count, encoding, size = struct.unpack_from("<III", d, o)
                o += 12
                raw = d[o : o + size]
                o += size
                if encoding:
                    raw = zlib.decompress(raw)
                dtype = {"f": "<f4", "d": "<f8", "l": "<i8", "i": "<i4", "b": "<i1"}[t]
                out.append(np.frombuffer(raw, dtype, count).copy())
            elif t in "SR":
                (size,) = struct.unpack_from("<I", d, o)
                o += 4
                out.append(d[o : o + size])
                o += size
            else:
                raise SystemExit(f"Unknown FBX property type {t!r}.")
        return out

    def node(o: int):
        end, count, length = struct.unpack_from("<QQQ" if wide else "<III", d, o)
        if end == 0:
            return None, o + header
        name_len = d[o + header - 1]
        name = d[o + header : o + header + name_len].decode()
        at = o + header + name_len
        p = props(at, count)
        c = at + length
        children = []
        while c < end:
            child, c = node(c)
            if child is None:
                break
            children.append(child)
        return (name, p, children), end

    nodes, o = [], 27
    while o < len(d) - header:
        n, o = node(o)
        if n is None:
            break
        nodes.append(n)
    return nodes


def child(n, name):
    return next((c for c in n[2] if c[0] == name), None)


def model_matrix(model) -> np.ndarray:
    """Lcl Translation / Rotation (XYZ Euler, degrees) / Scaling of a Model."""
    p70 = {p[1][0]: p[1][4:] for p in child(model, "Properties70")[2]}
    t = np.array(p70.get(b"Lcl Translation", [0, 0, 0]), np.float64)
    rx, ry, rz = np.radians(p70.get(b"Lcl Rotation", [0, 0, 0]))
    s = np.array(p70.get(b"Lcl Scaling", [1, 1, 1]), np.float64)
    cx, sx, cy, sy, cz, sz = np.cos(rx), np.sin(rx), np.cos(ry), np.sin(ry), np.cos(rz), np.sin(rz)
    x = np.array([[1, 0, 0], [0, cx, -sx], [0, sx, cx]])
    y = np.array([[cy, 0, sy], [0, 1, 0], [-sy, 0, cy]])
    z = np.array([[cz, -sz, 0], [sz, cz, 0], [0, 0, 1]])
    m = np.eye(4)
    m[:3, :3] = (z @ y @ x) * s
    m[:3, 3] = t
    return m


def layer_values(geometry, layer: str, values: str, index: str, corners: np.ndarray, width: int):
    """Per polygon corner values of a layer element (normals, UVs)."""
    element = child(geometry, layer)
    data = child(element, values)[1][0].reshape(-1, width)
    mapping = child(element, "MappingInformationType")[1][0]
    reference = child(element, "ReferenceInformationType")[1][0]
    if mapping == b"ByPolygonVertex":
        keys = np.arange(len(corners))
    elif mapping in (b"ByVertice", b"ByVertex"):
        keys = corners
    else:
        raise SystemExit(f"Unsupported {layer} mapping {mapping!r}.")
    if reference == b"IndexToDirect":
        keys = child(element, index)[1][0][keys]
    return data[keys]


def gather(path: pathlib.Path):
    """Triangles of the model's mesh, one vertex per corner, in +Y-up space:
    positions, normals, UVs."""
    nodes = read_fbx(path)
    objects = next(n for n in nodes if n[0] == "Objects")
    geometry = child(objects, "Geometry")
    model = child(objects, "Model")
    vertices = child(geometry, "Vertices")[1][0].reshape(-1, 3)
    polygon = child(geometry, "PolygonVertexIndex")[1][0]
    corners = np.where(polygon < 0, ~polygon, polygon)
    normals = layer_values(geometry, "LayerElementNormal", "Normals", "NormalsIndex", corners, 3)
    uvs = layer_values(geometry, "LayerElementUV", "UV", "UVIndex", corners, 2)
    # Fan-triangulate each polygon (they are triangles and quads).
    tris, start = [], 0
    for end in np.flatnonzero(polygon < 0):
        tris += [(start, k, k + 1) for k in range(start + 1, end)]
        start = end + 1
    tris = np.array(tris, np.int64)
    m = model_matrix(model) if model is not None else np.eye(4)
    p = (np.c_[vertices[corners], np.ones(len(corners))] @ m.T)[:, :3]
    n = normals @ np.linalg.inv(m[:3, :3])
    n /= np.linalg.norm(n, axis=1, keepdims=True)
    return p, n, uvs, tris


# --- Mesh ----------------------------------------------------------------------


def merge(positions, normals, uvs, triangles):
    """Share corners with the same position, normal and UV."""
    keys = np.c_[np.round(positions, 5), np.round(normals, 3), np.round(uvs, 5)]
    _, first, inverse = np.unique(keys, axis=0, return_index=True, return_inverse=True)
    inverse = inverse.reshape(-1)
    return positions[first], normals[first], uvs[first], inverse[triangles]


def subdivide(positions, normals, uvs, triangles):
    """Split every triangle into four at its edge midpoints."""
    edges = np.sort(np.r_[triangles[:, [0, 1]], triangles[:, [1, 2]], triangles[:, [2, 0]]], axis=1)
    unique, inverse = np.unique(edges, axis=0, return_inverse=True)
    inverse = inverse.reshape(-1)
    mid = len(positions) + inverse.reshape(3, -1).T  # (ab, bc, ca) per triangle
    a, b = unique[:, 0], unique[:, 1]
    positions = np.r_[positions, (positions[a] + positions[b]) / 2]
    n = normals[a] + normals[b]
    normals = np.r_[normals, n / np.maximum(np.linalg.norm(n, axis=1, keepdims=True), 1e-9)]
    uvs = np.r_[uvs, (uvs[a] + uvs[b]) / 2]
    t0, t1, t2 = triangles.T
    ab, bc, ca = mid.T
    triangles = np.c_[t0, ab, ca, ab, t1, bc, ca, bc, t2, ab, bc, ca].reshape(-1, 3)
    return positions, normals, uvs, triangles


def bake(uvs, image: np.ndarray) -> np.ndarray:
    """Linear RGB of the texture around each vertex's UV (FBX: V up)."""
    h, w = image.shape[:2]
    uv = uvs - np.floor(uvs)
    x = (uv[:, 0] * w).astype(int)
    y = ((1.0 - uv[:, 1]) * h).astype(int)
    r = max(1, w // 1024)
    acc = np.zeros((len(uvs), 3))
    for dy in (-r, 0, r):
        for dx in (-r, 0, r):
            acc += srgb_to_linear(image[np.clip(y + dy, 0, h - 1), np.clip(x + dx, 0, w - 1)] / 255.0)
    return acc / 9.0


def hand_space(forward, up, mirror: bool) -> np.ndarray:
    """Rows: the model directions that become +X, +Y and +Z."""
    z = -forward / np.linalg.norm(forward)
    y = up - z * (up @ z)
    y /= np.linalg.norm(y)
    x = np.cross(y, z)
    return np.array([-x if mirror else x, y, z])


def pieces(positions, triangles):
    """Triangle indices of each connected piece (corners at the same
    position are one), largest first."""
    _, ids = np.unique(np.round(positions, 4), axis=0, return_inverse=True)
    parent = np.arange(ids.max() + 1)

    def root(i):
        while parent[i] != i:
            parent[i] = parent[parent[i]]
            i = parent[i]
        return i

    for a, b, c in ids.reshape(-1)[triangles]:
        for u, v in ((a, b), (b, c)):
            ru, rv = root(u), root(v)
            if ru != rv:
                parent[ru] = rv
    piece = np.array([root(i) for i in ids.reshape(-1)[triangles[:, 0]]])
    found = [np.flatnonzero(piece == r) for r in np.unique(piece)]
    return sorted(found, key=len, reverse=True)


def compact(positions, normals, colours, triangles):
    used, inverse = np.unique(triangles, return_inverse=True)
    return positions[used], normals[used], colours[used], inverse.reshape(-1, 3)


def vector(text: str) -> np.ndarray:
    return np.array([float(v) for v in text.split(",")], np.float64)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("model", type=pathlib.Path)
    ap.add_argument("name", help="saved as assets/hands/<name>.glb")
    ap.add_argument("--forward", type=vector, required=True, help="where the fingers point, X,Y,Z")
    ap.add_argument("--up", type=vector, required=True, help="where the back of the hand faces, X,Y,Z")
    ap.add_argument("--mirror", action="store_true", help="the model is a left hand")
    ap.add_argument("--length", type=float, default=0.3, help="metres from fingertips to sleeve end")
    ap.add_argument("--texture", type=pathlib.Path, help="base colour (default: the PNG next to the model)")
    ap.add_argument("--triangles", type=int, default=4000,
                    help="subdivide up to about this many (finer colours, but more to draw)")
    args = ap.parse_args()

    texture = args.texture or next(args.model.parent.glob("*.png"), None)
    if texture is None:
        raise SystemExit("No texture found next to the model; pass --texture.")
    image = np.asarray(Image.open(texture).convert("RGB"))

    p, n, uv, t = merge(*gather(args.model))
    before = len(t)
    parts = pieces(p, t)
    for k, part in enumerate(parts):
        q = p[np.unique(t[part])]
        centre = q.mean(0)
        axis = np.linalg.svd(q - centre, full_matrices=False)[2][0]
        print(f"piece {k}: {len(part)} triangles, centre {centre.round(2)}, long axis {axis.round(3)}")

    m = hand_space(args.forward, args.up, args.mirror)
    p, n = p @ m.T, n @ m.T
    if args.mirror:
        t = t[:, ::-1]
    main_part = p[np.unique(t[parts[0]])]
    low, high = main_part.min(0), main_part.max(0)
    p = (p - (low + high) / 2) * (args.length / (high[2] - low[2]))

    while len(t) * 4 <= args.triangles * 1.5:
        # Subdividing keeps each piece's triangles together (4 in a row).
        parts = [np.ravel(4 * part[:, None] + np.arange(4)) for part in parts]
        p, n, uv, t = subdivide(p, n, uv, t)
    c = bake(uv, image)
    meshes = [("hand", *compact(p, n, c, t[parts[0]]))]
    if len(parts) > 1:
        meshes.append(("stick", *compact(p, n, c, t[np.concatenate(parts[1:])])))
    out = pathlib.Path(__file__).resolve().parent.parent / "assets" / "hands" / f"{args.name}.glb"
    out.parent.mkdir(parents=True, exist_ok=True)
    write_glb_meshes(out, meshes)
    print(f"{before} -> {len(t)} triangles: {out} ({out.stat().st_size // 1024} KB)")


if __name__ == "__main__":
    main()
