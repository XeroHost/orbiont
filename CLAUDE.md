# Orbiont

Fork del Modrinth App (GPL-3.0-only). Launcher de Minecraft solo-premium
con marca propia, para clientes de XeroHost.

**Empieza por [HANDOFF.md](HANDOFF.md)**: estado actual, ramas, cómo
arrancarlo, mapa del código y pendientes.

**Idioma**: el usuario habla español. Responde siempre en español claro y
sin jerga innecesaria. Commits también en español.

## Reglas duras

- NUNCA quitar ni debilitar `minecraft_entitlements()` en
  `packages/app-lib/src/state/minecraft_auth.rs`. Es lo que hace que el
  launcher sea solo-premium. Cualquier cambio que permita cuentas sin
  licencia se rechaza.
- NUNCA reintroducir: anuncios, PostHog, Sentry, Intercom, Stripe,
  cuenta Modrinth (`mr_auth`), amigos, tunnel, instancias compartidas,
  el flujo SISU/device token de Xbox, ni envío de estadísticas de juego a
  Modrinth u otros terceros.
- NUNCA añadir assets de marca de Modrinth ni el nombre Modrinth en la UI.
  El crédito al proyecto original va solo en el repo (README/NOTICE.md),
  nunca en la app. "Modrinth" solo aparece en la UI como fuente de
  contenido (pestaña Modrinth/CurseForge, "Open in Modrinth").
- Toda cadena de marca sale de `packages/branding`. No hardcodear
  "Orbiont" en componentes.
- Toda URL de servicio y todo ID de cliente (Microsoft, Discord) sale de
  `packages/app-lib/.env.*` vía `env!()`. No hardcodear endpoints en Rust
  ni en Vue.
- Una sola UI para todas las fuentes de contenido: CurseForge y el catálogo
  de XeroHost son fuentes de datos de los componentes nativos, nunca UIs
  propias.
- La API key de CurseForge vive solo en `orbiont-catalog` (proxy). Nunca en
  el launcher.
- NO hacer push a `origin`: apunta al upstream `modrinth/code`. El push y el
  merge a `main` los decide el usuario.
- No tocar `%APPDATA%\ModrinthApp` (la Modrinth App real del usuario).

## Estilo visual

- Cian = marca. Lo seleccionado: fondo marcado + texto blanco
  (`text-contrast`), nunca texto cian.
- El verde se queda en acciones positivas (Jugar, Instalar). No pasarlo a
  cian.

## Verificación

- `pnpm app:dev` debe arrancar después de cada cambio estructural (y la
  ventana debe verse: si `App.vue` falla al montar, queda oculta).
- `cargo check --workspace` y `pnpm lint` antes de cada commit; en el
  frontend también `npx vue-tsc --noEmit`.
- No compilar dos cosas de Rust a la vez (la máquina se queda sin memoria).
- Di siempre qué no pudiste probar (p. ej. un login real o algo visual).
