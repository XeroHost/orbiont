# GLib 0.18.5: corrección compatible de RUSTSEC-2024-0429

Orbiont conserva la API GLib 0.18 que requiere la cadena GTK3/WebKitGTK de Tauri.
`Cargo.toml` resuelve `glib` desde `patches/glib-0.18.5`; no cambia su versión ni
simula una actualización a GLib 0.20.

La copia parte del paquete publicado `glib 0.18.5` de crates.io, cuyo SHA-256 es
`233daaf6e83ae6a12a52055f568f9d7cf4671dabb78ff9560ab6da230ce00ee5`.
Su `.cargo_vcs_info.json` identifica el commit original
`42b9caf98e03ded086362d9653ca58fe94dc8658`. Se conservan todos los archivos
publicados, incluidos LICENSE (MIT), COPYRIGHT y avisos de autoría. `.cargo-ok`
es un marcador local de Cargo y no forma parte de la copia.

La única diferencia respecto del paquete original son dos líneas de
`src/variant_iter.rs`: `let p` pasa a `let mut p`, y el argumento de salida `&p`
pasa a `&mut p`. Es la corrección oficial del commit
[`b5a4071e439bef2b5eea76c3aa25e5ae84839e34`](https://github.com/gtk-rs/gtk-rs-core/commit/b5a4071e439bef2b5eea76c3aa25e5ae84839e34)
([PR #1343](https://github.com/gtk-rs/gtk-rs-core/pull/1343)). Evita que la función C
variádica escriba a través de una referencia Rust inmutable y que una compilación
optimizada conserve un puntero nulo. El
[aviso RustSec](https://rustsec.org/advisories/RUSTSEC-2024-0429.html)
documenta los cinco métodos afectados de `VariantStrIter`.

`patches/glib-0.18.5.integrity.json` registra SHA-256 de los 121 archivos corregidos,
del archivo original modificado y del archivo de publicación. El verificador fija
también el hash del propio manifiesto de integridad. Antes de exceptuar **sólo**
`RUSTSEC-2024-0429`, `scripts/check-cargo-advisories.mjs` exige que todos los bytes
coincidan, sin archivos adicionales, ausentes o enlaces simbólicos, y que Cargo
resuelva una única copia de GLib 0.18.5 desde esta ruta local. Otro origen, versión
o cambio de fuente hace fallar la comprobación. La excepción RSA existente sigue
dependiendo de su ausencia en el grafo normal/build de todas las plataformas;
los demás avisos permanecen visibles.

La versión del lockfile sigue dentro del rango de RustSec. Los escáneres basados
sólo en versiones pueden seguir notificándolo; cargo-audit 0.22.2 omite los
avisos de este paquete porque su origen ya es local. La comprobación del proyecto
exige igualmente verificar la fuente y acepta la corrección por esos bytes, no
por una versión inventada. Los avisos informativos sobre mantenimiento de GTK3
no quedan resueltos por este parche.

El harness `patches/glib-regression` es un proyecto aislado y usa la copia local
sin los paquetes de desarrollo de GLib. Prueba `next`, `next_back`, `nth`,
`nth_back` y `last` con cadenas Rust propias, UTF-8, vacías, largas y operaciones
combinadas; también comprueba agotamiento y saltos con overflow. El workflow
`glib-backport.yml` ejecuta las pruebas con `--release` y GLib nativo en Linux:

```sh
node --test scripts/glib-backport-integrity.test.mjs
cargo test --locked --release --manifest-path patches/glib-regression/Cargo.toml
node scripts/check-cargo-advisories.mjs
```

Windows permite comprobar la integridad y la resolución del grafo, pero no valida
la ejecución GLib/Linux. La prueba optimizada de Linux requiere ese entorno y
no debe presentarse como ejecutada hasta que el workflow termine correctamente.
Para retirar el parche, primero debe actualizarse la cadena Tauri/GTK a una
versión compatible con una publicación de GLib corregida, y repetirse las pruebas
nativas y el audit sin esta excepción.
