# Referencias de diseño TUI e imágenes

Consultadas el 2026-08-28. Referencias primarias; no se ha copiado código ni distribuido imágenes de esos proyectos. Los datos de VTracker siguen siendo ficticios y no hay consultas a Riot en la maqueta.

## Referencias visuales

| Referencia | Qué adoptamos para VTracker |
|---|---|
| [btop](https://github.com/aristocratos/btop#screenshots) | Densidad de información, títulos en los bordes, gráficos con caracteres y color por significado. Su captura oficial fue inspeccionada visualmente. |
| [Lazygit](https://github.com/jesseduffield/lazygit) | Navegación por teclado y foco de selección visible; atajos contextuales, sin botones grandes. |
| [Yazi](https://yazi-rs.github.io/docs/image-preview/) | Separar la lista compacta de una vista de detalle; tratar el soporte gráfico como una capacidad del terminal. |
| [Demo oficial de Ratatui](https://ratatui.rs/examples/apps/demo/) | Usar tablas, bordes, pestañas y barras que se puedan representar en celdas de terminal. |

Estas aplicaciones son referencias de composición, no plantillas vinculantes. El orden confirmado por el usuario sigue siendo Aliados → Tus rondas → Enemigos.

## Color y tamaño

- La interfaz usa [Gruvbox](https://github.com/morhetz/gruvbox) como sistema cromático. Noche corresponde a Gruvbox Dark de contraste medio (`dark0 #282828`, `light1 #ebdbb2`) y Claro a Gruvbox Light (`light0 #fbf1c7`, `dark1 #3c3836`). Se emiten valores RGB verdaderos para no depender de cómo el host reasigne ANSI 0–15.
- Aqua: navegación y foco. Verde: aliados, victorias y kills. Rojo: enemigos, derrotas y muertes. El rango usa una familia Gruvbox diferenciada, siempre acompañada por el nombre. Amarillo: dato oculto o pendiente.
- Riot explica que revisó la iconografía para distinguir mejor los colores de cada rango: [Rangos y modo competitivo](https://playvalorant.com/es-es/news/dev/los-rangos-y-el-modo-competitivo-de-valorant/). La [versión 5.0](https://playvalorant.com/es-es/news/game-updates/notas-de-la-version-5-0-de-valorant/) introdujo Ascendente entre Diamante e Inmortal. El catálogo comunitario de assets consultado el 2026-08-29 expone los tonos actuales por tier: [Competitive tiers](https://valorant-api.com/v1/competitivetiers). Se contrastó la familia y su iconografía, sin tratar ese espejo como documentación oficial de Riot.
- Familias en Noche: Hierro `#928374`, Bronce `#fe8019`, Plata `#d5c4a1`, Oro `#fabd2f`, Platino `#8ec07c`, Diamante `#83a598`, Ascendente `#b8bb26`, Inmortal `#d3869b` y Radiante `#fabd2f`. Claro usa las variantes faded oficiales cuando el tono brillante no alcanza contraste.
- Fondos, bordes y texto secundarios discretos; un único renglón resaltado indica la selección. Sin degradados, sombras, tarjetas ni retratos que eleven todas las filas.
- Sistema, Noche, Claro y Sin color. El teclado y los marcadores `›`, `V`, `D`, `K` y `—` no dependen de reconocer colores.
- 72 columnas: roster con rango, K/D, WR y últimas cinco. 38 columnas: conservar jugador, agente, rango y K/D; WR queda en el detalle.
- 24/26 líneas en partida, sin detalle abierto, gracias a las cantidades K/D en tres filas. Para una terminal con menos filas, la implementación real necesitará desplazamiento o vistas compactas; no debe recortar información silenciosamente. Para muchas rondas, paginar el timeline por ancho. Estas dos adaptaciones aún no están implementadas en Rust ni simuladas en esta muestra de siete rondas.
- No exigir Nerd Fonts ni emojis para información esencial. El ancho real de Unicode debe comprobarse en los terminales objetivo.

## Tipografía

Una TUI no elige de forma portable la fuente: Ratatui escribe caracteres y estilos, mientras el emulador decide cómo dibujarlos. Las [secuencias de terminal virtual de Windows](https://learn.microsoft.com/es-es/windows/console/console-virtual-terminal-sequences) cubren cursor, color y atributos, pero no una selección de familia tipográfica. Windows Terminal configura `font.face`, tamaño y peso en el perfil, según su [documentación oficial de apariencia](https://learn.microsoft.com/en-us/windows/terminal/customize-settings/profile-appearance). La investigación y las alternativas de integración están en [`docs/TERMINAL-APPEARANCE.md`](../TERMINAL-APPEARANCE.md).

Gruvbox muestra **Fira Mono** en sus capturas y **Fantasque Sans Mono** en su galería. En este equipo solo están disponibles Cascadia Mono, Cascadia Code y Consolas entre las candidatas inspeccionadas. VTracker no modifica la fuente global ni instala tipografías sin una decisión explícita del usuario. Los glifos esenciales mantienen alternativa textual y no requieren Nerd Fonts.

## ¿Se pueden poner imágenes?

Sí, si el **emulador de terminal** y el programa admiten un protocolo gráfico compatible. PowerShell es el shell; elegirlo no demuestra soporte de imágenes.

- Microsoft anunció **Sixel en Windows Terminal Preview 1.22** en agosto de 2024: [anuncio oficial](https://devblogs.microsoft.com/commandline/windows-terminal-preview-1-22-release/). No implica que toda consola de Windows o todo terminal integrado lo soporte.
- La guía de [Yazi para Windows](https://yazi-rs.github.io/docs/image-preview/#windows-users) identifica Windows Terminal >= 1.22.10352.0 entre los terminales compatibles con su previsualizador. Ese requisito de Yazi no certifica una integración propia de VTracker.
- [ratatui-image](https://github.com/ratatui/ratatui-image) proporciona selección de protocolo Sixel, Kitty o iTerm2 y una alternativa con caracteres de medio bloque. Su compatibilidad depende del terminal, backend, fuente y entorno; debe probarse en nuestra combinación concreta.

**Propuesta, pendiente de aprobación e implementación:** un retrato pequeño del agente o una insignia en Perfil/detalle, opcional y con espacio suficiente. La tabla principal conserva una línea por jugador. Detectar capacidades, conservar siempre nombre/agente en texto y permitir desactivar imágenes. Si no se detecta soporte fiable o falta espacio, usar texto; no fallar ni ocultar estadísticas. Evitar animaciones continuas y cachéar/redimensionar fuera del render.

La maqueta no incorpora retratos ni prueba un protocolo gráfico. Tampoco se ha verificado la versión del terminal del usuario. No se agregaron dependencias Rust.

## Verificación de esta revisión

- `node docs/mockups/check-maqueta.cjs`: filas de ancho exacto 72/38, altura inicial 24/26, roster completo, timeline numérico 8K/4D, jugador oculto, acceso simulado a Tracker sin enlaces reales, cinco vistas, cambios de tema, teclas y selección de historial.
- El HTML exportado se regenera desde el fragmento y se comprueba que lo incluya íntegro.
- La revisión visual del archivo local mediante Browser quedó bloqueada por la política de URL. No se intentó una vía alternativa para eludirla; las comprobaciones de esta revisión son estáticas y de lógica, no una validación visual ni en una terminal real.

## Tracker.gg del jugador

La [página oficial de Valorant Tracker](https://tracker.gg/valorant) ofrece búsqueda por Riot ID. El soporte de Tracker muestra la ruta `https://tracker.gg/valorant/profile/riot/{RiotID-codificado}/overview` y aclara que `#` debe codificarse como `%23`: [ejemplo del soporte](https://feedback.tracker.gg/t/cant-access-valorant-profile-via-url/17729). Referencias consultadas el 2026-08-28; no se visitaron perfiles de jugadores para probar el diseño.

Implementación Rust: construye la ruta sobre el dominio fijo `https://tracker.gg`, codifica como segmento el Riot ID completo visible y la abre solo por la acción explícita `g`; no ejecuta texto del proveedor como comando ni acepta dominios arbitrarios. `Jugador N` y datos ausentes no generan enlace. La maqueta sigue sin fabricar Riot IDs ni URLs: allí `g`/`[↗]` permanece deshabilitado y explica el motivo.
