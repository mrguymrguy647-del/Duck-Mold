"""Writes a small desk lamp as a .gltf by hand, without Duck Mold.

glTF directions: y up, the model faces +z, x is the model's left.
Four meshes, each laid out a different way, so that Duck Mold's import is
tested on all of them: a box with 16-bit corner numbers, a box with 8-bit
ones, a lamp shade as a triangle fan (open at the bottom), and a bulb with
no corner numbers at all. One object is placed by a matrix, the others by
translation, rotation and scale; the shade and bulb sit inside an arm that
is tilted so the shade opens to the front.
"""
import base64, json, math, struct, sys

buffer = bytearray()
views, accessors = [], []

def pad():
    while len(buffer) % 4:
        buffer.append(0)

def add_points(points):
    pad()
    start = len(buffer)
    for p in points:
        buffer.extend(struct.pack('<3f', *p))
    views.append({"buffer": 0, "byteOffset": start, "byteLength": len(buffer) - start, "target": 34962})
    lo = [min(p[i] for p in points) for i in range(3)]
    hi = [max(p[i] for p in points) for i in range(3)]
    # min and max as the 32-bit numbers stored
    lo = [struct.unpack('<f', struct.pack('<f', v))[0] for v in lo]
    hi = [struct.unpack('<f', struct.pack('<f', v))[0] for v in hi]
    accessors.append({"bufferView": len(views) - 1, "componentType": 5126, "count": len(points),
                      "type": "VEC3", "min": lo, "max": hi})
    return len(accessors) - 1

def add_indices(indices, kind):
    pad()
    start = len(buffer)
    fmt, comp = {'u8': ('<B', 5121), 'u16': ('<H', 5123)}[kind]
    for i in indices:
        buffer.extend(struct.pack(fmt, i))
    views.append({"buffer": 0, "byteOffset": start, "byteLength": len(buffer) - start, "target": 34963})
    accessors.append({"bufferView": len(views) - 1, "componentType": comp, "count": len(indices), "type": "SCALAR"})
    return len(accessors) - 1

def box(w, h, d):
    x, y, z = w / 2, h / 2, d / 2
    pts = [(-x, -y, -z), (x, -y, -z), (x, y, -z), (-x, y, -z),
           (-x, -y, z), (x, -y, z), (x, y, z), (-x, y, z)]
    # counter-clockwise seen from outside (glTF's front)
    quads = [(4, 5, 6, 7), (1, 0, 3, 2), (0, 4, 7, 3), (5, 1, 2, 6), (3, 7, 6, 2), (0, 1, 5, 4)]
    idx = []
    for a, b, c, dd in quads:
        idx += [a, b, c, a, c, dd]
    return pts, idx

def linear(c):
    c = c / 255
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4

def colour(hexcode):
    r, g, b = (int(hexcode[i:i + 2], 16) for i in (1, 3, 5))
    return [round(linear(r), 6), round(linear(g), 6), round(linear(b), 6), 1.0]

materials = [
    {"name": "stand", "pbrMetallicRoughness": {"baseColorFactor": colour("#3A3D44"), "metallicFactor": 0.0}},
    {"name": "shade", "pbrMetallicRoughness": {"baseColorFactor": colour("#2E7D6B"), "metallicFactor": 0.0}},
    {"name": "bulb", "pbrMetallicRoughness": {"baseColorFactor": colour("#FFE9A8"), "metallicFactor": 0.0}},
]

meshes = []
# The base: a flat box, 16-bit corner numbers.
pts, idx = box(0.22, 0.03, 0.22)
meshes.append({"name": "base", "primitives": [{"attributes": {"POSITION": add_points(pts)},
              "indices": add_indices(idx, 'u16'), "material": 0}]})
# The pole: a unit box, stretched by its object's matrix; 8-bit numbers.
pts, idx = box(1.0, 1.0, 1.0)
meshes.append({"name": "pole", "primitives": [{"attributes": {"POSITION": add_points(pts)},
              "indices": add_indices(idx, 'u8'), "material": 0}]})
# The shade: a cone open at the bottom, as a fan around its tip.
n = 10
ring = [(0.12 * math.cos(2 * math.pi * k / n), -0.07, 0.12 * math.sin(2 * math.pi * k / n)) for k in range(n)]
pts = [(0.0, 0.07, 0.0)] + ring
# A fan: triangle k is (k+1, k+2, 0); going round the ring clockwise seen
# from above, with the tip last, faces outwards.
fan = [1 + ((n - k) % n) for k in range(n + 1)]
meshes.append({"name": "shade", "primitives": [{"attributes": {"POSITION": add_points(pts)},
              "indices": add_indices([0] + fan, 'u8'), "mode": 6, "material": 1}]})
# The bulb: an eight-sided ball, every triangle with corners of its own.
r = 0.035
top, bottom = (0, r, 0), (0, -r, 0)
around = [(r, 0, 0), (0, 0, r), (-r, 0, 0), (0, 0, -r)]
tris = []
for k in range(4):
    a, b = around[k], around[(k + 1) % 4]
    tris += [top, b, a, bottom, a, b]
meshes.append({"name": "bulb", "primitives": [{"attributes": {"POSITION": add_points(tris)}, "material": 2}]})

tilt = math.radians(35)  # around x: the arm leans back, so the shade opens forward
nodes = [
    {"name": "lamp", "children": [1, 2, 3]},
    {"name": "base", "mesh": 0, "translation": [0, 0.015, 0]},
    # Column by column: x 0.025, y 0.42, z 0.025, moved up 0.24.
    {"name": "pole", "mesh": 1, "matrix": [0.025, 0, 0, 0, 0, 0.42, 0, 0, 0, 0, 0.025, 0, 0, 0.24, 0, 1]},
    {"name": "arm", "translation": [0, 0.45, 0],
     "rotation": [math.sin(-tilt / 2), 0, 0, math.cos(-tilt / 2)], "children": [4, 5]},
    {"name": "shade", "mesh": 2, "translation": [0, 0.0, 0.11]},
    {"name": "bulb", "mesh": 3, "translation": [0, -0.03, 0.11], "scale": [1, 1.2, 1]},
]
pad()
gltf = {
    "asset": {"version": "2.0", "generator": "written by hand for Duck Mold's import example"},
    "scene": 0,
    "scenes": [{"name": "lamp", "nodes": [0]}],
    "nodes": nodes,
    "meshes": meshes,
    "materials": materials,
    "accessors": accessors,
    "bufferViews": views,
    "buffers": [{"byteLength": len(buffer),
                 "uri": "data:application/octet-stream;base64," + base64.b64encode(bytes(buffer)).decode()}],
}
json.dump(gltf, open(sys.argv[1], 'w'), indent=1)
open(sys.argv[1], 'a').write('\n')
