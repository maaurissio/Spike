# VTracker — nombre temporal

> **Aviso:** `VTracker` es un nombre **temporal de desarrollo**. Se elegirá el nombre final antes del release (ver `TASKS.md:Prioridad 6`).

VTracker es una aplicación de terminal para observar el estado de VALORANT, consultar datos autorizados de partidas y jugadores, y presentar estadísticas de forma rápida y con bajo consumo de recursos.

El proyecto incluye un MVP de `watch`: detecta de forma no invasiva los procesos locales del cliente/juego y presenta el estado en terminal. No consulta APIs ni accede a memoria.

## MVP: `watch`

```powershell
cargo run -- watch
cargo run -- watch --once
cargo run -- watch --interval 5
cargo run -- doctor
```

En una terminal interactiva el panel se actualiza automáticamente. Para probar estados sin ejecutar VALORANT se puede usar `VTRACKER_STATE=closed`, `idle` o `game`.

El detector de procesos no puede distinguir lobby, selección de agente o partida. Cuando el ejecutable del juego está activo muestra "modo no confirmado"; no debe interpretarse como una partida en curso.

La configuración opcional está en `%APPDATA%\vtracker\config.toml`; consulta [`config.example.toml`](config.example.toml) para el formato. Si `log_transitions = true`, los cambios de estado se guardan en `%APPDATA%\vtracker\watch.log`.

`vtracker doctor` revisa la configuración, la consulta de procesos y los procesos Riot/VALORANT detectados. Es un diagnóstico local: no consulta APIs ni obtiene información de una partida.

Consulta la [lista de tareas](TASKS.md) para el trabajo realizado, prioridades y siguiente paso.

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
   * `config.toml` → solo intervalo, `log_transitions`, `autostart`, `profile` y futuras opciones no sensibles (ver `config.example.toml:6`)
4. **Estructura futura:**
   ```text
   src/providers/      # traits GameStateSource / capabilities
   src/requests/       # Request Manager central (dedupe, rate-limit, backoff)
   src/cache/          # L1 RAM + L2 disco
   ```
   Ningún módulo de UI hará HTTP directo; todo pasa por el Request Manager.

## Estructura actual

```text
src/
├── main.rs          # CLI y ciclo principal
├── cli/             # Parsing de argumentos y validación
├── config/          # Configuración y validación (TOML + env)
├── diagnostics/     # Comando doctor
├── game/            # Estados y detección local de procesos
├── providers/       # GameStateSource trait + Process/Mock (desacoplado)
├── ui/              # Renderizado de terminal
└── watch/           # Transiciones y persistencia de logs
```

Estructura objetivo (Prioridad 2+):

```text
src/
├── autostart/       # Registro en inicio (Windows Run key / Startup folder)
├── providers/       # capabilities.rs + riot.rs/tracker.rs (autorizados)
├── requests/        # manager.rs (dedupe, rate-limit, retry)
├── cache/           # memory.rs + disk.rs (L1/L2)
└── analytics/       # combat.rs, aggregates.rs
```

## Procesos observados (MVP local)

Verificación con `vtracker doctor` y `VTRACKER_STATE` en Windows (`tasklist /FO CSV /NH`):

| Estado mostrado | Señal | Procesos típicos | `client_found` | `game_found` |
|---|---|---|---|---|
| `Cliente cerrado` | `tasklist` sin coincidencias | *(ninguno Riot/VALORANT)* | false | false |
| `Cliente disponible` | `RiotClientServices` o `valorant` | `RiotClientServices.exe`, `RiotClientCrashHandler.exe` | true | false |
| `Cliente y juego abiertos (modo no confirmado)` | `VALORANT-Win64-Shipping` | `VALORANT-Win64-Shipping.exe` + procesos cliente | true | true |

> El detector (`src/game/mod.rs:75`) solo distingue estas 3 señales. No infiere `Lobby`/`AgentSelect`/`InMatch`; para eso se requiere una fuente autorizada (ver `TASKS.md`).

Tests: `cargo test` — 58 pruebas para `config`, `cli`, `game`, `diagnostics`, `providers` (`GamePhase`, `Mock`/`Process`, fallback) y `watch`.

