# Revisión de seguridad, fiabilidad y experiencia — 2026-10-07

Implementación local de los seis apartados de la auditoría solicitada. No se
publicó, firmó ni instaló una nueva versión; las verificaciones automáticas usan
fixtures temporales y no modifican mundos reales ni inician o detienen Minecraft.

## Cambios

| Apartado            | Resultado                                                                                                                                                                                          |
| ------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Errores y seguridad | Tokens OAuth/PKCE excluidos de spans, redacción de diagnósticos, rutas y enlaces validados, descargas y ZIP acotados, editores con revisiones y guardado recuperable.                              |
| Fiabilidad          | Lock único Bedrock, UUID/versión/ubicación para activaciones, diarios durables, restauración parcial reanudable y copias conservadas cuando hay conflicto.                                         |
| Rendimiento         | Lectura ZIP por bloques a staging en disco, contexto Java resuelto una vez, recuentos acotados, monitor de procesos ligero, caché de metadata, invalidación agrupada y Ace con imports explícitos. |
| Validación          | Typecheck real app/node, fixtures Windows en CI, análisis de dependencias y presupuestos de tamaño, pruebas de fallos/conflictos y navegación con borradores.                                      |
| Simplificación      | Centro común para copias de gestión/instalación y Java, versiones de pnpm unificadas, dependencias de UI no usadas retiradas y documentación técnica versionable.                                  |
| Funciones nuevas    | Recuperación con origen/estado/tamaño/vista previa, almacenamiento por categoría y limpieza explícita, versiones/dependencias de packs, estados de arranque y exportación sin datos sensibles.     |

Los límites de descarga distinguen archivos grandes en disco (2 GiB) y
respuestas mantenidas en memoria (64 MiB). ZIP tiene límites de entradas,
directorio central, tamaño por entrada y tamaño expandido, comprobados también
mientras se descomprime. Los manifiestos mrpack se leen con un máximo de 8 MiB.
El editor de texto conserva su borrador ante cambios externos o errores y guarda
el original antes de sustituirlo. La limpieza no considera mundos, mods o packs
instalados como caché eliminable.

El cierre del editor evita reenviar un segundo evento de cierre tras confirmar
el borrador. Completa la salida con la acción que utiliza Tauri y el permiso
restringido a la ventana local `main`. Si esa acción falla, se restablece la
protección del borrador. Las pruebas incluyen cancelación, fallo de cierre,
descarga de la página retrasada y liberación de listeners.

La coordinación serializa las operaciones de Orbiont. Un escritor externo puede
actuar entre la última comparación de revisión y la sustitución del archivo;
no se afirma una exclusión universal entre procesos. Una importación de varios
archivos tampoco constituye una transacción del sistema de archivos: si falla
la reversión, se devuelve el error y se conservan originales para recuperación.
No se simuló una caída eléctrica ni una sustitución hostil de directorios.

## Dependencias

Se actualizaron parches y versiones compatibles de Node/Rust y se migraron las
APIs DNS de Hickory y XML necesarias. El análisis de producción de pnpm se
ejecuta en CI. Las alertas sin parche de `braces` y `sprintf-js` pertenecen a
herramientas de desarrollo (patrones de Tailwind y generación de declaraciones),
no al frontend distribuido; no se han presentado como corregidas.

El cierre incorpora además `anyhow` 1.0.103, `event-listener` 5.4.2 y `rand`
0.8.6/0.9.3 para resolver avisos informativos de seguridad con parche compatible.
El ejemplo de ping usa Clap 4 en lugar de StructOpt; conserva sus argumentos y
ayuda, con pruebas de parseo sin conexión. Esto retira `atty`, `ansi_term`,
`structopt` y `proc-macro-error` de ese recorrido de desarrollo.

