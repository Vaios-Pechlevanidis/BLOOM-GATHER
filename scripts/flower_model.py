#!/usr/bin/env python3
"""Turn a 3D flower model (.glb) into the game's format and save it as
assets/flowers/<species id>.glb, plus simpler versions of it for the beds
and the distance: <species id>_mid.glb (beds near the player) and
<species id>_low.glb (further away):

  * the colours of its base-colour texture baked into vertex colours (the
    game's flowers are vertex-coloured and share one material, so a whole
    meadow batches together; no textures to ship)
  * welded and simplified to a triangle budget a meadow of them can afford
  * smooth normals; positions, normals, colours and indices only

Usage: scripts/flower_model.py <model.glb> <species id> [--triangles 2500] [--mid-triangles 700] [--low-triangles 160]
Example: scripts/flower_model.py ~/Downloads/Cartoon_Daisy_Flower.glb daisy

The game measures the model itself (stem base on the ground, top at the
species' height), so its size and position do not matter; it must stand
upright (+Y up, as glTF is). Requires numpy, Pillow and fast_simplification
(`pip install numpy pillow fast_simplification`). Rebuild the game afterwards;
the models are embedded at build time.
"""

import argparse
import base64
import io
import json
import pathlib
import struct

import numpy as np
from PIL import Image

Image.MAX_IMAGE_PIXELS = None

COMPONENTS = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}
TYPES = {5120: np.int8, 5121: np.uint8, 5122: np.int16, 5123: np.uint16, 5125: np.uint32, 5126: np.float32}
NORMALIZE = {5121: 255.0, 5123: 65535.0, 5120: 127.0, 5122: 32767.0}


class Glb:
    def __init__(self, path: pathlib.Path):
        data = path.read_bytes()
        if data[:4] != b"glTF":
            raise SystemExit(f"{path} is not a binary glTF (.glb) file.")
        length, _ = struct.unpack("<II", data[12:20])
        self.json = json.loads(data[20 : 20 + length])
        rest = 20 + length
        self.blob = b""
        if rest + 8 <= len(data):
            blen, _ = struct.unpack("<II", data[rest : rest + 8])
            self.blob = data[rest + 8 : rest + 8 + blen]
        self.dir = path.parent

    def buffer(self, i: int) -> bytes:
        uri = self.json["buffers"][i].get("uri")
        if uri is None:
            return self.blob
        if uri.startswith("data:"):
            return base64.b64decode(uri.split(",", 1)[1])
        return (self.dir / uri).read_bytes()

    def view(self, i: int) -> bytes:
        v = self.json["bufferViews"][i]
        o = v.get("byteOffset", 0)
        return self.buffer(v["buffer"])[o : o + v["byteLength"]]

    def accessor(self, i: int) -> np.ndarray:
        a = self.json["accessors"][i]
        n = COMPONENTS[a["type"]]
        dtype = np.dtype(TYPES[a["componentType"]])
        view = self.json["bufferViews"][a["bufferView"]]
        raw = self.view(a["bufferView"])
        stride = view.get("byteStride", n * dtype.itemsize)
        o = a.get("byteOffset", 0)
        out = np.ndarray((a["count"], n), dtype, raw, o, (stride, dtype.itemsize)).copy()
        if a.get("normalized") and a["componentType"] in NORMALIZE:
            out = out.astype(np.float64) / NORMALIZE[a["componentType"]]
        return out if n > 1 else out[:, 0]

    def image(self, texture: int) -> np.ndarray:
        img = self.json["images"][self.json["textures"][texture]["source"]]
        if "bufferView" in img:
            raw = self.view(img["bufferView"])
        elif img["uri"].startswith("data:"):
            raw = base64.b64decode(img["uri"].split(",", 1)[1])
        else:
            raw = (self.dir / img["uri"]).read_bytes()
        return np.asarray(Image.open(io.BytesIO(raw)).convert("RGB"))


def node_matrix(node: dict) -> np.ndarray:
    if "matrix" in node:
        return np.array(node["matrix"], np.float64).reshape(4, 4).T
    t = np.array(node.get("translation", [0, 0, 0]), np.float64)
    x, y, z, w = node.get("rotation", [0, 0, 0, 1])
    s = np.array(node.get("scale", [1, 1, 1]), np.float64)
    r = np.array(
        [
            [1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
            [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
            [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)],
        ]
    )
    m = np.eye(4)
    m[:3, :3] = r * s
    m[:3, 3] = t
    return m


def srgb_to_linear(c: np.ndarray) -> np.ndarray:
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)


