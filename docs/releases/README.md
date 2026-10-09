# GitHub publication language

All GitHub publications must use English for the international audience,
including release notes, release titles, pull request text, comments and commit
messages. Conversations with the project owner remain in Spanish.

For each release, write the English notes in `docs/releases/<version>.md` before
creating its tag. Preserve the source, license and attribution links in the
published release. Do not rewrite a published tag or rebuild installers merely
to correct release wording; edit the release description and update manifest
notes while retaining the artifact URLs, signatures and publication date.

## Final download list

Publish five installers: Windows x64 EXE, universal macOS DMG and Linux x64
AppImage, DEB and RPM. Keep `updates.json` for automatic updates. GitHub adds
two automatic source archives, so the release page shows eight assets while
the release API lists six uploaded assets.

Signed updater bundles (`Orbiont.app.tar.gz`, `*.AppImage.tar.gz` and
`*.nsis.zip`) are staged on GitHub only until XeroHost verifies and hosts them.
Before removing those three temporary assets, verify the hosted bundle
signatures and availability, then point the GitHub manifest to the hosted URLs.
If the hosting wait times out, complete these checks before finishing cleanup.
Preserve the installers, source archives, signatures and publication date.