`RUSTSEC-2023-0071` afecta al RSA opcional de `sqlx-mysql` presente en Cargo.lock.
Orbiont sólo habilita SQLite. `scripts/check-cargo-advisories.mjs` verifica que
RSA no aparece en el grafo normal/build de todo el workspace, todas las features
propias y todas las plataformas, antes de exceptuar exclusivamente esa alerta.
Si llega a ser una dependencia alcanzable, CI falla. No se deshabilita el resto
del análisis ni se afirma que exista un parche RSA.

El análisis Cargo actualizado conserva 11 avisos informativos visibles. GLib se
resuelve desde la copia local verificada descrita en [glib-backport.md](glib-backport.md):

| Aviso                                                           | Estado                                                                                                                                                                          |
| --------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `glib` 0.18.5, RUSTSEC-2024-0429                                | Backport compatible de las dos líneas oficiales aplicado y verificado por hashes y origen Cargo. La ejecución optimizada Linux está preparada en CI y pendiente de ese entorno. |
| `rand` 0.7.3, RUSTSEC-2026-0097                                 | Dependencia de generación PHF de Tauri. El escenario exige la feature `log`, ausente del grafo comprobado con todas las features y plataformas; el aviso no se oculta.          |
| `fxhash`, `paste`, `proc-macro-error` y cinco paquetes `unic-*` | Avisos de falta de mantenimiento en dependencias transitivas; actualizar sus consumidores requiere cambios adicionales de upstream.                                             |
| `spin` 0.9.8 y 0.10.0                                           | Versiones retiradas del índice que permanecen fijadas en dependencias transitivas. No se presentan como actualizadas.                                                           |

Por tanto, el análisis aprobado no significa que todos los avisos del ecosistema
estén corregidos. La ejecución optimizada del backport GLib en Linux y los cambios
mayores para retirar los avisos transitivos siguen pendientes.

El comando válido de tipos es
`pnpm --filter @orbiont/app-frontend tsc:check`. Ejecutar vue-tsc sin proyecto
explícito en el frontend no comprueba sus archivos: el tsconfig raíz sólo
contiene referencias. Se mantienen strict y las comprobaciones de errores.

## Medición reproducible

Después de `pnpm --filter @orbiont/app-frontend build`, ejecutar
`node scripts/frontend-budget.mjs --check`: cuenta archivos JS, bytes sin comprimir,
gzip y módulos auxiliares de Ace. CI comprueba el presupuesto de JS/Ace.

Para observar la aplicación abierta en Windows, usar
`./scripts/measure-launcher.ps1 -ProcessId <PID> -Seconds 10`. Registra CPU,
memoria y handles sin iniciar o detener procesos. Los datos de desarrollo no
sustituyen una medición del instalador de producción en el mismo equipo.
El script actualizado incluye el principal y los descendientes WebView2,
excluyendo Minecraft. Distingue reutilización de PID y marca mediciones parciales
si nacen o salen procesos durante las muestras. La suma de working sets puede
contar páginas compartidas varias veces; no equivale a memoria física exclusiva.

## Validaciones manuales pendientes de entorno real

1. Instalador Windows: login Java con compra, jugar/detener Java y Bedrock,
   importación oficial y resultado dentro del juego, actualizar vía Store y
   comprobar persistencia tras actualizar Orbiont.
2. Editar un archivo, modificarlo externamente y guardar; salir de la edición
   con borrador y cancelar. Probar con un mundo de prueba exportado, no uno único.
3. Retirar/restaurar un pack con dos versiones y dependencias; simular un cierre
   durante la operación y comprobar la recuperación en un mundo de prueba.
4. Medir arranque y reposo de una build instalada, bibliotecas grandes y discos
   lentos; comparar CPU/memoria y latencias con una línea base del mismo equipo.
5. Ejecutar macOS/Linux en sus runners y verificar la exclusión de operaciones
   Bedrock. Los jobs añadidos localmente no prueban por sí solos un CI remoto.
6. API externa `api-orbiont`: revisar límites, autenticación, almacenamiento de
   claves y protección de descargas en su propio repositorio. Este checkout no
   contiene su implementación ni permite auditarla completamente.
