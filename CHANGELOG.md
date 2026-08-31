# Changelog

Todos los cambios relevantes de Spike se documentan aquí. El formato toma como referencia [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/) y el proyecto sigue [versionado semántico](https://semver.org/lang/es/).

## [Sin publicar]

### Añadido

- Galería de la demo con tema Gruvbox en el README.
- Requisito explícito de Windows Terminal y Fira Mono para la experiencia compatible de Spike.

## [0.1.0] - En desarrollo

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

[Sin publicar]: https://github.com/maaurissio/Spike/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/maaurissio/Spike/releases/tag/v0.1.0
