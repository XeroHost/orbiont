# Handoff — Orbiont

Estado del proyecto para el siguiente agente o persona que lo retome.
Última actualización: **2026-09-26**. Léelo junto con [CLAUDE.md](CLAUDE.md)
(las reglas duras están ahí y mandan sobre todo lo de aquí).

El usuario habla **español**: todas las respuestas, en español claro y sin
jerga innecesaria. Los commits también van en español.

---

## 1. Qué es

Orbiont es un launcher de Minecraft **solo premium** (solo cuentas de
Microsoft con Minecraft comprado), fork del Modrinth App (GPL-3.0-only),
para clientes de XeroHost. "Orbiont" es el nombre en clave; todo lo de marca
sale de `packages/branding`, así que renombrar es cambiar ese archivo.

Hace tres cosas propias sobre el Modrinth App:

1. **Modpacks**: catálogo propio de XeroHost + búsqueda en Modrinth y
   CurseForge, todo en **una sola UI nativa** (CurseForge es solo una fuente
   de datos, nunca una UI aparte).
2. **Servidores de XeroHost**: página "Servers" en la barra lateral y
   destacados en la Home.
3. **Jugar en un clic**: instala el modpack del servidor si falta y entra por
   IP directamente.

## 2. Repos y ramas

| Repo            | Ruta local                                            | Qué es                                                                                     |
| --------------- | ----------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| launcher        | `C:\Users\Raimond\Documents\XeroHost\launcher`        | Este fork                                                                                  |
| orbiont-catalog | `C:\Users\Raimond\Documents\XeroHost\orbiont-catalog` | Backend Node/Express con dos servicios en PM2: catálogo (`:3010`, modpacks y servidores de XeroHost) y **fachada de CurseForge** (`:3011`, la única con la API key; límites por IP y globales) |

**⚠ El remote `origin` del launcher apunta a `https://github.com/modrinth/code.git`
(el upstream de Modrinth).** El destino real es `XeroHost/launcher`, que no
está configurado. **No hagas push a `origin`.** Hasta ahora no se ha hecho push
de nada (ni del launcher ni del catálogo).

Las ramas están **encadenadas** (cada una sale de la anterior); no se ha
mergeado nada a `main`:

```
main
 └ fase-0-terreno-limpio → … → login-microsoft-propio
   → fase-a-seguridad-grave → fase-b-seguridad-media
   → fase-c-limpieza   ← rama actual (HEAD), contiene todo
```

Catálogo: rama `fachada-curseforge`.

Sin commitear a propósito: `Plan.md`, `.serena/*` (config local) y un cambio
en `.gitignore` que no es del agente (ignora `.claude`, `.serena`,
`CLAUDE.md`, `HANDOFF.md`, `Plan.md`). **Pregunta al usuario antes de
commitearlo**: dejaría de versionar CLAUDE.md/HANDOFF.md para quien clone.

### Fases hechas (auditoría de 2026-09)