## Experiencia de usuario final — visión

Estado objetivo una vez completado el desarrollo (Prioridades 2-5):

1. **Al iniciar el PC / abrir VALORANT** — VTracker (nombre final por definir) se inicia en segundo plano si `autostart = true` en `config.toml` (`src/config/mod.rs:1`). No hace polling innecesario; espera eventos del `Game Engine`.
2. **Perfil propio** — al arrancar con cliente disponible muestra tu perfil vinculado (`RIOT_ID` configurado en `config.toml` o `.env`) con stats derivadas (K/D, WR, HS%, ADR/ACS si la fuente los expone) calculadas por `Analytics Engine` desde `Raw Data` cacheada.
3. **Encontrando partida / Agent Select** — al detectar `PreGame`/`AgentSelect` vía `GameStateSource` autorizado (no por procesos), consulta roster del lobby y muestra stats del equipo (capability `RosterSource` + `MatchHistorySource`) con último estado conocido si la API falla.
4. **En partida (`InMatch`)** — muestra contexto vivo del game actual: mapa, modo, composición y stats agregadas de sesión. Datos crudos separados de métricas derivadas (`Arquitectura-inicial.md:7` pipeline).

La TUI siempre es opcional: `vtracker watch` para modo live, y comandos `player`/`match`/`history` para consultas puntuales.

## Autoinicio (autostart) — diseño previsto

> Implementación en Prioridad 6 (Robustez y distribución). Desactivado por defecto.

