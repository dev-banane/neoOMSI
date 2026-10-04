"""python build.py --omsi "<OMSI 2 folder>" [--out <pack folder>] [--blender <blender.exe>] [--size 1024] [--jobs N]
"""

import argparse
import concurrent.futures
import io
import json
import os
import pathlib
import re
import shutil
import struct
import subprocess
import sys

from PIL import Image

HERE = pathlib.Path(__file__).resolve().parent
CACHE = HERE / ".cache"
# screen size (twice the person's height over the distance, in view heights) each
# level is drawn down to: about 14 m and 43 m at a 60 degree view
LOD_SIZES = [0.25, 0.08, 0.0]
BONES = ["OS_L", "OS_R", "US_L", "US_R", "OA_L", "OA_R", "UA_L", "UA_R",
         "Hip", "Main", "Head", "Hand_L", "Hand_R"]


def find_blender(given):
    if given:
        return given
    if os.environ.get("BLENDER"):
        return os.environ["BLENDER"]
    found = shutil.which("blender")
    if found:
        return found
    base = pathlib.Path(os.environ.get("ProgramFiles", "C:/Program Files")) / "Blender Foundation"
    for exe in sorted(base.glob("Blender */blender.exe"), reverse=True):
        return str(exe)
    sys.exit("Blender not found: pass --blender or set BLENDER")


def blocks(text):
    return [(i, line.strip().lower()) for i, line in enumerate(text.splitlines())
            if re.fullmatch(r"\[[^\]]+\]\s*", line)]


def values(lines, at, n):
    return [lines[at + 1 + k].strip() for k in range(n)]


def replace_values(lines, keyword, new):
    for i, k in blocks("\n".join(lines)):
        if k == keyword:
            for j, v in enumerate(new):
                lines[i + 1 + j] = v
            return
    lines.extend(["", keyword] + new)


def write_hum(stock_path, out_path, model, info, weight):
    text = stock_path.read_bytes().decode("cp1252")
    lines = text.splitlines()
    found = dict((k, i) for i, k in blocks(text))
    stock_links = [float(v) for v in values(lines, found["[links]"], 22)]
    seat = float(values(lines, found["[seatheight]"], 1)[0]) if "[seatheight]" in found else 0.0
    feet = values(lines, found["[humangeom]"], 1)[0] if "[humangeom]" in found else "0.04"
    links = info["links"]
    replace_values(lines, "[model]", [model])
    replace_values(lines, "[humangeom]", [feet, f"{info['height']:.2f}"])
    replace_values(lines, "[links]", [f"{v:.3f}" for v in links])
    if seat > 0.2:
        # keep the stock hip-above-seat lift so the new body sits as deep as the old one did
        replace_values(lines, "[seatheight]", [f"{links[2] - (stock_links[2] - seat):.2f}"])
    if weight != 1.0:
        replace_values(lines, "[neo_weight]", [f"{weight:g}"])
    out_path.write_bytes(("\r\n".join(lines) + "\r\n").encode("cp1252"))


def write_cfg(path, meshes, textures, alpha):
    out = []
    for size, mesh in zip(LOD_SIZES, meshes):
        out += ["[LOD]", str(size), "", "[mesh]", mesh, ""]
        for b in BONES:
            out += ["[setbone]", b, str(-2 - BONES.index(b)), ""]
        for t in textures:
            out += ["[matl]", t, "0"]
            if t in alpha:
                out += ["[matl_alpha]", "1"]
            out.append("")
    path.write_bytes(("\r\n".join(out) + "\r\n").encode("cp1252"))


