# `@orbiont/api-client`

HTTP client for the launcher frontend:

- **Modrinth content API** (`client.labrinth`): projects, tags, and content attribution types. Modrinth is a content provider; the launcher does not use Modrinth accounts or expose author profile pages.
- **mclo.gs** (`client.mclogs`): sharing and analyzing logs.
- **Loader manifests** (`client.launchermeta`): Fabric, Forge, NeoForge, and Quilt versions.

Requests use the Tauri HTTP plugin through `TauriApiClient`. Native Rust APIs handle CurseForge, skin catalog access, and Minecraft authentication.

## Usage

```ts
import { TauriApiClient, VerboseLoggingFeature } from '@orbiont/api-client'

const client = new TauriApiClient({
	userAgent: 'example-launcher/1.0.0',
	labrinthBaseUrl: 'https://api.modrinth.com',
	features: [new VerboseLoggingFeature()],
})

const project = await client.labrinth.projects_v2.get('sodium')
```

Components receive the client with `injectApiClient()` from `@orbiont/ui`. The app registers it with `provideApiClient` in `App.vue`.

## Adding a module

1. Create a class in `src/modules/<api>/<module>/<version>.ts` extending `AbstractModule`. Return its identifier from `getModuleID()`.
2. Add it to `MODULE_REGISTRY` in `src/modules/index.ts`, using a key such as `<api>_<module>_<version>`. The client type is inferred from this registry.
3. Define API types in `src/modules/<api>/types.ts`.