- **A (grave)**: updater propio (`https://xerohost.net/orbiont/updates.json`,
  clave de firma en `C:\Users\Raimond\.orbiont-signing\`), todo bajo
  `xerohost.net/orbiont` (no hay centro de datos de Orbiont), fuera el script
  de Tally, fachada de CurseForge en `api-curseforge.xerohost.net`.
- **B (media)**: descargas de CurseForge y modpacks del catálogo verificadas
  (SHA-1, https, hosts del CDN), sesiones cifradas (AES-256-GCM, clave en el
  llavero de Windows), login solo https, CSP y permisos de red recortados,
  confirmación antes de lanzar desde un enlace `orbiont://`, licencia de
  Minecraft obligatoria.
- **C (limpieza)**: fuera cuenta de Modrinth, estadísticas, Hosting,
  instancias compartidas, amigos/sockets, Intercom, reportes y la marca de
  Modrinth (logo, Rinthbot, Mr Pack, fuentes de su CDN). Paquetes internos
  `@orbiont/*`; agente Java `net.xerohost.launcher` (`launcher-agent.jar`);
  User-Agent y `launcher_name` de Orbiont; enlaces `orbiont://` en todas
  partes (antes el núcleo solo aceptaba `modrinth://`).

## 3. Cómo arrancarlo (Windows)

```bash
export PATH="/c/Program Files/CMake/bin:/c/Users/Raimond/AppData/Local/bin/NASM:$PATH"
pnpm install
pnpm app:dev
```

- Necesita CMake y NASM en el PATH (los pide una dependencia de Rust).
- El catálogo tiene que estar corriendo (`npx pm2 list` en `orbiont-catalog`);
  en local el launcher lo busca en `http://localhost:3010`
  y la fachada de CurseForge en `http://localhost:3011/v1`
  (`packages/app-lib/.env.local`).
- La configuración se lee de `packages/app-lib/.env` (copia de `.env.local`
  o `.env.prod`) **al compilar**: cambiar un valor obliga a recompilar Rust.
  El shell (`apps/app`) no carga `.env`: si necesita un valor, que lo exponga
  el núcleo (p. ej. `theseus::orbiont::PRODUCT_NAME`).
- Si cambias SQL (`sqlx::query!`), regenera la caché `.sqlx`: base temporal,
  `sqlx migrate run` y `cargo sqlx prepare` en `packages/app-lib` con
  `DATABASE_URL` apuntando a ella (no la dejes en `.env`).
- Si el disco se llena, `target/debug/incremental` es caché regenerable.
- **Memoria**: la máquina tiene 16 GB. Si compilas dos cosas de Rust a la vez
  (p. ej. `cargo check` mientras corre `app:dev`), el enlazado falla con
  "El archivo de paginación es demasiado pequeño". No es un error del código:
  cierra uno y repite.
- **Procesos huérfanos**: si una sesión se corta, `app:dev` deja vivos node,
  vite, cargo y `theseus_gui.exe`. Síntoma: "Port 1420 is already in use".
  Ciérralos antes de relanzar (y comprueba que no son del usuario).
- **Ventana invisible**: la ventana se crea oculta y la muestra `App.vue` al
  montarse (`invoke('show_window')`). Si `App.vue` lanza un error al
  arrancar, el proceso vive pero no hay ventana. Por eso ESLint tiene
  `no-undef` en los `.vue`/`.js` (que no pasan por `vue-tsc`).
- Si la UI no refleja cambios del frontend aunque Vite diga "hmr update", la
  ventana perdió la conexión con Vite: reinicia `app:dev`.
- Datos de la app: `%APPDATA%\Orbiont\app.db` (SQLite). Es independiente de la
  Modrinth App, que el usuario también tiene instalada: **no toques
  `%APPDATA%\ModrinthApp`**.

Verificación antes de cada commit (ver CLAUDE.md): `cargo check --workspace`,
`npx eslint .` en cada paquete y que `pnpm app:dev` arranque tras cambios
estructurales.

Ojo con `vue-tsc`: en `apps/app-frontend` el `tsconfig.json` tiene
`"files": []` y **no comprueba nada**. Lo fiable es: (1) el build de Vite
(`node scripts/used-modules.mjs "$TEMP/used.json"` en `apps/app-frontend`,
imprime "N modules with rendered code"), (2) eslint con `no-undef`, (3)
`vue-tsc -p tsconfig.json` en `packages/ui`, que tiene ~119 errores previos:
compara el número antes y después, no esperes 0.

## 4. Mapa del código

| Ruta                | Qué hay                                                                                                                                                        |
| ------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `apps/app`          | Shell de Tauri (Rust): comandos, ventana de login, updater. `build.rs` lista los comandos permitidos de cada plugin: **un comando nuevo hay que añadirlo ahí** |
| `apps/app-frontend` | UI (Vue 3 + Vite)                                                                                                                                              |
| `packages/app-lib`  | Núcleo Rust (`theseus`): instancias, instalación, auth, Discord, catálogo                                                                                      |
| `packages/ui`       | Componentes Vue compartidos (`@orbiont/ui`)                                                                                                                   |
| `packages/branding` | Nombre, identificadores, dominio, colores                                                                                                                      |
| `packages/api-client` | Cliente HTTP del frontend: solo API de contenido de Modrinth, mclo.gs y manifiesto de loaders (`TauriApiClient`, `injectApiClient`) |
| `packages/app-lib/java` | Agente Java que corre con el juego (`net.xerohost.launcher`, `launcher-agent.jar`) |
| `packages/assets`   | Iconos y estilos (`styles/variables.scss` = colores)                                                                                                           |

Piezas propias de Orbiont:

- **Catálogo y servidores**: `app-lib/src/api/orbiont.rs` (cliente con caché),
  `apps/app/src/api/orbiont.rs` (comandos), `app-frontend/src/helpers/orbiont.ts`,
  `composables/use-orbiont-catalog.ts`, `components/ui/orbiont/*`,
  `pages/Servers.vue`, `providers/orbiont-play.ts` (jugar en un clic).
- **CurseForge**: `app-frontend/src/helpers/curseforge.ts` traduce la API de
  CurseForge al modelo de datos de Modrinth (ids con prefijo `cf-<modId>` y
  `cf-<modId>-<fileId>`). `cache.js` e `instance.ts`/`install.ts` despachan
  los ids `cf-*`. Instala dependencias requeridas.
  `app-lib/src/api/curseforge_pack.rs` convierte un modpack de CurseForge
  (`manifest.json`) en `.mrpack` para el instalador nativo; los archivos que
  el autor solo deja bajar desde curseforge.com se omiten y la app avisa.
  La API key de CurseForge vive **solo** en `orbiont-catalog`, en la
  fachada (`src/facade/index.js`, `:3011`, dominio público
  `api-curseforge.xerohost.net`). Las descargas de CurseForge las hace el
  núcleo (`orbiont::download_curseforge_file`): pide el archivo a la fachada,
  exige https, host de `forgecdn.net` y SHA-1.
- **Login**: `app-lib/src/state/minecraft_auth.rs`. Flujo: Microsoft OAuth
  (`/consumers/oauth2/v2.0`, PKCE, cliente público sin secret) → Xbox Live
  `user/authenticate` (RpsTicket `d=<token>`) → XSTS →
  `api.minecraftservices.com/authentication/login_with_xbox` →
  `minecraft_entitlements()` → perfil. La ventana está en
  `apps/app/src/api/auth.rs` (redirect `…/common/oauth2/nativeclient`, valida
  `state`). **Probado con una cuenta real: funciona.**
- **Cuenta en la UI**: `components/ui/AccountsCard.vue` es el botón de cuenta
  de la barra superior (cabeza + nombre, lista de cuentas, añadir/quitar). Los
  "Primeros pasos" (`onboarding-checklist`) van dentro de ese menú mientras
  quede algún paso.
- **Discord Rich Presence**: `app-lib/src/state/discord.rs`, aplicación de
  Discord propia (`DISCORD_CLIENT_ID`), imagen `orbiont-icon-cyan-512x`. El
  interruptor de Ajustes > Privacidad se aplica al momento.

## 5. Configuración y credenciales

En `packages/app-lib/.env.*` (leídos con `env!()` al compilar):

| Variable                                            | Valor / estado                                                                                                                     |
| --------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| `MICROSOFT_CLIENT_ID`                               | `1596fe11-fe4c-4f81-b517-d15200a5d955` — propio, **aprobado por Mojang**                                                           |
| `DISCORD_CLIENT_ID`                                 | `1552901927375732847` — aplicación "Orbiont"                                                                                       |
| `ORBIONT_PRODUCT_NAME`, `ORBIONT_SITE_URL`, `ORBIONT_SUPPORT_EMAIL`, `ORBIONT_DEEP_LINK_SCHEME` | Marca para el lado Rust (mismos valores que `packages/branding`) |
| `ORBIONT_CATALOG_BASE_URL` | local `http://localhost:3010`; prod `https://xerohost.net/orbiont/api` (**aún no desplegado**) |
| `ORBIONT_CURSEFORGE_API_URL` | local `http://localhost:3011/v1`; prod `https://api-curseforge.xerohost.net/v1` (**aún no desplegado**) |
| `ORBIONT_UPDATES_URL` | `https://xerohost.net/orbiont/updates.json` (manifiesto firmado del updater) |
| `MODRINTH_URL`, `MODRINTH_API_URL(_V3)`, `MODRINTH_LAUNCHER_META_URL` | Modrinth solo como **fuente de contenido** y metadata de loaders |

La API key de CurseForge está en el `.env` de `orbiont-catalog` (no en el
launcher). No subas `.env` a ningún sitio.

## 6. Decisiones del usuario (no las deshagas)

- **Una sola UI para todas las fuentes**: CurseForge/Modrinth/XeroHost se
  muestran con los mismos componentes nativos. Nada de UIs por proveedor.
- **Colores**: cian = marca. **Lo seleccionado lleva fondo marcado y texto
  blanco** (`text-contrast`), nunca texto cian (desplegables, chips, radios,
  pestañas). El **verde** se queda como acento de acciones positivas
  (Jugar, Instalar): no lo cambies a cian.
- **Discord Rich Presence se queda** (con la app de Orbiont).
- **Nada de estadísticas a terceros**: se quitó el envío de tiempo de juego y
  de "server play" a Modrinth, y todo `trackEvent`.
- **Solo premium, con licencia**: `minecraft_entitlements()` exige
  `product_minecraft` o `game_minecraft` (aprobado por el usuario).
- El crédito a Modrinth va solo en README/NOTICE.md, nunca en la app.

## 7. Pendiente

### Del usuario (no delegable)

- Dominio y nombre definitivos (Orbiont choca con ORBIONT LTD, UK).
- Desplegar `orbiont-catalog` y fijar su URL real en `.env.prod`.
- Crear/configurar el repo `XeroHost/launcher` y decidir cuándo hacer push y
  mergear las ramas a `main`.
- Aviso/permiso a Modrinth por el uso comercial de su API.
- Certificado de firma de código (Fase 5).

### Técnico

- **Fase D (publicar 1.0.0 beta en `main`)**: falta el repo `XeroHost/launcher`
  como remote (hoy solo existe `origin` = modrinth/code, **no hacer push ahí**).
  Los workflows (`theseus-build.yml`, `theseus-release.yml`, `turbo-ci.yml`)
  aún usan los runners privados de Modrinth (`namespace-profile-*`) y su caché
  (`nscloud-cache-action`): hay que pasarlos a runners de GitHub antes de que
  un tag compile. Firmar con la clave del updater y subir `updates.json` a
  `xerohost.net/orbiont`.
- **Perfil de usuario** (`packages/ui/src/layouts/shared/user-profile`,
  ~1000 líneas): es la página de perfil de la web de Modrinth con
  herramientas de staff (facturación, afiliados, bloquear, reportar). En la
  app están ocultas (no hay cuenta), pero el código sigue. Reescribirla con
  prueba visual.
- `packages/api-client/src/modules/labrinth/types.ts` conserva tipos de la
  API de Modrinth que ya no se usan (facturación, payouts…). Solo tipos.
- El crate del núcleo se sigue llamando `theseus` (nombre en clave del
  upstream) y el shell `theseus_gui`; renombrarlo toca todos los `theseus::`.
- Traducciones: `intl:extract` regeneró el inglés; los demás idiomas
  conservan claves de textos ya borrados (inofensivo).
- **Panel de XeroHost**: administrar el servidor del cliente (mods, plugins,
  modpacks) desde Orbiont. Necesita cuenta de XeroHost en el launcher.
- Metadata de versiones propia con Daedalus en vez de
  `launcher-meta.modrinth.com` (opcional).
- Warnings conocidos de clippy: 7 en `theseus` (`set_readonly(false)`,
  punteros crudos), anteriores a este trabajo.
- "Hide already installed" no reconoce instalaciones hechas desde CurseForge.
- De la auditoría, el usuario dejó para después "Falta por implementar" y
  "Malas prácticas".

## 8. Cómo se ha trabajado (y conviene seguir)

- Una rama por bloque de trabajo, commits en español con cuerpo explicando
  el porqué, terminando con la línea `Co-Authored-By` del agente.
- Textos de la UI con `defineMessages` y traducción al español (`es-ES` y
  `es-419`) de las claves nuevas; tras cambiar mensajes, `pnpm run
intl:extract` en `apps/app-frontend`.
- Antes de borrar código "muerto" del frontend, comprobar que el bundle no
  lo usa (se hizo construyendo con Vite y listando los módulos renderizados).
- Verificar con datos reales siempre que se pueda (API de Microsoft/Discord,
  base de datos en solo lectura) y decir claramente qué no se pudo probar.
