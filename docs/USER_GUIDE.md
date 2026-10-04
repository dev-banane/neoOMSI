# User guide

This guide covers running neoOMSI, essential keybindings, and common configuration options.

> [!IMPORTANT]
> **neoOMSI requires an existing OMSI 2 installation.** neoOMSI does not distribute copyrighted game content. On first launch, you must provide the path to your OMSI 2 installation folder.

## Getting started

1. Download the latest release from the [Releases](https://github.com/neoOMSI/neoOMSI/releases) page for your operating system.
2. Extract the archive into a folder with write permissions (e.g. within your user directory).
3. Launch `neoomsi` (`neoomsi.exe` on Windows).
4. If prompted, select your OMSI 2 installation directory (containing `Omsi.exe` and `maps/`).
5. Select a map, vehicle, and duty, then start the simulation.

## Keybindings

### Driving controls

| Action | Primary Key | Alternative |
| --- | --- | --- |
| **Throttle** | `W` | `Up Arrow` |
| **Brake** | `S` | `Down Arrow` |
| **Steer Left** | `A` | `Left Arrow` |
| **Steer Right** | `D` | `Right Arrow` |
| **Mouse Steering** | `O` | Toggles mouse steering on/off |

### Vehicle operations

| Action | Key | Description |
| --- | --- | --- |
| **Battery / Ignition** | `E` | Inserts key and powers electrical system |
| **Engine Starter** | `M` | Hold to crank engine until started |
| **Drive Gear (D)** | `Shift + D` | Engages forward drive |
| **Neutral (N)** | `N` | Neutral gear |
| **Reverse (R)** | `R` | Reverse gear |
| **Parking Brake** | `.` | Toggles handbrake |
| **Quick Autostart** | `Shift + U` | Automates the complete startup sequence |

### Camera & cockpit

- **Cockpit switches:** Left-click to toggle, click and drag to turn rotary dials.
- **Look around:** Hold Right-Mouse-Button and move mouse (or arrow keys / `I`/`J`/`K`/`L`).
- **In-game menu:** Press `Esc` to access settings, switch buses, or exit.

## Command-line options

You can launch directly into a specific scenario using command-line arguments:

```sh
neoomsi --map maps/Grundorf/global.cfg --bus Vehicles/MAN_SD200/MAN_SD80.bus
```

| Flag | Description |
| --- | --- |
| `--root <path>` | Path to the OMSI 2 base directory |
| `--map <path>` | Path to the map global configuration (`maps/.../global.cfg`) |
| `--bus <path>` | Vehicle file to load (`Vehicles/.../*.bus`) |
| `--weather <path>` | Weather profile to apply (`Weather/*.owt`) |
| `--time <HH:MM>` | Initial simulation time |
| `--date <YYYY-MM-DD>` | Initial simulation date |
| `--enhanced` | Enable enhanced physically based rendering mode |

## Passengers

Settings → Gameplay in the launcher has three passenger settings:

| Setting | Choices | Default |
| --- | --- | --- |
| Passenger models | **OMSI 2**: the people of your OMSI 2 installation. **Realistic**: the RealisticPax pack (applies on the next start). | OMSI 2 |
| Passenger movement | **Natural**: everyone walks at a pace and in a style of their own, people make room for each other, spread over the doors, run for a bus that is about to leave, look around while waiting and lean into reaching for the validator or the cash desk. **OMSI 2**: exactly as OMSI 2 animates and moves them. | Natural |
| Passenger voices | Greetings and tickets, only the ticket asked for, or silent. | Greetings and tickets |

With Passenger models and Passenger movement both on **OMSI 2**, the passengers look and move as in OMSI 2. In either case every passenger gets a soft shadow on the ground or the bus floor under them; Settings → Graphics → Shadow patches under vehicles and people switches those off together with OMSI's shadow meshes under the buses.

The Realistic pack is not part of neoOMSI: it is built on your own computer from Microsoft Rocketbox and MakeHuman assets with [tools/realistic-pax](../tools/realistic-pax/README.md) and copied to `Packs/RealisticPax` in the content folder. Without it, Realistic falls back to the OMSI 2 people.

## Modding

Place add-on content into the `Mods/` directory alongside the `neoomsi` executable. neoOMSI mounts add-ons into its virtual filesystem without altering original OMSI 2 files.