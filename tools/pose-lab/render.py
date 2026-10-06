"""Filmstrips and GIFs of pose_lab output: the skinned figure, its skeleton and the scene.

python tools/pose-lab/render.py lab.json --scene validator --figure adult --times 0,0.5,1 --view side --out strip.png
python tools/pose-lab/render.py lab.json --scene validator --figure adult --gif out.gif --view 3q
"""

import argparse
import json
import math

from PIL import Image, ImageDraw, ImageFont

PART = {
    0: (52, 73, 110), 1: (52, 73, 110), 2: (52, 73, 110), 3: (52, 73, 110),
    4: (120, 125, 135), 5: (120, 125, 135), 6: (120, 125, 135), 7: (120, 125, 135),
    8: (60, 80, 120), 9: (130, 135, 145), 10: (222, 180, 150),
    11: (222, 180, 150), 12: (222, 180, 150),
    13: (40, 40, 45), 14: (40, 40, 45), 15: (40, 40, 45), 16: (40, 40, 45),
}

BONES = [(0, 1), (1, 2), (2, 3)]
for s in (4, 14):
    BONES += [(0, s), (s, s + 1), (s + 1, s + 2), (s + 2, s + 3), (s + 3, s + 4), (s + 4, s + 5),
              (s + 2, s + 4), (2, s + 6), (s + 6, s + 7), (s + 7, s + 8), (s + 8, s + 9)]


def hexcol(h):
    h = h.lstrip("#")
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def place(o, h, p):
    r = math.radians(h)
    s, c = math.sin(r), math.cos(r)
    x, y, z = p
    return (o[0] + x * c + y * s, o[1] - x * s + y * c, o[2] + z)


def skin(mesh, b):
    pos, sk = mesh["pos"], mesh["skin"]
    out = []
    for i in range(len(pos) // 3):
        x, y, z = pos[3 * i], pos[3 * i + 1], pos[3 * i + 2]
        ax = ay = az = 0.0
        for k in range(4):
            w = sk[8 * i + 2 * k + 1]
            if w == 0:
                continue
            m = 12 * int(sk[8 * i + 2 * k])
            ax += w * (b[m] * x + b[m + 3] * y + b[m + 6] * z + b[m + 9])
            ay += w * (b[m + 1] * x + b[m + 4] * y + b[m + 7] * z + b[m + 10])
            az += w * (b[m + 2] * x + b[m + 5] * y + b[m + 8] * z + b[m + 11])
        out.append((ax, ay, az))
    return out


class Cam:
    def __init__(self, eye, target, fov, w, h, ortho=None):
        self.eye, self.w, self.h, self.ortho = eye, w, h, ortho
        f = norm(sub(target, eye))
        r = norm(cross(f, (0, 0, 1)))
        u = cross(r, f)
        self.f, self.r, self.u = f, r, u
        self.k = (h / 2) / math.tan(math.radians(fov) / 2)

    def proj(self, p):
        d = sub(p, self.eye)
        x, y, z = dot(d, self.r), dot(d, self.u), dot(d, self.f)
        if self.ortho:
            s = self.h / self.ortho
            return (self.w / 2 + x * s, self.h / 2 - y * s, z)
        z = max(z, 0.05)
        return (self.w / 2 + x * self.k / z, self.h / 2 - y * self.k / z, z)


def sub(a, b):
    return (a[0] - b[0], a[1] - b[1], a[2] - b[2])


def dot(a, b):
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]


def cross(a, b):
    return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])


def norm(a):
    l = math.sqrt(dot(a, a)) or 1.0
    return (a[0] / l, a[1] / l, a[2] / l)


LIGHT = norm((0.4, -0.6, 0.8))


def shade(col, n, cam):
    l = max(0.0, dot(n, LIGHT)) * 0.65 + 0.35
    return tuple(min(255, int(c * l)) for c in col)


