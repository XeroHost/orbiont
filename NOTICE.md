# Notice

Orbiont is a fork of the [Modrinth App](https://github.com/modrinth/code),
originally created by Rinth, Inc. and licensed under GPL-3.0-only. This
fork is developed independently by XeroHost and is not affiliated with,
endorsed by, or sponsored by Modrinth or Rinth, Inc.

## What changed

- Removed everything that depends on a Modrinth account or Rinth, Inc.
  services: account sign-in (`mr_auth`), friends, shared instances, and
  the ads/consent system.
- Removed third-party telemetry and marketing integrations: PostHog,
  Sentry, Intercom, and Stripe.
- Replaced all Modrinth branding (name, icons, identifiers, color theme)
  with Orbiont's own, kept behind a single `packages/branding` module.
- Added a first-party catalog and server list for XeroHost, replacing
  the modrinth.com hosting/server-browsing integration.

## What's unchanged

- The core launcher: instance management, mod/modpack installation,
  worlds, skins, screenshots, Java management, and Microsoft/Minecraft
  account sign-in and entitlement checks.
- The modpack search integrations with Modrinth's and CurseForge's public
  APIs, used under their respective terms of service.

## License

The code inherited from the Modrinth App remains licensed under
GPL-3.0-only, as it was upstream. This repo licenses per package rather
than with a single root LICENSE file; see each package's own LICENSE
(for example [apps/app/LICENSE](apps/app/LICENSE) and
[apps/app-frontend/LICENSE](apps/app-frontend/LICENSE)). Those files and
the copyright notices from Rinth, Inc. are preserved as inherited.
