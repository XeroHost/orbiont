# Orbiont

Launcher de Minecraft **solo premium** para clientes de XeroHost.

## Desarrollo

Requisitos: Node (con pnpm), Rust (`rust-toolchain.toml`), CMake y NASM.

```bash
pnpm install
pnpm app:dev
```

`pnpm app:build` genera el instalador. Antes de cada commit:
`cargo check --workspace` y `pnpm lint`.

El catálogo de modpacks/servidores y el proxy de CurseForge viven en otro
repo (`orbiont-catalog`); en local el launcher lo espera en
`http://localhost:3010` (`packages/app-lib/.env.local`).

## Estructura

| Ruta                  | Qué es                                            |
| --------------------- | ------------------------------------------------- |
| `apps/app`            | Shell de Tauri (Rust): ventana, comandos, updater |
| `apps/app-frontend`   | UI del launcher (Vue 3)                           |
| `packages/app-lib`    | Núcleo (Rust): instancias, instalación, auth      |
| `packages/ui`         | Componentes Vue compartidos                       |
| `packages/assets`     | Iconos y estilos                                  |
| `packages/branding`   | Nombre, identificadores y colores de la marca     |
| `packages/api-client` | Cliente tipado de la API de contenido             |
| `packages/daedalus`   | Tipos de metadata de Minecraft/loaders            |
| `packages/*` (resto)  | Librerías Rust que usa `app-lib`                  |

## Créditos y licencia

Orbiont es un fork del [Modrinth App](https://github.com/modrinth/code)
(GPL-3.0-only), de Rinth, Inc. No está afiliado a Modrinth. Ver
[NOTICE.md](NOTICE.md) y [COPYING.md](COPYING.md).