def box_faces(b):
    x0, y0, z0, x1, y1, z1, col = b
    v = [(x, y, z) for x in (x0, x1) for y in (y0, y1) for z in (z0, z1)]
    quads = [(0, 1, 3, 2), (4, 6, 7, 5), (0, 4, 5, 1), (2, 3, 7, 6), (0, 2, 6, 4), (1, 5, 7, 3)]
    return [([v[i] for i in q], hexcol(col)) for q in quads]


def render(data, scene, fr, cam, skeleton=True, mesh=True, label=None, ghost=None, boxes=True):
    img = Image.new("RGB", (cam.w, cam.h), (246, 247, 249))
    d = ImageDraw.Draw(img)
    polys = []
    for b in scene["boxes"] if boxes else []:
        for q, col in box_faces(b):
            n = norm(cross(sub(q[1], q[0]), sub(q[2], q[0])))
            if dot(n, sub(q[0], cam.eye)) > 0 and not cam.ortho:
                continue
            if cam.ortho and dot(n, cam.f) > 0:
                continue
            pts = [cam.proj(p) for p in q]
            polys.append((sum(p[2] for p in pts) / 4, [(p[0], p[1]) for p in pts], shade(col, n, cam), (110, 110, 115)))
    fig = data["figures"][scene["figure"]]
    o, h, b = fr["o"], fr["h"], fr["b"]
    if mesh:
        for m in fig["meshes"]:
            sk = skin(m, b)
            world = [place(o, h, p) for p in sk]
            pr = [cam.proj(p) for p in world]
            idx = m["idx"]
            skw = m["skin"]
            for t in range(0, len(idx) - 2, 3):
                a, bb, c = idx[t], idx[t + 1], idx[t + 2]
                n = cross(sub(world[bb], world[a]), sub(world[c], world[a]))
                ln = math.sqrt(dot(n, n))
                if ln < 1e-12:
                    continue
                n = (n[0] / ln, n[1] / ln, n[2] / ln)
                facing = dot(n, cam.f) if cam.ortho else dot(n, sub(world[a], cam.eye))
                if facing > 0:
                    n = (-n[0], -n[1], -n[2])
                slot = int(skw[8 * a])
                col = PART.get(slot, (150, 150, 150))
                z = (pr[a][2] + pr[bb][2] + pr[c][2]) / 3
                polys.append((z, [pr[a][:2], pr[bb][:2], pr[c][:2]], shade(col, n, cam), None))
    polys.sort(key=lambda p: -p[0])
    for _, pts, col, outline in polys:
        d.polygon(pts, fill=col, outline=outline)
    if skeleton:
        j = fr["j"]
        js = [place(o, h, (j[3 * i], j[3 * i + 1], j[3 * i + 2])) for i in range(len(j) // 3)]
        pj = [cam.proj(p) for p in js]
        for a, bb in BONES:
            if any(4 <= x < 14 for x in (a, bb)):
                col = (40, 110, 230)
            elif any(x >= 14 for x in (a, bb)):
                col = (230, 60, 50)
            else:
                col = (30, 160, 80)
            d.line([pj[a][:2], pj[bb][:2]], fill=col, width=3)
        for p in pj:
            d.ellipse([p[0] - 3, p[1] - 3, p[0] + 3, p[1] + 3], fill=(20, 20, 20))
    if ghost:
        for path, col in ghost:
            pts = [cam.proj(p)[:2] for p in path]
            if len(pts) > 1:
                d.line(pts, fill=col, width=2)
    if label:
        d.rectangle([0, 0, cam.w, 22], fill=(255, 255, 255))
        d.text((6, 4), label, fill=(20, 20, 20), font=FONT)
    return img


try:
    FONT = ImageFont.truetype("arial.ttf", 14)
except OSError:
    FONT = ImageFont.load_default()


def make_cam(view, fr, w, h, zoom, cz=0.9, yaw=0.0):
    o = fr["o"]
    hd = math.radians(fr["h"] + yaw)
    fwd = (math.sin(hd), math.cos(hd), 0)
    right = (math.cos(hd), -math.sin(hd), 0)
    c = (o[0], o[1], o[2] + cz)
    views = {
        "side": (right, 1.0), "left": ((-right[0], -right[1], 0), 1.0),
        "front": (fwd, 1.0), "back": ((-fwd[0], -fwd[1], 0), 1.0),
        "3q": (norm((fwd[0] + right[0] * 1.2, fwd[1] + right[1] * 1.2, 0.35)), 0),
        "3qback": (norm((-fwd[0] + right[0] * 1.2, -fwd[1] + right[1] * 1.2, 0.45)), 0),
    }
    axis, ortho = views[view]
    if ortho:
        eye = (c[0] + axis[0] * 6, c[1] + axis[1] * 6, c[2])
        return Cam(eye, c, 30, w, h, ortho=2.3 / zoom)
    eye = (c[0] + axis[0] * 3.2 / zoom, c[1] + axis[1] * 3.2 / zoom, c[2] + axis[2] * 3.2 / zoom)
    return Cam(eye, c, 40, w, h)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("json")
    ap.add_argument("--scene", required=True)
    ap.add_argument("--figure", default="adult")
    ap.add_argument("--times")
    ap.add_argument("--every", type=float)
    ap.add_argument("--view", default="side")
    ap.add_argument("--fixed", action="store_true", help="camera fixed at the first frame")
    ap.add_argument("--zoom", type=float, default=1.0)
    ap.add_argument("--cz", type=float, default=0.9)
    ap.add_argument("--yaw", type=float, default=0.0)
    ap.add_argument("--size", default="300x420")
    ap.add_argument("--cols", type=int, default=8)
    ap.add_argument("--out")
    ap.add_argument("--gif")
    ap.add_argument("--from", dest="t0", type=float, default=0.0)
    ap.add_argument("--to", dest="t1", type=float, default=1e9)
    ap.add_argument("--noskin", action="store_true")
    ap.add_argument("--noboxes", action="store_true")
    ap.add_argument("--eye", help="fixed camera: x,y,z in the scene frame")
    ap.add_argument("--at", help="what the fixed camera looks at")
    ap.add_argument("--ortho", type=float, help="orthographic, this many metres high")
    a = ap.parse_args()
    data = json.load(open(a.json))
    sc = [s for s in data["scenes"] if s["figure"] == a.figure and s["title"].startswith(a.scene)][0]
    frames = [f for f in sc["frames"] if a.t0 <= f["t"] <= a.t1]
    w, h = map(int, a.size.split("x"))
    if a.times:
        ts = [float(t) for t in a.times.split(",")]
    elif a.every:
        ts = [frames[0]["t"] + k * a.every for k in range(int((frames[-1]["t"] - frames[0]["t"]) / a.every) + 1)]
    else:
        ts = [f["t"] for f in frames]
    pick = [min(frames, key=lambda f: abs(f["t"] - t)) for t in ts]
    anchor = frames[0]
    tiles = []
    for fr in pick:
        if a.eye:
            eye = tuple(map(float, a.eye.split(",")))
            at = tuple(map(float, a.at.split(",")))
            cam = Cam(eye, at, 40 / a.zoom, w, h, ortho=a.ortho)
        else:
            cam = make_cam(a.view, anchor if a.fixed else fr, w, h, a.zoom, a.cz, a.yaw)
        tiles.append(render(data, sc, fr, cam, mesh=not a.noskin, label=f"{fr['t']:.2f}s {fr['n']}", boxes=not a.noboxes))
    if a.gif:
        tiles[0].save(a.gif, save_all=True, append_images=tiles[1:], duration=int(1000 * (ts[1] - ts[0]) if len(ts) > 1 else 33), loop=0)
    if a.out:
        cols = min(a.cols, len(tiles))
        rows = (len(tiles) + cols - 1) // cols
        sheet = Image.new("RGB", (cols * w, rows * h), (255, 255, 255))
        for k, t in enumerate(tiles):
            sheet.paste(t, ((k % cols) * w, (k // cols) * h))
        sheet.save(a.out)


if __name__ == "__main__":
    main()
