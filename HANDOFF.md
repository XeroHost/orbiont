# Handoff — Orbiont

Estado del proyecto para el siguiente agente o persona que lo retome.
Última actualización: **2026-09-25**. Léelo junto con [CLAUDE.md](CLAUDE.md)
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
| orbiont-catalog | `C:\Users\Raimond\Documents\XeroHost\orbiont-catalog` | Backend Node/Express (catálogo, servidores, proxy de CurseForge). Corre con PM2 en `:3010` |

**⚠ El remote `origin` del launcher apunta a `https://github.com/modrinth/code.git`
(el upstream de Modrinth).** El destino real es `XeroHost/launcher`, que no
está configurado. **No hagas push a `origin`.** Hasta ahora no se ha hecho push
de nada (ni del launcher ni del catálogo).

Las ramas están **encadenadas** (cada una sale de la anterior); no se ha
mergeado nada a `main`:

```
main
 └ fase-0-terreno-limpio → fase-1-desmarcado → fase-2-identidad-propia
   → fase-4-catalogo-curseforge → limpieza-monorepo → curseforge-modpacks
   → quitar-restos-modrinth → login-microsoft-propio   ← rama actual (HEAD)
```

`login-microsoft-propio` contiene todo. Catálogo: rama `curseforge-proxy`.

Archivos sin commitear a propósito: `Plan.md` (plan de build original),
`.serena/*` (config local). `.claude/docs/CHANGELOG.md` es un registro
técnico detallado pero está en `.gitignore` (solo local).

## 3. Cómo arrancarlo (Windows)

```bash
export PATH="/c/Program Files/CMake/bin:/c/Users/Raimond/AppData/Local/bin/NASM:$PATH"
pnpm install
pnpm app:dev
```

- Necesita CMake y NASM en el PATH (los pide una dependencia de Rust).
- El catálogo tiene que estar corriendo (`npx pm2 list` en `orbiont-catalog`);
  en local el launcher lo busca en `http://localhost:3010`
  (`packages/app-lib/.env.local`).
- La configuración se lee de `packages/app-lib/.env` (copia de `.env.local`
  o `.env.prod`) **al compilar**: cambiar un valor obliga a recompilar Rust.
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
`pnpm lint`, y que `pnpm app:dev` arranque tras cambios estructurales. Para el
frontend, además: `npx vue-tsc --noEmit` en `apps/app-frontend`.

## 4. Mapa del código

| Ruta                | Qué hay                                                                                                                                                        |
| ------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `apps/app`          | Shell de Tauri (Rust): comandos, ventana de login, updater. `build.rs` lista los comandos permitidos de cada plugin: **un comando nuevo hay que añadirlo ahí** |
| `apps/app-frontend` | UI (Vue 3 + Vite)                                                                                                                                              |
| `packages/app-lib`  | Núcleo Rust (`theseus`): instancias, instalación, auth, Discord, catálogo                                                                                      |
| `packages/ui`       | Componentes Vue compartidos (`@modrinth/ui`, conserva el nombre)                                                                                               |
| `packages/branding` | Nombre, identificadores, dominio, colores                                                                                                                      |
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
  La API key de CurseForge vive **solo** en `orbiont-catalog` (proxy con
  allowlist en `/v1/curseforge/api/*`, GET y dos POST batch).
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
| `ORBIONT_CATALOG_BASE_URL`                          | local `http://localhost:3010`; prod `https://catalog.orbiont.gg` es **placeholder** (el catálogo aún no está desplegado)           |
| `MODRINTH_API_URL`, `MODRINTH_LAUNCHER_META_URL`, … | Modrinth como **fuente de contenido** (búsqueda, mods) y metadata de versiones de Minecraft/loaders (`launcher-meta.modrinth.com`) |

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
  de "server play" a Modrinth. Las llamadas `trackEvent` del frontend son
  stubs vacíos a propósito.
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

- **Fase 5 (release)**: firma de código, instalador y **updater**. Ojo: en
  `apps/app/tauri-release.conf.json` el updater todavía apunta a
  `https://launcher-files.modrinth.com/updates.json` con la clave pública de
  Modrinth; hay que cambiarlo por un endpoint y una clave propios antes de
  publicar nada.
- **CSP** (`apps/app/tauri.conf.json`, `connect-src`): sigue permitiendo
  dominios de Modrinth Hosting (`*.nodes.modrinth.com`) y un dominio de
  Tailscale (`*.taila228c5.ts.net`) que venían del upstream. Revisar y dejar
  solo lo necesario.
- **Servidores del cliente**: el plan pide mostrar también los servidores del
  propio cliente de XeroHost cuando inicia sesión. Necesita una cuenta de
  XeroHost en el launcher (no existe todavía).
- Metadata de versiones propia con Daedalus en vez de `launcher-meta.modrinth.com`
  (opcional, recomendado en el plan).
- `minecraft_entitlements()` deserializa a un struct vacío: cualquier
  respuesta JSON válida pasa. En la práctica una cuenta sin Minecraft falla
  después, al pedir el perfil. **No lo toques sin hablar con el usuario**
  (regla dura de CLAUDE.md).
- Warnings conocidos de `cargo check`: `LegacyModrinthCredentials` y
  `LoggedIntoModrinth` (datos antiguos que se conservan a propósito).
- "Hide already installed" no reconoce instalaciones hechas desde CurseForge.

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
