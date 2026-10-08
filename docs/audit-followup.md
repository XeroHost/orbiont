# Ampliación de la auditoría con agentes — 2026-10-07

Cambios locales realizados sobre la rama de trabajo existente, conservando las
mejoras de la otra sesión. Se mantuvieron la comprobación de compra Java, la
integración oficial de Bedrock, las licencias y las firmas del actualizador.

## Cambios implementados

| Área                | Resultado                                                                                                                                                                                                                                                                                       |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Sesiones            | Una indisponibilidad del almacén de claves ya no crea otra clave ni elimina las cuentas. La migración comprueba la lectura antes de retirar el fallback. Los originales permanecen disponibles para recuperar el acceso.                                                                        |
| Recuperaciones Java | Selección global de las 1.000 copias más recientes con desempate estable; se informa si el inventario omite entradas o encuentra metadatos inválidos.                                                                                                                                           |
| Importación Bedrock | Preflight ZIP antes del lector, límites compartidos de 2 GiB comprimidos, 20.000 entradas, 512 MiB por entrada y 8 GiB expandidos; rutas/enlaces rechazados y lectura real acotada. Se comprueba espacio para staging y copias antes de escribir.                                               |
| Cierre Bedrock      | Cierre normal de la aplicación retail identificada, espera máxima de 5 segundos y confirmación explícita para forzar. El token conserva el handle del mismo proceso, expira a los 10 minutos y evita apuntar a otro proceso por reutilización de PID. La espera forzada se limita a 2 segundos. |
| Inventario          | Medición por iteradores de profundidad, sin reunir todos los caminos; conserva tamaños parciales y marca datos incompletos. Las exclusiones de packs embebidos se indexan por raíz y ancestros, sin volver a recorrer todos los elementos por cada mundo.                                       |
| Conservación        | Antigüedad y número de copias configurables, por archivo/mundo. La acción selecciona copias elegibles para revisión y requiere confirmar el borrado. Una revisión fallida invalida selección y autorización anteriores. No hay limpieza automática ni selección de contenido instalado.         |
| Frontend            | Cuentas actualizadas por eventos y solicitudes serializadas, sin polling de 250 ms. Ace, tema y modos se cargan al abrir texto; imágenes y navegación no lo inicializan. Los errores permiten reintentar. Scroll agrupado mediante rAF con limpieza de listeners y observadores.                |
| Configuración       | Vite restringe el entorno público a la configuración necesaria. Las variables de firma y claves de proveedores quedan fuera de los prefijos expuestos. Motion usa una versión común; herramientas de tipos y SVG pasan a dependencias de desarrollo.                                            |
| Traducciones        | Los 32 catálogos traducidos, más en-US, incluyen las cadenas nuevas: 2.938 mensajes comprobados por idioma, cero huecos y contratos ICU compatibles. El porcentaje sigue calculándose desde las fuentes y traducciones.                                                                         |
| Higiene             | Adjuntos remotos, logs de fallos, archivos privados de agentes y variantes de `.env` quedan excluidos de Git. No se borraron los archivos del usuario.                                                                                                                                          |
| Medición y CI       | CPU/memoria del principal y WebView2 descendiente, separados de Minecraft; identidad PID/fecha de arranque y detección de muestras parciales. Fixtures nativos macOS/Linux y prueba optimizada del parche GLib preparados en CI.                                                                |

### Ajustes posteriores de interfaz

El almacenamiento utiliza el Checkbox compartido, con control circular visible,
estado accesible y un único cambio por clic. Las rutas largas se ajustan al ancho,
los tamaños se muestran en KiB/MiB/GiB y el modal tiene un único desplazamiento.
Los elementos protegidos siguen deshabilitados y el borrado mantiene su
confirmación explícita.

El diagnóstico conserva sus datos y estructura durante las consultas automáticas.
La verificación manual anima el icono permanente del botón, comparte solicitudes
en curso y evita dobles clics. Un fallo conserva el último resultado válido y
muestra el error. No se añadieron mensajes nuevos.

## Seguridad de dependencias

GLib 0.18.5 conserva la API que necesita GTK/Tauri y recibe las dos líneas de la
corrección oficial. El verificador exige los hashes de los 121 archivos y una
única resolución Cargo local. Se conservan LICENSE y COPYRIGHT tanto en el
repositorio como en los recursos legales del instalador. Véase
[glib-backport.md](glib-backport.md) para origen, pruebas y retirada del parche.

El audit de producción JavaScript no encuentra vulnerabilidades conocidas.
El análisis completo conserva dos alertas sin versión corregida publicada:
`braces` (Tailwind/Chokidar) y `sprintf-js` (API Extractor/argparse), herramientas
de desarrollo. No se han silenciado ni presentado como corregidas.

El audit Cargo pasa con la exclusión condicionada del RSA opcional no alcanzable
y la comprobación adicional de GLib. Conserva 11 advertencias visibles: ocho
paquetes sin mantenimiento, `rand` 0.7.3 y dos versiones retiradas de `spin`.
El caso de `rand` necesita la feature `log`, ausente del grafo actual; no se añadió
otra excepción. La ejecución optimizada GLib en Linux sigue pendiente del runner.

## Verificación local

- Biblioteca nativa: 161 pruebas pasan, 0 fallan y 6 opt-in requieren Minecraft
  real; quedan omitidas. El cambio posterior del índice de almacenamiento pasa
  sus 6 pruebas de regresión.
- Frontend final: 135 pruebas pasan, incluidos 5 fixtures de gestión y 5 de
  refresco del diagnóstico. ESLint y Prettier pasan en los archivos de ambos
  ajustes de interfaz.
- Tipos app/node y build de producción: correctos. Presupuesto: 123 archivos JS,
  6.829.897 bytes sin comprimir y 1.938.030 bytes gzip. Este tamaño total incluye
  módulos diferidos y no representa memoria ni descargas iniciales.
- Internacionalización: 16 pruebas de contrato/fuente y todos los catálogos sin
  huecos. Integridad GLib: 12 pruebas; presupuesto: 2 pruebas.
- Selección de procesos de medición: fixture correcto; excluye Minecraft y
  navegadores ajenos. El endpoint de CPU se captura después de la espera final.
- `cargo check --workspace --locked`: correcto. `pnpm lint`: 16 tareas correctas;
  permanecen 135 advertencias previas de Vue. El Clippy final de todos los targets
  de la biblioteca pasa con `-D warnings`; el ejecutable pasa con y sin updater.
  Arranque local: Orbiont abierto y ventana visible; Minecraft no se inició.
- Revisión independiente: corregidas y verificadas la autorización antigua de
  limpieza y el desfase del último intervalo de CPU. Sin hallazgos pendientes en
  los cambios revisados, incluidos los ajustes posteriores de selección y
  refresco silencioso. La revisión automatizada no sustituye una comprobación
  visual de píxeles en la ventana nativa.

Entorno: Windows x64, Rust 1.95.0, Node incluido 24.19.0 y pnpm 10.33.2.
Las compilaciones Rust se ejecutan en serie. No se cambió el Node global.

## Límites de la validación

No se iniciaron ni terminaron procesos reales de Minecraft ni se editaron mundos
reales. Login con compra, cierre dentro del juego, importación oficial completa,
fallos de disco físicos y pruebas de instaladores macOS/Linux requieren el
entorno correspondiente. Los workflows preparados no equivalen a CI remoto
ejecutado. Esta evidencia corresponde a la validación local previa a la
publicación; durante esa validación no hubo commits, push ni despliegue.
La beta.2 reúne estos cambios y los de la otra sesión en sus
[notas de versión](releases/1.0.0-beta.2.md).
