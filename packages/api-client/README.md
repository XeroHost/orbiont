# `@orbiont/api-client`

Cliente HTTP del frontend del launcher. Solo cubre lo que la app usa:

- **API de contenido de Modrinth** (`client.labrinth`): proyectos, etiquetas y
  usuarios (autores). Modrinth es una fuente de contenido; el launcher no usa
  cuentas de Modrinth.
- **mclo.gs** (`client.mclogs`): compartir y analizar logs.
- **Manifiesto de loaders** (`client.launchermeta`): versiones de Fabric,
  Forge, NeoForge y Quilt.

Las peticiones pasan por el plugin HTTP de Tauri (`TauriApiClient`). El resto
de servicios del launcher (catálogo de XeroHost, fachada de CurseForge,
Minecraft) los llama el núcleo en Rust, no este paquete.

## Uso

```ts
import { TauriApiClient, VerboseLoggingFeature } from '@orbiont/api-client'

const client = new TauriApiClient({
	userAgent: 'mi-launcher/1.0.0',
	labrinthBaseUrl: 'https://api.modrinth.com',
	features: [new VerboseLoggingFeature()],
})

const project = await client.labrinth.projects_v2.get('sodium')
```

En los componentes, el cliente se inyecta con `injectApiClient()` de
`@orbiont/ui` (la app lo registra con `provideApiClient` en `App.vue`).

## Añadir un módulo

1. Crea la clase en `src/modules/<api>/<modulo>/<version>.ts` extendiendo
   `AbstractModule` y devuelve su id en `getModuleID()` (`<api>_<modulo>`).
2. Añádela a `MODULE_REGISTRY` en `src/modules/index.ts`; el tipo del cliente
   se infiere solo.
3. Los tipos de la API van en `src/modules/<api>/types.ts`.
