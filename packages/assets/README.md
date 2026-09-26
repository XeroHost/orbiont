# `@orbiont/assets`

Iconos, ilustraciones y estilos base del launcher.

Los iconos vienen de [Lucide](https://lucide.dev/) y se importan/exportan
automáticamente en `index.ts`, que se genera con `pnpm run fix`.

## Añadir recursos

- **Icono de Lucide**: descarga el SVG, guárdalo en `icons/` en kebab-case
  (p. ej. `example-icon.svg`) y ejecuta `pnpm run fix`.
- **Cualquier otra cosa**: añade el import y el export a mano en `index.ts`.

No añadas recursos de marca de Modrinth (ver `CLAUDE.md`).