def write_dds(img, dst, pixel_format):
    """Block-compressed with the whole mip chain: the engine uploads such a file as it is."""
    block = 8 if pixel_format == "DXT1" else 16
    data, header, levels = [], None, 0
    while True:
        buf = io.BytesIO()
        img.save(buf, format="DDS", pixel_format=pixel_format)
        raw = buf.getvalue()
        header = header or bytearray(raw[:128])
        want = max(1, (img.width + 3) // 4) * max(1, (img.height + 3) // 4) * block
        if len(raw) - 128 != want:
            raise SystemExit(f"{dst}: unexpected DDS level size {len(raw) - 128}, want {want}")
        data.append(raw[128:])
        levels += 1
        if img.width == 1 and img.height == 1:
            break
        img = img.resize((max(1, img.width // 2), max(1, img.height // 2)), Image.LANCZOS)
    flags, = struct.unpack_from("<I", header, 8)
    struct.pack_into("<I", header, 8, flags | 0x20000)
    struct.pack_into("<I", header, 28, levels)
    caps, = struct.unpack_from("<I", header, 108)
    struct.pack_into("<I", header, 108, caps | 0x400008)
    dst.write_bytes(bytes(header) + b"".join(data))


def convert_texture(src, dst, size, keep_alpha):
    img = Image.open(src).convert("RGBA" if keep_alpha else "RGB")
    if max(img.size) > size:
        img = img.resize((size, size), Image.LANCZOS)
    write_dds(img, dst, "DXT5" if keep_alpha else "DXT1")


def weight_of(avatar, weights):
    best = ""
    for prefix in weights:
        if avatar.startswith(prefix) and len(prefix) > len(best):
            best = prefix
    return weights.get(best, 1.0)


def build_figure(args, blender, stock, hum_out, avatar, weight):
    name = avatar.rsplit("/", 1)[1]
    src = CACHE / avatar
    model_dir = hum_out.parent / "rocketbox" / name
    tex_dir = model_dir / "texture"
    tex_dir.mkdir(parents=True, exist_ok=True)
    sidecar = model_dir / f"{name}.json"
    r = subprocess.run(
        [blender, "-b", "--factory-startup", "-P", str(HERE / "blender_export.py"), "--",
         str(src / "Export" / f"{name}.fbx"), str(model_dir / name), str(sidecar)],
        capture_output=True, text=True)
    if r.returncode != 0 or not sidecar.exists():
        raise RuntimeError(f"{avatar}: Blender failed\n{r.stdout[-3000:]}\n{r.stderr[-3000:]}")
    info = json.loads(sidecar.read_text())
    sidecar.unlink()
    files = {p.name.lower(): p for p in (src / "Textures").iterdir()}
    textures, alpha = [], []
    for m in info["materials"]:
        if not m["texture"]:
            continue
        tga = files.get(m["texture"].lower())
        if tga is None:
            raise RuntimeError(f"{avatar}: no texture {m['texture']} for material {m['name']}")
        dds = tga.stem + ".dds"
        see_through = "opacity" in m["name"].lower() or "opacity" in tga.stem.lower()
        if dds not in textures:
            textures.append(dds)
            if see_through:
                alpha.append(dds)
        if not (tex_dir / dds).exists():
            convert_texture(tga, tex_dir / dds, args.size, see_through)
    write_cfg(model_dir / f"{name}.cfg", [lv["file"] for lv in info["levels"]], textures, alpha)
    write_hum(stock, hum_out, f"rocketbox\\{name}\\{name}.cfg", info, weight)
    tris = " / ".join(str(lv["triangles"]) for lv in info["levels"])
    return f"{hum_out.relative_to(args.out)} <- {avatar}: {tris} triangles, {info['height']} m"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--omsi", required=True, type=pathlib.Path)
    ap.add_argument("--out", type=pathlib.Path, default=HERE / "build" / "RealisticPax")
    ap.add_argument("--blender")
    ap.add_argument("--size", type=int, default=1024)
    ap.add_argument("--jobs", type=int, default=max(1, min(6, (os.cpu_count() or 2) // 2)))
    ap.add_argument("--only", help="build just this slot (a .hum as named in pax.json)")
    args = ap.parse_args()
    blender = find_blender(args.blender)
    pax = json.loads((HERE / "pax.json").read_text())

    work = []
    for hum, avatar in pax["slots"].items():
        if args.only and hum != args.only:
            continue
        stock = args.omsi / hum
        if not stock.exists():
            print(f"skip {hum}: not in {args.omsi}")
            continue
        work.append((stock, args.out / hum, avatar, 1.0))
        for alt in pax["alternates"].get(hum, []):
            out = args.out / hum
            out = out.with_name(f"{out.stem}~{alt.rsplit('/', 1)[1]}.hum")
            work.append((stock, out, alt, weight_of(alt, pax["weights"])))

    failed = False
    with concurrent.futures.ThreadPoolExecutor(args.jobs) as pool:
        jobs = [pool.submit(build_figure, args, blender, *w) for w in work]
        for job in concurrent.futures.as_completed(jobs):
            try:
                print(job.result(), flush=True)
            except RuntimeError as e:
                print(e, flush=True)
                failed = True

    shutil.copy(CACHE / "LICENSE.md", args.out / "LICENSE-Rocketbox.md")
    (args.out / "README.txt").write_text(
        "Realistic passengers for neoOMSI, made from the Microsoft Rocketbox avatars\n"
        "(https://github.com/microsoft/Microsoft-Rocketbox, MIT licence, see LICENSE-Rocketbox.md).\n"
        "Each <name>.hum replaces the stock OMSI 2 passenger of the same name; the\n"
        "<name>~<other>.hum files are drawn in its place now and then.\n")
    print(f"pack written to {args.out}")
    if failed:
        sys.exit(1)


if __name__ == "__main__":
    main()
