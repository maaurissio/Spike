# Changelog

## [0.2.1] - 2026-08-31

### Añadido

- Colores independientes para el logo inicial, CPU, RAM, barras de RR y niveles de Logs en los temas Noche y Claro.
- Acceso **Paleta editable** en Ajustes para abrir directamente la carpeta de `palette.toml` mediante Enter o clic.

### Cambiado

- `F5` ahora restaura la terminal y relanza Spike con el mismo modo y argumentos para aplicar toda la paleta desde el arranque.
- `palette.toml` se normaliza con las mismas claves y el mismo orden en `[dark]` y `[light]`, usando `body` en ambos bloques y sin comentarios generados.

## [0.2.0] - 2026-08-31

### Añadido

- Galería de la demo con tema Gruvbox en el README.
- Requisito explícito de Windows Terminal y Fira Mono para la experiencia compatible de Spike.
- Archivo `%APPDATA%\spike\palette.toml` para personalizar la interfaz sin recompilar.
- Recarga de la paleta mediante `F5`.
- Paletas independientes para los temas Gruvbox Noche y Gruvbox Claro.

### Cambiado

- Los colores semánticos ahora separan fondo, texto del contenido, títulos, foco, bordes, selección, rangos, advertencias, victorias y derrotas.
- Encabezados como **SPIKE**, **MI PERFIL**, **ÚLTIMAS 5 RANKED** y **ESTADO DE PARTIDA** usan `title`, mientras el contenido normal usa `body` y los controles destacados usan `primary`.
- Los archivos de paleta del formato inicial se migran automáticamente a secciones explícitas `[dark]` y `[light]` sin perder los colores personalizados.

## [0.1.0] - 2026-08-31

### Añadido

- Dashboard de terminal para VALORANT con Resumen, Mi perfil, Historial, Ajustes y Logs.
- Vista contextual de partida durante selección de agente, partida o postpartida.
- Historial propio de hasta veinte partidas Ranked y gráfico de ganancias/pérdidas de RR.
- Resumen por agente en Mi perfil.
- Métricas locales de CPU, RAM, uptime, picos de sesión y actividad sanitizada.
- Demo local reproducible con datos ficticios mediante `spike dashboard --demo`.
- Perfil aislado de Windows Terminal con paleta Gruvbox Dark y fuente Fira Mono.
- Licencia MIT.

### Seguridad y privacidad

- Operación de solo lectura: no se lee memoria del juego, no se inyecta código ni se automatizan controles.
- No se guardan credenciales de sesión; se respetan datos ausentes e identidades ocultas.

[0.2.1]: https://github.com/maaurissio/Spike/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/maaurissio/Spike/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/maaurissio/Spike/releases/tag/v0.1.0
