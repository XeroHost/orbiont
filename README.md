# Orbiont

A Minecraft Java Edition launcher by XeroHost for Windows, macOS, and Linux,
with local Minecraft Bedrock integration on Windows.

## Features

- Microsoft sign-in with Minecraft ownership verification.
- Instance management with Vanilla, Fabric, Forge, NeoForge, and Quilt.
- Modpacks and add-ons from Modrinth and CurseForge.
- Import and export Orbiont (`.orbpack`), Modrinth (`.mrpack`), and CurseForge (`.zip`) modpacks.
- Optional OptiFine setup using an installer downloaded from the official website.
- Skin selection, local worlds, screenshots, and game settings.
- Switch between Java and Bedrock. On Windows, detect and open the official
  Bedrock installation, browse local content, files, worlds, add-ons and content
  logs, open official Store updates, and send `.mcworld`, `.mcpack`, and `.mcaddon`
  files to Minecraft for import.
- Bedrock add-ons, resource packs, worlds, and scripts from CurseForge, with
  pack installation scoped to a selected world and recovery copies of its
  activation settings.
- Shared screenshot gallery and viewer for Java and Bedrock, with local pack
  and world icons in the Bedrock content lists.

## Downloads

See [GitHub Releases](https://github.com/XeroHost/orbiont/releases) for published installers.
A Microsoft account that owns Minecraft Java Edition is required to play Java.
Bedrock requires the official Minecraft for Windows installation; Minecraft
and Microsoft Store handle its sign-in and license validation.

| Platform                        | Downloads                   |
| ------------------------------- | --------------------------- |
| Windows (x64)                   | `.exe` installer            |
| macOS (Intel and Apple Silicon) | Universal `.dmg`            |
| Linux (x64)                     | `.AppImage`, `.deb`, `.rpm` |
| Source code                     | Matching `.tar.gz` archive  |

## Development

Requirements: Node.js 24.15 or later, pnpm (via Corepack), Rust as specified
in `rust-toolchain.toml`, JDK 17, CMake, NASM, and the native build tools for
your operating system.

```sh
corepack enable
pnpm install
cp packages/app-lib/.env.example packages/app-lib/.env
pnpm app:dev
```

Before starting the launcher, set `MICROSOFT_CLIENT_ID` in `.env` to your
Microsoft application's public client ID. `DISCORD_CLIENT_ID` is optional.
Environment values are embedded during compilation; keep API keys and
client secrets on the server.

To build an installer for your current operating system:

```sh
pnpm app:build
```

Windows installers are generated in `target/release/bundle/nsis/`.
Platform-specific build steps are defined in
[the build workflow](.github/workflows/theseus-build.yml).
For GitHub builds, configure the repository variable `MICROSOFT_CLIENT_ID`
and, optionally, `DISCORD_CLIENT_ID`.
Tagged releases also require the repository secrets `TAURI_PRIVATE_KEY` and
`TAURI_KEY_PASSWORD` for signing update bundles. Use the private key that matches
the public key in `apps/app/tauri-release.conf.json`; keep it outside Git.

## Credits and license

Orbiont is an independent fork of the [Modrinth App](https://github.com/modrinth/code),
originally developed by Rinth, Inc. It is not affiliated with, endorsed by,
or sponsored by Modrinth or Rinth, Inc.

**Modified by XeroHost — 2026-10-04:** custom branding and launcher UI,
CurseForge integration, Orbpack support, OptiFine setup, skin catalogs,
local translations, and local Bedrock integration for Windows.

Orbiont is not an official Minecraft product and is not approved by or
associated with Mojang or Microsoft. No Minecraft game files are distributed
with this launcher.

Animated interface icons use the free [Hugeicons](https://hugeicons.com/) set,
distributed under the [MIT license](packages/ui/src/components/base/animated-icons/LICENSE-Hugeicons.md).

The launcher and its modifications are licensed under
[GPL-3.0-only](apps/app/LICENSE), without warranty as described in the license.
Individual packages and third-party code retain their respective licenses
and copyright notices. These licenses allow modification and redistribution
under their terms; they do not grant rights to third-party trademarks.

Each release includes a matching source archive. Build instructions are
provided above and in the build workflow. Licenses are included with the
installed application in `legal/`.

<details>
<summary>Modrinth trademark notice</summary>

The use of Modrinth branding elements, including but not limited to the wrench-in-labyrinth logo, the landing image, and any variations thereof, is strictly prohibited without explicit written permission from Rinth, Inc. This includes trademarks, logos, or other branding elements.

> All rights reserved. © 2020-2025 Rinth, Inc.

If you fork this repository, you must remove all Modrinth branding assets from your fork.

</details>