7. Confirmar autorización comercial de los proveedores de contenido. Nitro
   rechazó la solicitud; no hay red publicitaria aprobada ni SDK activado.

Se conservan licencias y créditos existentes, la validación premium Java y la
autenticación oficial Bedrock. Una auditoría de código no concede permisos de
uso de servicios externos.

## Evidencia de esta revisión

Resultados de la primera revisión local (antes de la ampliación con agentes
documentada en [audit-followup.md](audit-followup.md)):

- Biblioteca nativa: 142 pruebas aprobadas, ninguna fallida y 6 pruebas opt-in
  omitidas. Los casos usan fixtures; no se ejecutaron pruebas destructivas sobre
  los datos reales del usuario.
- `cargo check --workspace --locked`, formato Rust y Clippy estricto de todos
  los targets: aprobados. También pasa Clippy estricto del ejecutable con
  el actualizador habilitado.
- Ejemplo de ping: tres pruebas aprobadas con la configuración predeterminada
  y tres con SRV habilitado; todas prueban argumentos sin conectar a un servidor.
- Frontend: 114 pruebas aprobadas. Los proyectos app/node pasan la comprobación
  estricta de tipos; no se desactivó `strict` ni se ocultaron errores globalmente.
- Internacionalización: 16 pruebas aprobadas de contrato ICU y catálogos.
  Las 36 cadenas iniciales se tradujeron primero en en-US/es-ES/es-419. La
  ampliación posterior completó las cadenas nuevas en los 33 idiomas soportados.
- Build de producción: 102 archivos JavaScript, 6.943.327 bytes sin comprimir
  (6,62 MiB) y 1.946.416 bytes gzip (1,86 MiB). Ace aporta 22 módulos y
  1.957.227 bytes de código renderizado (1,87 MiB). Pasa el presupuesto; sus dos
  pruebas comprueban también que no se acepte una métrica ausente.
- `pnpm audit --prod --audit-level=moderate`: ninguna vulnerabilidad conocida.
  El análisis Cargo pasa con la excepción RSA condicionada descrita arriba.
- Revisión independiente: cuatro hallazgos adicionales corregidos y aceptados:
  rollback ZIP que podía descartar originales, directorio central con tamaño
  declarado falso, doble contabilización de packs embebidos y manifiestos con BOM
  que podían ocultar dependencias activas.
- La revisión final del editor detectó y aceptó la corrección adicional del
  permiso de cierre y del manejo de errores de la ventana; sus pruebas pasan.
- `pnpm lint`: 16 tareas aprobadas. ESLint conserva 135 advertencias de Vue
  existentes (principalmente props y usos de HTML); no se presentan como cero
  advertencias ni se deshabilitaron reglas para ocultarlas. El formato pasa.

Entorno de verificación: Windows x64, Rust 1.95.0, Node 24.19.0 del runtime
incluido y pnpm 10.33.2. El Node global 24.14.0 no se modificó. Las compilaciones
Rust se ejecutaron una a una, con caché incremental desactivada tras observar
un fallo interno de esa caché del compilador.

En la primera revisión, el dev recompilado abrió la ventana Orbiont en Windows y se comprobó que estaba
visible y respondía. La primera muestra de arranque del proceso principal tuvo
consumo alto de CPU; ese pico no se mantuvo en las muestras posteriores.
Tras 118 segundos desde el arranque, una muestra de 10,1 segundos registró
15,9 % de un núcleo lógico (aproximadamente 2 % de los ocho disponibles),
88,85–89,51 MiB de working set y 33,02–33,89 MiB de memoria privada.
No se atribuye el pico a una causa sin un perfil de stacks, ni se presenta esta
muestra como una comparación antes/después de las optimizaciones. Tampoco suma
WebView2 ni certifica los pasos manuales enumerados. El dev queda ejecutándose.