def primitive_colours(glb: Glb, prim: dict, images: dict) -> np.ndarray:
    """Linear RGB per vertex: COLOR_0 x base colour factor x texture."""
    attrs = prim["attributes"]
    n = glb.json["accessors"][attrs["POSITION"]]["count"]
    rgb = np.ones((n, 3))
    if "COLOR_0" in attrs:
        rgb *= glb.accessor(attrs["COLOR_0"])[:, :3]
    material = glb.json.get("materials", [{}])[prim["material"]] if "material" in prim else {}
    pbr = material.get("pbrMetallicRoughness", {})
    rgb *= np.array(pbr.get("baseColorFactor", [1, 1, 1, 1])[:3])
    tex = pbr.get("baseColorTexture")
    if tex is not None:
        uv = glb.accessor(attrs[f"TEXCOORD_{tex.get('texCoord', 0)}"]).astype(np.float64)
        if tex["index"] not in images:
            images[tex["index"]] = glb.image(tex["index"])
        img = images[tex["index"]]
        h, w = img.shape[:2]
        uv = uv - np.floor(uv)  # glTF textures repeat
        # A small box around each vertex's texel: smooths texture noise.
        r = max(1, w // 1024)
        x = (uv[:, 0] * w).astype(int)
        y = (uv[:, 1] * h).astype(int)
        acc = np.zeros((n, 3))
        for dy in (-r, 0, r):
            for dx in (-r, 0, r):
                acc += srgb_to_linear(img[np.clip(y + dy, 0, h - 1), np.clip(x + dx, 0, w - 1)] / 255.0)
        rgb *= acc / 9.0
    return rgb


def gather(glb: Glb):
    """All triangles of the default scene in world space, with colours."""
    positions, colours, triangles = [], [], []
    images: dict = {}
    count = 0
    scene = glb.json["scenes"][glb.json.get("scene", 0)]

    def visit(i: int, parent: np.ndarray):
        nonlocal count
        node = glb.json["nodes"][i]
        m = parent @ node_matrix(node)
        if "mesh" in node:
            for prim in glb.json["meshes"][node["mesh"]]["primitives"]:
                if prim.get("mode", 4) != 4:
                    continue
                p = glb.accessor(prim["attributes"]["POSITION"]).astype(np.float64)
                p = (np.c_[p, np.ones(len(p))] @ m.T)[:, :3]
                if "indices" in prim:
                    idx = glb.accessor(prim["indices"]).astype(np.int64).reshape(-1, 3)
                else:
                    idx = np.arange(len(p)).reshape(-1, 3)
                if np.linalg.det(m[:3, :3]) < 0:
                    idx = idx[:, ::-1]
                positions.append(p)
                colours.append(primitive_colours(glb, prim, images))
                triangles.append(idx + count)
                count += len(p)
        for c in node.get("children", []):
            visit(c, m)

    for i in scene["nodes"]:
        visit(i, np.eye(4))
    if not positions:
        raise SystemExit("The model has no triangles.")
    return np.vstack(positions), np.vstack(colours), np.vstack(triangles)


def weld(positions, colours, triangles):
    """One vertex per position (texture seams split vertices; the simplifier
    would tear the surface apart along them). Colours are averaged."""
    extent = np.ptp(positions, axis=0).max()
    keys = np.round(positions / (extent * 1e-5)).astype(np.int64)
    _, first, inverse = np.unique(keys, axis=0, return_index=True, return_inverse=True)
    inverse = inverse.reshape(-1)
    welded = positions[first]
    sums = np.zeros((len(first), 3))
    np.add.at(sums, inverse, colours)
    counts = np.bincount(inverse, minlength=len(first))[:, None]
    tris = inverse[triangles]
    keep = (tris[:, 0] != tris[:, 1]) & (tris[:, 1] != tris[:, 2]) & (tris[:, 0] != tris[:, 2])
    return welded, sums / counts, tris[keep]


def simplify(positions, colours, triangles, target: int):
    import fast_simplification

    if len(triangles) <= target:
        return positions, colours, triangles
    reduction = 1.0 - target / len(triangles)
    _, _, collapses = fast_simplification.simplify(
        positions, triangles, target_reduction=reduction, agg=7, return_collapses=True
    )
    points, faces, mapping = fast_simplification.replay_simplification(positions, triangles, collapses)
    sums = np.zeros((len(points), 3))
    np.add.at(sums, mapping, colours)
    counts = np.maximum(np.bincount(mapping, minlength=len(points)), 1)[:, None]
    return points, sums / counts, faces.astype(np.int64)


def smooth_normals(positions, triangles):
    a, b, c = (positions[triangles[:, k]] for k in range(3))
    face = np.cross(b - a, c - a)  # area weighted
    normals = np.zeros_like(positions)
    for k in range(3):
        np.add.at(normals, triangles[:, k], face)
    length = np.linalg.norm(normals, axis=1, keepdims=True)
    return normals / np.where(length > 0, length, 1)


def write_glb(path: pathlib.Path, positions, normals, colours, triangles):
    p = positions.astype(np.float32)
    n = normals.astype(np.float32)
    c = np.c_[colours, np.ones(len(colours))].astype(np.float32)
    i = triangles.astype(np.uint32).reshape(-1)
    chunks = [p.tobytes(), n.tobytes(), c.tobytes(), i.tobytes()]
    views, offset = [], 0
    for k, chunk in enumerate(chunks):
        view = {"buffer": 0, "byteOffset": offset, "byteLength": len(chunk)}
        view["target"] = 34963 if k == 3 else 34962
        views.append(view)
        offset += len(chunk)
    doc = {
        "asset": {"version": "2.0", "generator": "Bloom & Gather scripts/flower_model.py"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0}],
        "meshes": [{"primitives": [{"attributes": {"POSITION": 0, "NORMAL": 1, "COLOR_0": 2}, "indices": 3}]}],
        "accessors": [
            {"bufferView": 0, "componentType": 5126, "count": len(p), "type": "VEC3",
             "min": p.min(0).tolist(), "max": p.max(0).tolist()},
            {"bufferView": 1, "componentType": 5126, "count": len(n), "type": "VEC3"},
            {"bufferView": 2, "componentType": 5126, "count": len(c), "type": "VEC4"},
            {"bufferView": 3, "componentType": 5125, "count": len(i), "type": "SCALAR"},
        ],
        "bufferViews": views,
        "buffers": [{"byteLength": offset}],
    }
    js = json.dumps(doc, separators=(",", ":")).encode()
    js += b" " * (-len(js) % 4)
    blob = b"".join(chunks)
    blob += b"\0" * (-len(blob) % 4)
    total = 12 + 8 + len(js) + 8 + len(blob)
    path.write_bytes(
        struct.pack("<4sII", b"glTF", 2, total)
        + struct.pack("<I4s", len(js), b"JSON") + js
        + struct.pack("<I4s", len(blob), b"BIN\0") + blob
    )


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("model", type=pathlib.Path)
    ap.add_argument("species", help="species id from game_data.ron, e.g. daisy")
    ap.add_argument("--triangles", type=int, default=2500, help="triangle budget")
    ap.add_argument("--mid-triangles", type=int, default=700, help="budget for beds near the player")
    ap.add_argument("--low-triangles", type=int, default=160, help="budget for beds and flowers far away")
    args = ap.parse_args()

    positions, colours, triangles = gather(Glb(args.model))
    before = len(triangles)
    positions, colours, triangles = weld(positions, colours, triangles)
    folder = pathlib.Path(__file__).resolve().parent.parent / "assets" / "flowers"
    versions = [
        (args.species, args.triangles),
        (f"{args.species}_mid", args.mid_triangles),
        (f"{args.species}_low", args.low_triangles),
    ]
    for name, budget in versions:
        p, c, t = simplify(positions, colours, triangles, budget)
        out = folder / f"{name}.glb"
        write_glb(out, p, smooth_normals(p, t), c, t)
        print(f"{before} -> {len(t)} triangles, {len(p)} vertices: {out} ({out.stat().st_size // 1024} KB)")


if __name__ == "__main__":
    main()