* **Windows:** `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run` o carpeta `Startup` (`%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup`). Librería recomendada: [`auto-launch`](https://crates.io/crates/auto-launch) (soporta Windows/macOS/Linux, usa `AutoLaunchBuilder`).
* **Alternativa Tauri:** `tauri-plugin-autostart` si se migra a Tauri v2 (mismo principio, builder con `args: ["--minimized"]`).
* **Comportamiento:** `config.toml` tendrá:
  ```toml
  [autostart]
  enabled = false
  minimized = true   # arranca sin robar foco
  ```
  Y CLI: `vtracker autostart enable|disable|status` (gestiona el registro del sistema, nunca silencioso sin consentimiento). `doctor` mostrará `Autostart: habilitado/deshabilitado` sin tocar registro.
* **Buena práctica:** el autostart lo configura el instalador o un comando explícito del usuario; nunca se activa solo por ejecutar el binario (principio de *least surprise*, MITRE T1547.001 como referencia de mecanismo).

## Objetivos

- Detectar transiciones relevantes del cliente y de una partida de VALORANT sin inyección ni lectura de memoria.
- Mostrar información mediante una interfaz de terminal navegable y adaptable.
- Consultar datos desde proveedores intercambiables, como Riot, Tracker u otros que se validen.
- Calcular métricas a partir de datos de partidas, sin mezclar cálculos con la interfaz ni con las APIs.
- Reducir uso de red, CPU y memoria mediante eventos, caché y solicitudes centralizadas.
- Ser configurable y operable: `config.toml`, `doctor` y autostart explícito.

## Arquitectura resumida

```text
                 VTracker (nombre temporal)
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

## Principios técnicos, buenas prácticas y conformidad

Inspirado en patrones 2026 para TUIs Rust (Elm Architecture / TEA, `ratatui` + `tokio::select!`, component-based) y en **sistema integrado ISO 9001/14001/27001** (ver `docs/ISO.md`):

- **Rust como lenguaje principal:** binario nativo, concurrencia segura y uso eficiente de recursos.
- **Tokio para tareas asíncronas** y `tracing` para observabilidad.
- **Ratatui y Crossterm** para la interfaz de terminal.
- **Reqwest y Serde** para HTTP, JSON y modelos tipados.
- **TOML para configuración; Clap para comandos; Tracing para logs.**
- **Caché en dos niveles:** L1 en memoria y L2 en disco (versionado, TTL).
- **Request Manager** con deduplicación, prioridades, límites, timeout, reintentos, backoff y cancelación.
- **Diseño orientado a eventos** para evitar polling y cálculos innecesarios.
- **Separación estricta:** `AppState` solo datos de presentación; I/O y cálculos fuera del renderizado (Elm: Model → Message → Update → View).
- **Autostart explícito** con `auto-launch` crate, nunca implícito.
- **Testing primero:** 58 tests unitarios actuales, fixtures para analytics, `cargo fmt`/`clippy` en CI.
- **Seguridad (ISO 27001):** secretos solo en `.env`/env vars, `.gitignore` estricto, `doctor` enmascara claves (`***`), `cargo audit`+SBOM futuro, controles 8.25-8.29.
- **Calidad (ISO 9001):** control documental (`TASKS.md`/`Arquitectura-inicial.md`), trazabilidad Raw/Derived, `watch.log` como registro, medición antes de optimizar.
- **Ambiental (ISO 14001):** green coding — event-driven, L1/L2 para evitar red, `minimized` en autostart, binario Rust ligero; medición de CPU idle/mem en `docs/BENCHMARKS.md` futuro.

Referencias: Ratatui Elm Architecture, `auto-launch` crate, Riot Developer Portal (RSO/consentimiento), `Arquitectura-inicial.md:20-21`, `docs/ISO.md`.

## Interfaz prevista

La TUI tendrá las vistas:

- Dashboard (perfil + estado + resumen sesión)
- Match (contexto de partida y mapa/modo)
- Team / Player (roster + stats de equipo con último dato conocido si falla provider)
- History (filtros, tendencia y desglose por periodo)
- Settings (configuración, provider, TTL, apariencia, autostart, diagnóstico)

La interfaz debe adaptarse a terminales pequeñas y grandes, sin depender de una resolución fija.

## Comandos previstos

```text
vtracker watch [--once] [--interval SEGUNDOS]   # modo live (MVP actual)
vtracker doctor                                  # diagnóstico local + providers (sin exponer secretos)
vtracker config show|edit|validate              # ver/editar/validar %APPDATA%\vtracker\config.toml
vtracker autostart enable|disable|status        # gestionar inicio automático (requiere consentimiento)
vtracker player <riot-id>                       # perfil autorizado (requiere RSO/opt-in)
vtracker match [id]                              # detalle de partida
vtracker history [player]                        # historial propio autorizado
vtracker cache <subcomando>                      # inspeccionar/limpiar caché L1/L2
```

## Plan inicial

1. Validar fuentes de datos, autenticación, límites de uso y políticas aplicables (RSO y opt-in).
2. Crear el MVP de `vtracker watch`: configuración, logs, detección de estado y TUI mínima. **✓ Hecho**
3. Añadir el primer proveedor y la caché (con `GameStateSource` desacoplado).
4. Implementar Analytics Engine y métricas básicas.
5. Completar navegación, pantallas, `config` y `autostart`.
6. Medir CPU, RAM, tiempos de arranque y rendimiento de caché antes de optimizar.

## Estado actual y conformidad

MVP local y Prioridad 1 completados (58 tests, tabla de procesos, `doctor` testeable, `.env` protegido, `GameStateSource` desacoplado con `Process`/`Mock` y `resolve_with_fallback`). Siguiente: **validar fuente autorizada (Riot docs)** y luego implementar adaptador Riot (`src/providers/riot.rs`) tras `2B`. No se infiere estado de partida desde procesos locales. Nombre `VTracker` temporal hasta release.

**Compromiso ISO:** se adoptan principios **ISO 9001 (Calidad) + ISO 14001 (Ambiental) + ISO 27001 (Seguridad)** como sistema integrado PHVA desde el inicio (ver `docs/ISO.md` y `Arquitectura-inicial.md:21`). No es burocracia vacía: tests + `clippy`/`fmt` (calidad), `cargo` eficiente + caché (ambiental), `.env` + `doctor` enmascarado + RSO (seguridad). Certificación formal opcional a medio plazo.

## Desarrollo

```powershell
cargo run -- watch --once
cargo run -- doctor
cargo test
cargo fmt
cargo check
cargo clippy
```
