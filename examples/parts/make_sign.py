"""Writes a small signpost as a .gltf by hand, without Duck Mold, with
pictures on it: Duck Mold's test of models that bring their own pictures.

Three materials, each with a picture kept a different way:

- "post": a PNG beside the file (sign_post.png), repeated seven and a
  half times up the post;
- "board": a PNG inside the file's block of numbers (sign_board.png is
  read and put in);
- "cap": a JPEG written inside the file as base64 text (sign_cap.jpg is
  read and put in).

The two PNGs are drawn by this script, pixel by pixel. The JPEG is made
with the Pillow library when it is there (`--jpeg`); without that
switch the sign_cap.jpg already in the folder is used, so that the file
comes out the same every time.

    python3 make_sign.py sign.gltf

glTF directions: y up, the model faces +z, x is the model's left. A
place on a picture is counted across from its left and down from its
top.
"""
import base64, json, os, struct, sys, zlib

here = os.path.dirname(os.path.abspath(__file__))

# ---- the pictures ------------------------------------------------------------

def png(width, height, pixels):
    """A PNG file's bytes from rows of (r, g, b) pixels."""
    raw = b''.join(b'\x00' + bytes(c for pixel in row for c in pixel) for row in pixels)
    def chunk(kind, data):
        body = kind + data
        return struct.pack('>I', len(data)) + body + struct.pack('>I', zlib.crc32(body))
    return (b'\x89PNG\r\n\x1a\n'
            + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(raw, 9))
            + chunk(b'IEND', b''))

def hexed(code):
    return tuple(int(code[i:i + 2], 16) for i in (1, 3, 5))

WOOD, GRAIN, DARK = hexed('#8A5A2B'), hexed('#7A4E24'), hexed('#5E3A18')
BOARD, EDGE, PAINT = hexed('#C8A064'), hexed('#9A7440'), hexed('#F4EBD0')

# Everything is 100 pixels to the metre, so the pixels are one size all
# over the sign.

# The post: 8 x 16, wood with grain running up it, and a knot.
post = [[WOOD] * 8 for _ in range(16)]
for y in range(16):
    post[y][2] = GRAIN
    post[y][5] = GRAIN if y % 8 else DARK
post[6][6] = post[7][6] = DARK
open(os.path.join(here, 'sign_post.png'), 'wb').write(png(8, 16, post))

# The board: 64 x 32, a plank with a darker edge and an arrow to the
# picture's right.
board = [[BOARD] * 64 for _ in range(32)]
for x in range(64):
    board[0][x] = board[31][x] = EDGE
for y in range(32):
    board[y][0] = board[y][63] = EDGE
for y in (5, 26):
    for x in range(4, 60, 9):
        board[y][x] = board[y][x + 1] = EDGE      # a few marks of grain
for x in range(12, 42):                           # the arrow's shaft
    for y in range(14, 18):
        board[y][x] = PAINT
for step in range(10):                            # and its head
    for y in range(6 + step, 26 - step):
        board[y][42 + step] = PAINT
open(os.path.join(here, 'sign_board.png'), 'wb').write(png(64, 32, board))

