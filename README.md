# VTracker

VTracker es una aplicación de terminal para observar el estado de VALORANT, consultar datos autorizados de partidas y jugadores, y presentar estadísticas de forma rápida y con bajo consumo de recursos.

El proyecto incluye un MVP de `watch`: detecta de forma no invasiva los procesos locales
del cliente/juego y presenta el estado en terminal. No consulta APIs ni accede a memoria.

## MVP: `watch`

```powershell
cargo run -- watch
cargo run -- watch --once
cargo run -- watch --interval 5
cargo run -- doctor
```

En una terminal interactiva el panel se actualiza automáticamente. Para probar estados
sin ejecutar VALORANT se puede usar `VTRACKER_STATE=closed`, `idle` o `game`.

El detector de procesos no puede distinguir lobby, selección de agente o partida. Cuando
el ejecutable del juego está activo muestra “modo no confirmado”; no debe interpretarse
como una partida en curso.

La configuración opcional está en `%APPDATA%\\vtracker\\config.toml`; consulta
[`config.example.toml`](config.example.toml) para el formato.

Si `log_transitions = true`, los cambios de estado se guardan en
`%APPDATA%\vtracker\watch.log`.

`vtracker doctor` revisa la configuración, la consulta de procesos y los procesos Riot/VALORANT detectados. Es un diagnóstico local: no consulta APIs ni obtiene información de una partida.

Consulta la [lista de tareas](TASKS.md) para el trabajo realizado, prioridades y siguiente paso.

## Estructura actual

```text
src/
├── main.rs          # CLI y ciclo principal
├── config/          # Configuración y validación
├── diagnostics/     # Comando doctor
├── game/            # Estados y detección local de procesos
├── ui/              # Renderizado de terminal
└── watch/           # Transiciones y persistencia de logs
```

## Objetivos

- Detectar transiciones relevantes del cliente y de una partida de VALORANT.
- Mostrar información mediante una interfaz de terminal navegable.
- Consultar datos desde proveedores intercambiables, como Riot, Tracker u otros que se validen.
- Calcular métricas a partir de datos de partidas, sin mezclar cálculos con la interfaz ni con las APIs.
- Reducir uso de red, CPU y memoria mediante eventos, caché y solicitudes centralizadas.

## Arquitectura resumida

```text
                 VTracker
                     |
   +-----------------+-----------------+
   |                 |                 |
Game Engine      Data Engine        UI Engine
   |                 |                 |
Estado y eventos  Providers, caché   Ratatui y AppState
                  y analytics
```

- **Application Core:** inicia la aplicación, carga configuración, coordina tareas y realiza un apagado limpio.
- **Game Engine:** detecta estados como `Idle`, `PreGame`, `AgentSelect`, `InMatch` y `PostMatch`, y emite eventos cuando cambian.
- **Data Engine:** obtiene, normaliza y guarda datos; incluye proveedores, caché, el gestor de solicitudes y el motor de estadísticas.
- **UI Engine:** renderiza la interfaz desde el estado de la aplicación y gestiona la navegación por teclado.

## Flujo de datos

```text
Provider -> Request Manager -> Cache -> Raw Data
                                         |
                                         v
                                  Analytics Engine
                                         |
                                         v
                                  Derived Stats -> AppState -> TUI
```

Los datos originales (*raw data*) se conservan separados de las métricas calculadas (*derived data*). Así es posible revisar el origen de un resultado y recalcularlo si cambia una fórmula.

Ejemplos de estadísticas previstas: K/D, porcentaje de headshots, win rate, ADR, ACS, KAST, rachas, rendimiento por agente y rendimiento por mapa. Las fórmulas finales dependerán de los campos que exponga cada fuente de datos.

## Principios técnicos

- Rust como lenguaje principal: binario nativo, concurrencia segura y uso eficiente de recursos.
- Tokio para tareas asíncronas.
- Ratatui y Crossterm para la interfaz de terminal.
- Reqwest y Serde para HTTP, JSON y modelos tipados.
- TOML para configuración; Clap para comandos; Tracing para logs.
- Caché en dos niveles: L1 en memoria y L2 en disco.
- Request Manager con deduplicación, prioridades, límites de solicitudes, timeout, reintentos, backoff y cancelación.
- Diseño orientado a eventos para evitar polling y cálculos innecesarios.

## Interfaz prevista

La TUI tendrá las vistas:

- Dashboard
- Match
- Team / Player
- History
- Settings

La interfaz debe adaptarse a terminales pequeñas y grandes, sin depender de una resolución fija.

## Comandos previstos

```text
vtracker watch
vtracker player <riot-id>
vtracker match [id]
vtracker history [player]
vtracker cache <subcomando>
vtracker config <subcomando>
vtracker doctor
```

## Plan inicial

1. Validar fuentes de datos, autenticación, límites de uso y políticas aplicables.
2. Crear el MVP de `vtracker watch`: configuración, logs, detección de estado y TUI mínima.
3. Añadir el primer proveedor y la caché.
4. Implementar Analytics Engine y métricas básicas.
5. Completar navegación, pantallas y diagnóstico.
6. Medir CPU, RAM, tiempos de arranque y rendimiento de caché antes de optimizar.

## Estado actual

La base Rust ya está creada. El siguiente paso es validar cómo obtener de forma permitida y fiable el estado de VALORANT antes de implementar detectores y proveedores.

## Desarrollo

```powershell
cargo run
cargo test
cargo fmt
cargo clippy
```
