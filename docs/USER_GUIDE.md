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

## Trip evaluation

At the final stop of a scheduled trip, stop the bus and open a passenger door to
show the trip evaluation. Service trips and vehicles without a passenger cabin
only require stopping. The screen pauses single-player simulation; LAN sessions
continue running.

The table lists every planned stop with planned and actual arrival/departure
times (`HH:MM:SS`), signed differences in seconds, and a status. Positive
differences mean late; negative differences mean early. Day offsets identify
trips crossing midnight. Missing observations appear as `—`; the final departure
has no actual time when the report opens on arrival.

Use the in-game menu to view the current trip or the last completed trip. Scroll
with the mouse wheel, arrow keys, or Page Up/Page Down. Select **Save as text…**
(or press `Ctrl+S`) to export the table. **Continue** or `Esc` resumes driving.
On Android, exports are saved in the content directory's `Reports/` folder.

Observations are recorded while a duty is active. Earlier observations are not
reconstructed when loading a saved situation or joining a trip partway through.
The last completed report remains available during the current session; save it
to a text file to keep it after exiting.

## Passenger seating

Under **Settings → Gameplay → Passengers**, enable **Passengers prefer available seats**
to reserve free seats before using standing places when passengers board. Standing places
are used once all seats are occupied or reserved. This neoOMSI option is off by default;
with it off, passengers choose randomly among all free places, as in OMSI 2.

The option can also be changed through the in-game **Options → Gameplay** menu. Changes
apply to subsequent place reservations in player and timetable buses. In `settings.cfg`,
the option is stored as `pax_prefer_seats=1` (enabled) or `pax_prefer_seats=0` (disabled).

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

Set `OMSI_NO_SURF=1` before starting neoOMSI to disable OMSI `.surf` height maps for an A/B comparison of wheel contact.
