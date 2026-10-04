# RealisticPax

Builds a content pack of realistic passengers: every adult and child of the
[Microsoft Rocketbox](https://github.com/microsoft/Microsoft-Rocketbox) avatars (MIT) and the
professions whose work clothes a bus passenger might wear, plus children, teenagers and old
people made with MakeHuman's [MPFB](https://static.makehumancommunity.org/mpfb/) from its CC0
system assets (Rocketbox has four children and nobody old).

The pack is plain OMSI content (`.hum`, `.cfg`, `.o3d`, DDS textures) on the stock 13-bone
rig, with three LOD levels and three recoloured outfits (`[CTC]` clothing variants) per
figure, so the engine treats the new people like the old ones. `pax.json` decides who goes
where:

- `slots`: the avatar that replaces each stock `.hum` of OMSI 2.
- `alternates`: more avatars for a slot, written as `<name>~<avatar>.hum` beside it. Whenever
  the slot is drawn (a map's `humans.txt` names `man01.hum`), the engine picks the slot's own
  figure or one of these, so a map keeps its mix of men, women and children.
- `weights`: how often an alternate comes up against its slot's own figure (1), by avatar
  name prefix, written to the `.hum` as `[neo_weight]`. Work clothes come up less often.
- `generated`: MakeHuman people, each an alternate of its `slot`: `spec` is MPFB's human
  description (sliders, skin, hair, clothes), `age` goes to the `.hum` (tickets), `walk`
  replaces its `[walk_param]`. Hair and eyebrows of anybody 60 or older are greyed.

Needs Python 3 with Pillow 11+ and Blender 4.2+.

```bash
python tools/realistic-pax/fetch.py
python tools/realistic-pax/build.py --omsi "C:/Steam/steamapps/common/OMSI 2"
```

`fetch.py` downloads about 3.5 GB into `.cache/` and installs MPFB into a Blender profile of
its own there. `build.py` writes the pack to `build/RealisticPax` (or `--out`). Copy that
folder to `<content folder>/Packs/RealisticPax` and choose Settings → Gameplay → Passenger
models → Realistic in the launcher; the change applies on the next start. With the pack
missing or the setting on OMSI 2, the stock passengers are used.
