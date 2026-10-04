"""Download the Rocketbox avatars named in pax.json (mesh + colour textures only)."""

import json
import pathlib
import sys
import urllib.request

REPO = "microsoft/Microsoft-Rocketbox"
BRANCH = "master"
HERE = pathlib.Path(__file__).resolve().parent
CACHE = HERE / ".cache"


def wanted(path, avatar):
    name = avatar.rsplit("/", 1)[1]
    if path == f"Assets/Avatars/{avatar}/Export/{name}.fbx":
        return True
    return path.startswith(f"Assets/Avatars/{avatar}/Textures/") and "_color" in path.rsplit("/", 1)[1]         and path.endswith(".tga")


def main():
    pax = json.loads((HERE / "pax.json").read_text())
    avatars = sorted(set(pax["slots"].values()) | {a for v in pax["alternates"].values() for a in v})
    with urllib.request.urlopen(
        f"https://api.github.com/repos/{REPO}/git/trees/{BRANCH}?recursive=1"
    ) as r:
        tree = json.load(r)["tree"]
    for avatar in avatars:
        files = [t for t in tree if t["type"] == "blob" and wanted(t["path"], avatar)]
        if not any(f["path"].endswith(".fbx") for f in files):
            sys.exit(f"{avatar}: not in {REPO}")
        for f in files:
            out = CACHE / f["path"].removeprefix("Assets/Avatars/")
            if out.exists() and out.stat().st_size == f["size"]:
                continue
            out.parent.mkdir(parents=True, exist_ok=True)
            print(f"{f['path']} ({f['size'] / 1e6:.1f} MB)", flush=True)
            url = f"https://raw.githubusercontent.com/{REPO}/{BRANCH}/{f['path']}"
            tmp = out.with_suffix(out.suffix + ".part")
            urllib.request.urlretrieve(url, tmp)
            tmp.replace(out)
        lic = CACHE / "LICENSE.md"
        if not lic.exists():
            urllib.request.urlretrieve(
                f"https://raw.githubusercontent.com/{REPO}/{BRANCH}/LICENSE.md", lic
            )


if __name__ == "__main__":
    main()
