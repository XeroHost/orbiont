# Orbiont

Fork del Modrinth App (GPL-3.0-only). Launcher de Minecraft solo-premium
con marca propia, para clientes de XeroHost.

## Reglas duras
- NUNCA quitar ni debilitar `minecraft_entitlements()` en
  `packages/app-lib/src/state/minecraft_auth.rs`. Es lo que hace que el
  launcher sea solo-premium. Cualquier cambio que permita cuentas sin
  licencia se rechaza.
- NUNCA reintroducir: anuncios, PostHog, Sentry, Intercom, Stripe,
  cuenta Modrinth (`mr_auth`), amigos, tunnel, instancias compartidas.
- NUNCA añadir assets de marca de Modrinth ni el nombre Modrinth en la UI.
  La única mención permitida está en el About y en NOTICE.md.
- Toda cadena de marca sale de `packages/branding`. No hardcodear
  "Orbiont" en componentes.
- Toda URL de servicio sale de `packages/app-lib/.env.prod`. No hardcodear
  endpoints en Rust ni en Vue.

## Verificación
- `pnpm app:dev` debe arrancar después de cada cambio estructural.
- `cargo check --workspace` y `pnpm lint` antes de cada commit.
