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

## Configuración y secretos (API) — protegido por diseño

> **La API se implementa en Prioridad 2** (`TASKS.md:29`). Antes de pedir cualquier key ya está preparada la protección.

1. **Plantilla:** copia `.env.example` a `.env` (nunca commitees `.env`):
   ```powershell
   Copy-Item .env.example .env
   # edita .env y pega tu RIOT_API_KEY solo cuando valides el provider
   ```
2. **Protegido:** `.env`, `.env.local`, `*.log` y `config.toml` real están en `.gitignore:4`. `doctor` nunca imprimirá una key completa (muestra `***` o `no configurada`).
3. **Donde van los secretos:**
   * `RIOT_API_KEY` y futuras `*_API_KEY` → solo en variable de entorno / `.env` (ver `.env.example:9`)
   * `config.toml` → solo intervalo, `log_transitions` y futuras opciones no sensibles (ver `config.example.toml:6`)
4. **Estructura futura:**
   ```text
   src/providers/      # traits GameStateSource / capabilities
   src/requests/       # Request Manager central (dedupe, rate-limit, backoff)
   src/cache/          # L1 RAM + L2 disco
   ```
   Ningún módulo de UI hará HTTP directo; todo pasa por el Request Manager.

Consulta la [lista de tareas](TASKS.md) para el trabajo realizado, prioridades y siguiente paso.

## Estructura actual

```text
src/
├── main.rs          # CLI y ciclo principal
├── cli/             # Parsing de argumentos y validación
├── config/          # Configuración y validación
├── diagnostics/     # Comando doctor
├── game/            # Estados y detección local de procesos
├── ui/              # Renderizado de terminal
└── watch/           # Transiciones y persistencia de logs
```

## Procesos observados (MVP local)

Verificación con `vtracker doctor` y `VTRACKER_STATE` en Windows (`tasklist /FO CSV /NH`):

| Estado mostrado | Señal | Procesos típicos | `client_found` | `game_found` |
|---|---|---|---|---|
| `Cliente cerrado` | `tasklist` sin coincidencias | *(ninguno Riot/VALORANT)* | false | false |
| `Cliente disponible` | `RiotClientServices` o `valorant` | `RiotClientServices.exe`, `RiotClientCrashHandler.exe` | true | false |
| `Cliente y juego abiertos (modo no confirmado)` | `VALORANT-Win64-Shipping` | `VALORANT-Win64-Shipping.exe` + procesos cliente | true | true |

> El detector (`src/game/mod.rs:75`) solo distingue estas 3 señales. No infiere `Lobby`/`AgentSelect`/`InMatch`; para eso se requiere una fuente autorizada (ver `TASKS.md`).

Tests: `cargo test` — 43 pruebas para `config`, `cli`, `game::observation_from_process_list`, `diagnostics::find_riot_processes` y `watch`.

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

MVP local y Prioridad 1 completados (43 tests, tabla de procesos, `doctor` testeable). Siguiente: **Prioridad 2A — seguridad API** (`.env` ya protegido) y **2B — validar fuente autorizada** antes de implementar `GameStateSource`. No se infiere estado de partida desde procesos locales.

## Desarrollo

```powershell
cargo run
cargo test
cargo fmt
cargo clippy
```
