export interface Question {
  q: string
  a: string
}

export const FAQ: Question[] = [
  {
    q: 'What is neoOMSI?',
    a: 'neoOMSI is a free, open-source recreation of the bus simulator OMSI 2, written from scratch in Rust. It plays the maps, buses and mods of an installed OMSI 2 on a modern 64-bit engine with native builds for Windows, macOS, Linux and Android.',
  },
  {
    q: 'Is neoOMSI free?',
    a: 'Yes. neoOMSI costs nothing and its source code is published on GitHub under the GPL-3.0-or-later license. You only need your own copy of OMSI 2, which neoOMSI uses for the game content.',
  },
  {
    q: 'Do I need OMSI 2 to play neoOMSI?',
    a: 'Yes. neoOMSI contains no maps, vehicles or other game content. It reads them from an installed copy of OMSI 2 and does not start without one. On the first start, you point the launcher to the OMSI 2 folder that contains the maps and Vehicles folders.',
  },
  {
    q: 'Does neoOMSI work with my OMSI 2 maps, buses and mods?',
    a: 'That is the goal of the project. neoOMSI targets the observable behavior of OMSI 2.2.032, so community maps, buses, splines and scripts are meant to run without conversion. Subsystems are verified against the original game one by one and protected by regression tests once they match. Content that still behaves differently can be reported on GitHub.',
  },
  {
    q: 'Can I play OMSI 2 on a Mac?',
    a: 'Yes, with neoOMSI. OMSI 2 itself only runs on Windows, but neoOMSI has native macOS builds for Apple silicon (M1 or newer) and Intel Macs on macOS 11 or newer. Copy your OMSI 2 folder to the Mac and point neoOMSI to it.',
  },
  {
    q: 'Can I play OMSI 2 on Linux?',
    a: 'Yes, with neoOMSI. It runs natively on Linux for x86-64 and ARM64 with Vulkan drivers, so you do not need Wine or Proton. It uses the files of your OMSI 2 installation.',
  },
  {
    q: 'Can I play OMSI 2 on Android?',
    a: 'Yes, with neoOMSI. The Android build runs on arm64 phones and tablets with Android 8.0 or newer. Install the APK, allow access to all files and copy your whole OMSI 2 folder to neoOMSI/OMSI 2 on the device.',
  },
  {
    q: 'Which platforms does neoOMSI support?',
    a: 'neoOMSI has native builds for Windows 10 and 11 (x64), Windows 11 on ARM, macOS 11 or newer (Apple silicon and Intel), Linux (x64 and ARM64) and Android 8.0 or newer (arm64). There is also a dedicated server for Windows and Linux.',
  },
  {
    q: 'Does neoOMSI change my OMSI 2 installation?',
    a: 'No. neoOMSI only reads your OMSI 2 folder. Add-ons go into a separate Mods folder next to neoOMSI, or onto the Mods page of the launcher, so trying or removing them never touches your OMSI 2 files.',
  },
  {
    q: 'Does neoOMSI remove the memory limits of OMSI 2?',
    a: 'Yes. OMSI 2 is a 32-bit program, while neoOMSI is a 64-bit engine with multithreaded tile streaming and background texture decompression, so large maps and detailed buses are not held back by a 32-bit memory ceiling.',
  },
  {
    q: 'Which graphics APIs does neoOMSI use?',
    a: 'neoOMSI renders through wgpu, which uses DirectX 12 on Windows, Metal on macOS and Vulkan on Linux and Android.',
  },
  {
    q: 'Does neoOMSI support multiplayer?',
    a: 'neoOMSI includes a headless dedicated server for hosting multiplayer sessions. It needs no GPU or display, simulates the map, AI traffic and timetables, and runs on Windows and Linux, including Raspberry Pi 4 and 5.',
  },
  {
    q: 'What is the difference between neoOMSI and openOMSI?',
    a: 'neoOMSI is built to be the more stable choice. Two team members check every change, and changes are bundled into proper releases that are tested before they come out. Both are separate projects that play your own OMSI 2 maps, buses and mods.',
  },
  {
    q: 'Is neoOMSI affiliated with the makers of OMSI 2?',
    a: 'No. neoOMSI is an independent community project and is not affiliated with or endorsed by MR Software or Aerosoft. It contains no code or assets from OMSI 2.',
  },
  {
    q: 'Is neoOMSI finished?',
    a: 'No. neoOMSI is an early release. Expect bugs, missing features and changes between versions. A new build is published for every change, and the Releases page lists what changed.',
  },
  {
    q: 'How do I install mods in neoOMSI?',
    a: 'Put add-on content into the Mods folder next to the neoOMSI executable, or add it on the Mods page of the launcher. neoOMSI mounts add-ons on top of your OMSI 2 files without changing them.',
  },
  {
    q: 'How do I report a bug in neoOMSI?',
    a: 'Open an issue on the neoOMSI GitHub repository and describe the map, bus and steps that show the problem. Comparing the behavior with OMSI 2.2.032 helps the most. You can also ask on the neoOMSI Discord.',
  },
]

export const HOME_FAQ = FAQ.filter((f) =>
  ['What is neoOMSI?', 'Do I need OMSI 2 to play neoOMSI?', 'Can I play OMSI 2 on a Mac?', 'Does neoOMSI change my OMSI 2 installation?', 'What is the difference between neoOMSI and openOMSI?'].includes(f.q),
)

export const OPENOMSI_FAQ: Question[] = [
  {
    q: 'Is neoOMSI the same as openOMSI?',
    a: 'No. neoOMSI is a separate project with its own team, releases and place to report bugs.',
  },
  {
    q: 'Does neoOMSI contain code from openOMSI?',
    a: "Some of neoOMSI's early code came from openOMSI by usonskyyyy. It is credited in the NOTICE file of the neoOMSI repository.",
  },
  {
    q: 'Is neoOMSI affiliated with openOMSI?',
    a: 'No. neoOMSI is made by a different team and is not endorsed by the openOMSI team.',
  },
  {
    q: 'Do neoOMSI and openOMSI both need OMSI 2?',
    a: 'Yes. Both read the maps, buses and mods of an installed copy of OMSI 2, and neither ships game content of its own.',
  },
  {
    q: 'How do I switch from openOMSI to neoOMSI?',
    a: 'Download neoOMSI for your system, unpack it into its own folder and point the launcher to the same OMSI 2 folder you used before. Copy your add-ons into the Mods folder next to neoOMSI. Your OMSI 2 files are never changed, so you can keep openOMSI installed alongside it.',
  },
  {
    q: 'Where can I download openOMSI?',
    a: 'openOMSI is published on GitHub at github.com/openOMSI-Project/openOMSI. neoOMSI builds are on the neoOMSI download page.',
  },
]