# The cap: a JPEG, as Blender would keep one.
cap_path = os.path.join(here, 'sign_cap.jpg')
if '--jpeg' in sys.argv:
    from PIL import Image
    picture = Image.new('RGB', (16, 8))
    for y in range(8):
        for x in range(16):
            # Green copper, lighter towards the top, with streaks.
            shade = 150 - 9 * y + (14 if x % 5 == 0 else 0)
            picture.putpixel((x, y), (shade // 3, shade, shade * 2 // 3 + 20))
    picture.save(cap_path, quality=92, subsampling=0, optimize=False)
cap_jpeg = open(cap_path, 'rb').read()

# ---- the shapes --------------------------------------------------------------

buffer = bytearray()
views, accessors = [], []

def pad():
    while len(buffer) % 4:
        buffer.append(0)

def add(numbers, per, kind, component, fmt, bounds=False):
    pad()
    start = len(buffer)
    for item in numbers:
        buffer.extend(struct.pack(fmt, *(item if per > 1 else (item,))))
    views.append({"buffer": 0, "byteOffset": start, "byteLength": len(buffer) - start})
    accessor = {"bufferView": len(views) - 1, "componentType": component,
                "count": len(numbers), "type": kind}
    if bounds:
        as_stored = lambda v: struct.unpack('<f', struct.pack('<f', v))[0]
        accessor["min"] = [as_stored(min(p[i] for p in numbers)) for i in range(per)]
        accessor["max"] = [as_stored(max(p[i] for p in numbers)) for i in range(per)]
    accessors.append(accessor)
    return len(accessors) - 1

def box(w, h, d, sides, top):
    """A box with corners of its own for every face, so that every face
    has its own place on the picture. `sides` gives each of the four
    sides' places as (left, top, right, bottom) for front, back, the
    model's left and the model's right; `top` the same for top and
    bottom."""
    x, y, z = w / 2, h / 2, d / 2
    # Each face: its four corners counter-clockwise seen from outside,
    # starting at the bottom left as you look at it.
    faces = [
        ([(-x, -y, z), (x, -y, z), (x, y, z), (-x, y, z)], sides[0]),      # front (+z)
        ([(x, -y, -z), (-x, -y, -z), (-x, y, -z), (x, y, -z)], sides[1]),  # back
        ([(x, -y, z), (x, -y, -z), (x, y, -z), (x, y, z)], sides[2]),      # the model's left (+x)
        ([(-x, -y, -z), (-x, -y, z), (-x, y, z), (-x, y, -z)], sides[3]),  # the model's right
        ([(-x, y, z), (x, y, z), (x, y, -z), (-x, y, -z)], top),           # top
        ([(-x, -y, -z), (x, -y, -z), (x, -y, z), (-x, -y, z)], top),       # bottom
    ]
    points, places, corners = [], [], []
    for quad, (left, up, right, down) in faces:
        first = len(points)
        points += quad
        places += [(left, down), (right, down), (right, up), (left, up)]
        corners += [first, first + 1, first + 2, first, first + 2, first + 3]
    return points, places, corners

def primitive(points, places, corners, material):
    return {"attributes": {"POSITION": add(points, 3, "VEC3", 5126, '<3f', bounds=True),
                           "TEXCOORD_0": add(places, 2, "VEC2", 5126, '<2f')},
            "indices": add(corners, 1, "SCALAR", 5123, '<H'), "material": material}

meshes = []
# The post: 8 cm square, 1.2 m high. Its picture goes once across each
# side and seven and a half times up it.
whole = (0.0, 0.0, 1.0, 1.0)
up_the_post = (0.0, 0.0, 1.0, 7.5)
meshes.append({"name": "post", "primitives": [primitive(*box(0.08, 1.2, 0.08, [up_the_post] * 4, whole), 0)]})
# The board: the whole picture on its front, the same turned round on
# its back (so the arrow points the same way from both sides), and the
# picture's edge colour on its thin sides.
thin = (0.0, 0.0, 1 / 64, 1.0)
flat = (0.0, 0.0, 1.0, 1 / 32)
meshes.append({"name": "board", "primitives": [primitive(*box(0.64, 0.32, 0.03, [whole, (1.0, 0.0, 0.0, 1.0), thin, thin], flat), 1)]})
# The cap: a low pyramid on the post, each side showing the JPEG.
r, h = 0.08, 0.06
tip = (0.0, h, 0.0)
ring = [(-r, 0.0, r), (r, 0.0, r), (r, 0.0, -r), (-r, 0.0, -r)]
points, places = [], []
for k in range(4):
    points += [ring[k], ring[(k + 1) % 4], tip]
    places += [(0.0, 1.0), (1.0, 1.0), (0.5, 0.0)]
meshes.append({"name": "cap", "primitives": [{
    "attributes": {"POSITION": add(points, 3, "VEC3", 5126, '<3f', bounds=True),
                   "TEXCOORD_0": add(places, 2, "VEC2", 5126, '<2f')},
    "material": 2}]})

# The board's PNG goes into the block of numbers.
pad()
board_png = open(os.path.join(here, 'sign_board.png'), 'rb').read()
start = len(buffer)
buffer.extend(board_png)
views.append({"buffer": 0, "byteOffset": start, "byteLength": len(board_png)})
board_view = len(views) - 1
pad()

white = [1.0, 1.0, 1.0, 1.0]
gltf = {
    "asset": {"version": "2.0", "generator": "written by hand for Duck Mold's import example"},
    "scene": 0,
    "scenes": [{"name": "sign", "nodes": [0]}],
    "nodes": [
        {"name": "sign", "children": [1, 2, 3]},
        {"name": "post", "mesh": 0, "translation": [0, 0.6, 0]},
        {"name": "board", "mesh": 1, "translation": [0, 0.95, 0.055]},
        {"name": "cap", "mesh": 2, "translation": [0, 1.2, 0]},
    ],
    "meshes": meshes,
    "materials": [
        {"name": "post", "pbrMetallicRoughness": {"baseColorFactor": white, "baseColorTexture": {"index": 0}, "metallicFactor": 0.0}},
        {"name": "board", "pbrMetallicRoughness": {"baseColorFactor": white, "baseColorTexture": {"index": 1}, "metallicFactor": 0.0}},
        {"name": "cap", "pbrMetallicRoughness": {"baseColorFactor": white, "baseColorTexture": {"index": 2}, "metallicFactor": 0.0}},
    ],
    "textures": [{"source": 0, "sampler": 0}, {"source": 1, "sampler": 0}, {"source": 2, "sampler": 0}],
    # Sharp pixels, repeating.
    "samplers": [{"magFilter": 9728, "minFilter": 9728, "wrapS": 10497, "wrapT": 10497}],
    "images": [
        {"name": "post", "uri": "sign_post.png"},
        {"name": "board", "bufferView": board_view, "mimeType": "image/png"},
        {"name": "cap", "uri": "data:image/jpeg;base64," + base64.b64encode(cap_jpeg).decode()},
    ],
    "accessors": accessors,
    "bufferViews": views,
    "buffers": [{"byteLength": len(buffer),
                 "uri": "data:application/octet-stream;base64," + base64.b64encode(bytes(buffer)).decode()}],
}
json.dump(gltf, open(sys.argv[1], 'w'), indent=1)
open(sys.argv[1], 'a').write('\n')
