# Maqueta de interfaz

Primera propuesta visual, valorada positivamente por el usuario el 2026-08-28. Referencia para implementar la TUI; no representa funcionalidad ya integrada en Rust.

**Estado del traspaso (2026-08-29):** la presentación está implementada en Rust mediante `vtracker dashboard --demo`, con datos ficticios, sin red ni persistencia. La aplicación conectada ya integra roster real, selección de agente para aliados, nivel, premades normalizados y apertura explícita de Tracker cuando existe un Riot ID público. Las rondas en vivo siguen pendientes de un proveedor de eventos aprobado; la maqueta no demuestra disponibilidad de una API.

**Revisión de terminal:** toda la maqueta se representa con celdas monoespaciadas y bordes de caracteres, no como una web con tarjetas. Una línea por jugador, pestañas textuales y atajos visibles en las cinco vistas. Dos anchos: 72 columnas y 38 columnas para ventanas pequeñas; en la versión compacta se omiten WR y últimas cinco de la tabla. Clics y teclas 1–5 permiten navegar, flechas seleccionan jugadores, Enter abre la selección enfocada, Tab mueve el foco y Esc vuelve a Partida. Sigue siendo una simulación HTML de la TUI, no el ejecutable Rust.

**Revisión de color (2026-08-28):** encabezados integrados en bordes de caracteres, cian para foco, verde para aliados/victorias/kills, coral para enemigos/derrotas/muertes, violeta para Diamante y ámbar para datos pendientes. Las etiquetas siguen siendo suficientes sin color. `t` recorre Sistema, Noche, Claro y Sin color, desde cualquier vista o desde Ajustes. Tras compactar las rondas, la vista de partida ocupa 24 líneas a 72 columnas y 26 a 38 columnas sin detalle expandido. Los diez jugadores y el timeline permanecen visibles.

Fuentes, decisiones de diseño y compatibilidad de imágenes: [REFERENCES.md](REFERENCES.md).

- [`vtracker-maqueta.html`](vtracker-maqueta.html): versión exportada para abrir directamente en un navegador moderno, sin compilar el proyecto.
- [`vtracker-maqueta.source.html`](vtracker-maqueta.source.html): fragmento editable original (estilos, contenido y lógica de interacción).
- [`check-maqueta.cjs`](check-maqueta.cjs): ejecutar con `node docs/mockups/check-maqueta.cjs`. Verifica anchos, altura inicial, diez jugadores, K/D por ronda, datos ocultos, navegación y temas sin dependencias adicionales. Es una prueba lógica con DOM simulado, no un renderizador de terminal.

Incluye Resumen, Partida, Mi perfil, Historial y Ajustes. La vista inicial destaca a los diez jugadores; permite seleccionar jugadores y alternar entre partida y postpartida. Todos los nombres, estados y estadísticas son ficticios. No consulta Riot ni modifica la configuración real. El bloque técnico de fuentes se mantiene fuera de la interfaz final.

Actualización solicitada el 2026-08-28: las rondas propias se muestran **entre aliados y enemigos durante la partida**, con kills y muertes por ronda y acumulados. La ronda en curso aparece pendiente, no con ceros inventados. Postpartida conserva solo el resumen final en esta maqueta. La disponibilidad de datos por ronda durante una partida real sigue pendiente de validación; el diseño no demuestra que una API los entregue.

El timeline usa **tres líneas monoespaciadas**: kills numéricas (`1K`, `4K`), etiquetas (`R1`, `R2`, …) y muertes numéricas (`0D`, `1D`, `2D`). No se apilan letras ni tarjetas. Los ceros representan datos confirmados; la ronda en curso muestra `—K / —D` y `*`. La muestra de siete rondas ocupa 28 columnas, con acumulados 8K/4D calculados desde sus datos.

**Tracker del jugador:** `[↗]` en la columna `TRK`, clic en el detalle o `g` sobre el jugador seleccionado muestra la acción `Abrir Tracker.gg`. La apertura externa está deshabilitada en esta maqueta: no hay Riot IDs reales verificados. En la aplicación conectada se construye el enlace únicamente desde el `GameName#TagLine` canónico resuelto; para `Jugador N` o identidad ausente no se crea ningún enlace.

GitHub muestra el código del HTML en el repositorio. Para interactuar con la maqueta, descarga el archivo exportado y ábrelo en tu navegador. Subir los archivos no publica automáticamente una página web.

Al modificar el fragmento, actualizar también el exportado. La exportación inicial se generó con `scripts/render.py` de la habilidad `visualize`; no se necesita esa herramienta para abrir el HTML ya exportado. Mantener el roster como requisito principal según ADR-011.
