# Changelog

## [0.3.1] - 2026-09-01

### Cambiado

- Si Windows Terminal y Fira Mono ya están disponibles, Spike abre el dashboard directamente en el proceso actual y repara silenciosamente su copia o perfil cuando hace falta.
- Cuando Riot oculta los PartyID enemigos en vivo, Spike contrasta el último resultado Ranked terminado y exige que los jugadores compartieran el mismo PartyID real antes de mostrarlos como premade.
- Los jugadores de cada equipo en Historial y postpartida se ordenan por ACS descendente.
- La paleta personalizada de desarrollo pasa a ser la paleta oficial predeterminada para los temas Noche y Claro; las instalaciones que aún usaban los colores oficiales anteriores se migran automáticamente sin sobrescribir paletas personalizadas.

### Corregido

- El arranque ya no cierra una primera ejecución para abrir correctamente solo después de intentarlo por segunda vez.
- La partida activa se excluye de la inferencia de premades para que sus diez participantes no se confundan con compañeros de cola.
- La estructura anidada actual de Presence tiene prioridad sobre los campos planos antiguos, evitando mezclar el premade propio con otro grupo del equipo.
- Una actualización completa de Presence reemplaza correctamente las estimaciones provisionales, incluso cuando confirma que un jugador está solo.
- El orden por ACS ya no cruza nombres, agentes, estadísticas ni premades: ahora se aplica únicamente al orden visual del marcador.
- El jugador propio muestra su Riot ID real en Historial y postpartida en lugar de sustituirlo por `Tú`.

## [0.3.0] - 2026-09-01

### Añadido

- Preparación automática del primer arranque: Spike instala Windows Terminal, Fira Mono y su perfil Gruvbox antes de abrir el dashboard.
- Fira Mono se distribuye dentro del ejecutable bajo la SIL Open Font License 1.1, sin depender de una descarga adicional.
- Instalación alternativa desde el paquete estable oficial de Microsoft cuando WinGet no está disponible.
- Detección completa de premades del roster en vivo, incluyendo grupos aliados, enemigos y five-stacks.
- Espera dirigida por los jugadores de la partida para evitar que presencias ajenas oculten grupos reales.
- Actualización progresiva y liviana de premades mientras Riot termina de publicar los grupos enemigos.
- Marcador completo de aliados y enemigos al abrir una partida del historial, con agente, rango, K/D/A, ACS y HS%.
- Riot ID y acceso a Tracker.gg para cada jugador disponible en el marcador histórico.

### Cambiado

- Abrir `spike.exe` sin argumentos ahora repara los requisitos faltantes y se relanza automáticamente en el perfil **SPIKE** de Windows Terminal.
- La observación del estado de VALORANT ahora funciona en un trabajador independiente y continúa mientras se cargan perfiles, historial o estadísticas.
- `F5` guarda los ajustes pendientes antes de relanzar Spike, por lo que conserva el último tema seleccionado.
- El encabezado de actividad en Logs usa el color configurable de títulos.
- Ascendente usa el color `#8ec07c` en los temas Noche y Claro.
- Los premades se identifican únicamente mediante puntos de colores pastel consistentes en la partida en vivo y sus detalles.
- La caché del historial conserva hasta veinte marcadores normalizados y reutiliza los detalles ya descargados.
- La postpartida muestra el mismo marcador completo de jugadores y estadísticas que queda guardado en Historial.
- La demo reproduce el marcador completo actual, los premades, veinte partidas y la misma transición entre Historial y postpartida.

### Corregido

- El identificador de una partida terminada ya no retrasa ni contamina la carga de la siguiente partida.
- Los amigos y otras presencias ajenas al roster ya no adelantan incorrectamente la detección de premades.
- Las presencias antiguas marcadas como inválidas ya no producen grupos incorrectos.
- La información de un five-stack se normaliza como un único grupo de cinco jugadores.
- La selección de jugadores sigue el orden visual de aliados y enemigos sin saltos en Historial, postpartida y partida en vivo.
- El tema seleccionado se guarda automáticamente y los ajustes pendientes terminan de guardarse antes de salir con `q` o `Ctrl+C`.
- Ajustes ya no repite el bloque de privacidad y permite alcanzar **Paleta editable** con ↑/↓ para abrirla con Enter.

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

[0.3.1]: https://github.com/maaurissio/Spike/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/maaurissio/Spike/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/maaurissio/Spike/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/maaurissio/Spike/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/maaurissio/Spike/releases/tag/v0.1.0
