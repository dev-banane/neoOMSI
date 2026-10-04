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
- `weights`: how often a figure comes up against the others of its slot (1), by avatar
  name prefix, written to the `.hum` as `[neo_weight]`. Work clothes, headscarves and Gulf
  robes come up less often: the maps are mostly German towns.
- `generated`: MakeHuman people, each an alternate of its `slot`: `spec` is MPFB's human
  description (sliders, skin, hair, clothes), `age` goes to the `.hum` (tickets), `walk`
  replaces its `[walk_param]`. Hair and eyebrows of anybody 60 or older are greyed.

Each figure gets the ticket-pack voice of its age and sex (a child's, an old woman's, the
deepest man's for an old man) instead of the voice of the stock person whose place it takes.

Needs Python 3 with Pillow 11+ and Blender 4.2+.

```bash
python tools/realistic-pax/fetch.py
python tools/realistic-pax/build.py --omsi "C:/Steam/steamapps/common/OMSI 2"
```

`fetch.py` downloads about 3.5 GB into `.cache/` and installs MPFB into a Blender profile of
its own there. `build.py` writes the pack to `build/RealisticPax` (or `--out`); after a change
to `weights` or the voices, `build.py --hums-only` rewrites just the `.hum` files of a pack
built before, without Blender. Copy the folder to `<content folder>/Packs/RealisticPax` and
choose Settings → Gameplay → Passenger models → Realistic in the launcher; the change applies
on the next start. With the pack missing or the setting on OMSI 2, the stock passengers are
used.

## Licences

Nothing of the avatars is in the neoOMSI repository or its releases: `fetch.py` downloads
them and the pack is built on your machine.

| Source | Licence | In the pack |
| --- | --- | --- |
| [Microsoft Rocketbox](https://github.com/microsoft/Microsoft-Rocketbox) avatars | MIT | `rocketbox/`, `LICENSE-Rocketbox.md` |
| MakeHuman base mesh, targets and system assets ([MakeHuman](http://www.makehumancommunity.org)) | CC0 1.0 | `generated/`, `LICENSE-MakeHuman.txt` |
| [MPFB](https://static.makehumancommunity.org/mpfb/) Blender add-on | GPL-3.0-or-later | nothing: it only runs in Blender while building |
| Your OMSI 2 installation's passenger `.hum` files | OMSI 2's | every `.hum`, with body, age and voice changed |

Because the `.hum` files are derived from OMSI 2's, a built pack is for your own use and must
not be redistributed. The scripts in this folder are part of neoOMSI and GPL-3.0-or-later like
the rest of its source.
