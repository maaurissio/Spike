# VTracker — nombre temporal

> **Aviso:** `VTracker` es un nombre **temporal de desarrollo**. Se elegirá el nombre final antes del release (ver `TASKS.md:Prioridad 6`).

VTracker es una aplicación de terminal para observar el estado de VALORANT, consultar datos autorizados de partidas y jugadores, y presentar estadísticas de forma rápida y con bajo consumo de recursos.

**Objetivo principal confirmado el 2026-08-28:** mostrar aliados y enemigos de la partida (diez jugadores en 5v5), con sus agentes, rangos y estadísticas históricas disponibles y permitidos. El perfil propio, historial y resumen postpartida son complementos. **El roster aún no está implementado:** la versión actual solo muestra contexto propio. Véase [ADR-011](docs/DECISIONS.md#adr-011--roster-de-la-partida-como-requisito-principal-2026-08-28).

Las referencias históricas a acceso mediante tokens locales describen una vía técnica, no una autorización de uso de datos de terceros. Antes de implementar el roster se deben validar fuentes, términos y consentimiento aplicable; las identidades ocultas y datos restringidos se respetan y los campos ausentes se muestran como no disponibles.

El proyecto incluye un MVP de `watch`: detecta de forma no invasiva los procesos locales del cliente/juego y presenta el estado en terminal. Si Riot Client está abierto, comprueba su API local en `127.0.0.1` usando el lockfile y escucha metadatos del WebSocket local; no accede a memoria ni automatiza el juego.

## MVP: `watch`

```powershell
cargo run -- watch
cargo run -- watch --once
cargo run -- watch --interval 5
cargo run -- doctor
```

En una terminal interactiva el panel se actualiza automáticamente. Para probar estados sin ejecutar VALORANT se puede usar `VTRACKER_STATE=closed`, `idle` o `game`.

El detector de procesos por sí solo no puede distinguir lobby, selección de agente o partida. El proveedor local puede elevar la confianza cuando recibe una URI conocida del WebSocket; si no recibe una reciente, el panel conserva el estado honesto de procesos y muestra "modo no confirmado".

La configuración opcional está en `%APPDATA%\vtracker\config.toml`; consulta [`config.example.toml`](config.example.toml) para el formato. Si `log_transitions = true`, los cambios de estado se guardan en `%APPDATA%\vtracker\watch.log`.

`vtracker doctor` revisa la configuración, la consulta de procesos, el lockfile y —cuando el cliente está abierto— los endpoints locales base y el handshake WAMP. Solo informa estado y metadatos seguros; no imprime credenciales, tokens ni payloads de eventos.

Consulta la [lista de tareas](TASKS.md) para el trabajo realizado, prioridades y siguiente paso.

## Dashboard interactivo

Ejecuta `vtracker dashboard` (alias `tui`). Abrir el ejecutable sin argumentos sigue iniciando `watch`.

- `1–5` o `←/→`: cambiar vista. `Tab`/`Shift+Tab`: alternar foco entre pestañas y contenido; `Enter` entra al contenido o abre el detalle seleccionado.
- `↑/↓` selecciona jugadores (demo), partidas o ajustes. `PgUp/PgDn` desplaza el contenido; la selección se mantiene visible en ventanas pequeñas.
- `r`: actualizar datos propios bajo demanda; el historial se carga al entrar en su pestaña.
- `t`: previsualizar Sistema, Noche, Claro o Sin color. En **Ajustes**, `s` guarda el tema junto al intervalo/registro; `r` descarta el borrador. `+/-` edita y `Espacio` alterna el registro o tema seleccionado. Cerrar sin guardar descarta los cambios.
- `Esc`: cerrar detalle o volver a Partida. `q` o `Ctrl+C`: salir y restaurar la terminal.

La presentación nativa sigue [`docs/mockups`](docs/mockups/README.md): bordes de caracteres, cinco vistas, tablas compactas y orden **Aliados → Tus rondas → Enemigos**. A 72 columnas la partida de demostración cabe en 24 filas; a 38 columnas, en 26. En ventanas más bajas se habilita desplazamiento; por debajo de 38×10 se solicita ampliar la terminal.

Para ver la maqueta completa en Rust, sin VALORANT:

```powershell
.\target\release\vtracker.exe dashboard --demo
```

**DEMO** está siempre identificado: todos sus jugadores y estadísticas son ficticios. No crea proveedores reales, no lee configuración personal ni guarda en disco. `p` alterna partida/postpartida, `g` explica por qué el enlace a Tracker no está disponible, `[`/`]` pagina timelines largos. No abre perfiles externos. El HTML original se conserva intacto como referencia.

En modo normal se conservan los datos propios ya disponibles; no se han añadido endpoints. El roster real, marcador y rondas en vivo, estadísticas enriquecidas de Perfil/Historial, resumen de sesión e imágenes siguen pendientes. Se muestran estados vacíos explícitos, nunca fixtures de la demo. Los modos continuos tienen una vista propia sin inventar equipos ni rondas.

Las consultas se ejecutan en un trabajador con colas acotadas y una sola operación a la vez; el teclado sigue disponible durante la carga. Un refresh fallido conserva los datos anteriores de la sesión y avisa en la vista. El resumen postpartida permanece al volver al menú, hasta otra partida o el cierre del cliente. Las respuestas atrasadas de fases/sesiones anteriores se descartan.

Están disponibles para edición el intervalo local (1–60 s), `log_transitions` y `theme` (`"system"`, `"dark"`, `"light"`, `"mono"`). Sistema hereda fondo/texto y colores ANSI del terminal; no detecta el tema de Windows. TTL y autoinicio siguen pendientes. El registro se habilita únicamente después de guardar ese cambio; nunca contiene credenciales. No hay caché de historial en disco ni polling periódico de estadísticas remotas.

## Documentación

### Maqueta interactiva

La [primera maqueta de interfaz](docs/mockups/vtracker-maqueta.html) muestra el roster de diez jugadores, perfil, historial, ajustes y postpartida con datos ficticios. Descarga el HTML y ábrelo en tu navegador para interactuar; GitHub muestra su código, no la interfaz. [Archivos y notas de diseño](docs/mockups/README.md). Su presentación ya está trasladada a Rust en `dashboard --demo`; la integración del roster y sus datos reales sigue pendiente.

| Documento | Contenido |
|---|---|
| [`docs/SPEC-LOCAL-API.md`](docs/SPEC-LOCAL-API.md) | Contrato pre-código de la Local Client API (lockfile, GLZ/PD, WebSocket) |
| [`docs/API-2026.md`](docs/API-2026.md) | **Vigencia 2026** de todas las fuentes: Riot Oficial, Henrik v4, Valorant-API.com y Local (rate limits, opt-in, endpoints) |
| [`docs/AGENTS.md`](docs/AGENTS.md) | Roster completo 2026: 29 agentes, 4 roles (8/7/7/7), UUIDs vigentes, estrategia de mapeo dinámico |
| [`docs/TRACKERS.md`](docs/TRACKERS.md) | Cómo funcionan los trackers vigentes (Tracker.gg/Overwolf, Blitz, Instalock, Vantage) y qué adopta VTracker |
| [`docs/PERFORMANCE.md`](docs/PERFORMANCE.md) | RAM/cache sin BD + mínimo CPU: `moka`/`cache-rs`/`CacheKit`, dirty-flag Ratatui, perfil release |
| [`docs/DECISIONS.md`](docs/DECISIONS.md) | Registro de decisiones (ADRs) |
| [`docs/SPEC-ROUNDS.md`](docs/SPEC-ROUNDS.md) | Spec del tracking de rondas |
| [`docs/DESIGN-UI.md`](docs/DESIGN-UI.md) | Spec de interfaz: LIVE MATCH, PostMatch, layouts adaptativos |
| [`docs/ISO.md`](docs/ISO.md) | Sistema integrado ISO 9001/14001/27001 |
| [`Arquitectura-inicial.md`](Arquitectura-inicial.md) | Arquitectura estructural y decisiones base |

## Fuentes de datos — Local Client API (agilizado)

**Investigación 2026-08-24** (ver `Arquitectura-inicial.md:20`): VALORANT expone una **API local** cuando corre — lockfile en `%LocalAppData%\Riot Games\Riot Client\Config\lockfile` + REST en `127.0.0.1:{port}` + WebSocket `wss://riot:{password}@127.0.0.1:{port}` — y con sus tokens se accede a GLZ/PD. **No se necesita API key de producción ni RSO** para la experiencia principal:

| Dato | Fuente | Cuándo |
|---|---|---|
| Fase (`Lobby`/`PreGame`/`AgentSelect`/`InMatch`/`PostMatch`) | WebSocket local | En vivo, event-driven; mapeo de URIs aún en validación |
| Roster 10 jugadores (ranks, nivel, agente — privados incluidos) | GLZ Pre-Game/Current Game | En vivo |
| Perfil propio, MMR, historial | PD con tokens locales | En vivo/post |
| **Desglose por ronda** (`roundResults[]`: kills/deaths/resultado por ronda) | PD `match-details` | Al terminar la partida |

> **Rondas:** al finalizar la partida el tracker muestra la tabla `Ronda | Resultado | Kills | ¿Moriste?` por cada ronda jugada. Si `match-details` respondiera a mitad de partida se muestra progreso incremental; si no, degrada con elegancia. No se usa OCR ni lectura de memoria.

Lectura solo-lectura (sin inyección ni memoria); el password del lockfile vive solo en memoria y `doctor` lo enmascara (`***`). `RIOT_API_KEY` en `.env` queda **opcional** para mejoras futuras.

### Fuentes consultadas en la investigación (2026-08-24)

> Se listan **todas** las fuentes revisadas durante la investigación, incluidas las que no fue posible acceder completamente (403/404/bloqueo) — quedan como referencia y para reintentar.

**Documentación técnica de la API del cliente (accedidas):**

* **Valorant API Docs (techchrism)** — <https://valapidocs.techchrism.me/> — documentación no oficial de los endpoints internos del cliente: `Local Help`, `Local WebSocket` (`wss://riot:{password}@127.0.0.1:{port}`), `Entitlements Token`, `Pre-Game Match`, `Current Game Match` (`glz-{region}-1.{shard}.a.pvp.net/core-game/v1/matches/{id}`), `Match Details` (`pd.{shard}.a.pvp.net/match-details/v1/matches/{id}` con `roundResults[]` y `playerStats[].kills`), `Match History`, `Sessions`. Fuente principal de esta investigación.
* **Riot Developer Portal — VALORANT** — <https://developer.riotgames.com/docs/valorant> — política oficial: RSO, opt-in de datos personales, casos de uso aprobados (accesible vía resultados de búsqueda; la página raíz `/docs/riot-games` devolvió 404).
* **VAL-MATCH-V1 / endpoints oficiales** — <https://developer.riotgames.com/apis> — índice oficial (Account, Content, Match, Ranked, Status); referenciado vía perfiles de terceros.

**Proyectos de la comunidad (accedidos, evidencia de que la Local Client API funciona):**

* **VALORANT-rank-yoinker (vRY)** — <https://github.com/zayKenyon/VALORANT-rank-yoinker> (563★, desarrollo finalizado) — lee lockfile + WebSocket local para ranks/skins en vivo; respeta streamer mode por política de Riot.
* **Vantage** — <https://github.com/ccjakje/vantage> + `DOCUMENTATION.md` — tracker Rust/Tauri con CLI y roadmap de overlays; su doc detalla lockfile, servidores (`127.0.0.1`, `glz`, `pd`), por qué funcionan perfiles privados, y **per-round stats vía PD `/match-details`**. Su fase "Tab Overlay (per-round stats)" valida nuestro requisito de rondas.
* **ValorantClientAPI (RumbleMike)** — <https://github.com/HeyM1ke/ValorantClientAPI> (414★) — investigación pionera de la API in-game/cliente; Docs con GettingStarted, PlayerID, CompetitiveHistory, MatchHistory.
* **Valorant-Overlay (LuqmanKareem)** — <https://github.com/LuqmanKareem/Valorant-Overlay> — killfeed en vivo vía **OCR** (MSS + OpenCV + Tesseract); evidencia de que NO existe API para kills en vivo (por eso lo descartamos).
* **valorant-api.com** — <https://valorant-api.com/> — assets/contenido (agentes, mapas, armas) no oficial; útil para iconos futuros.
* **HenrikDev API (no oficial)** — <https://docs.henrikdev.xyz/valorant/api-reference/match> — `/valorant/v4/match/{region}/{matchId}` con `rounds[]`/`player_stats[]`; alternativa con key propia (Basic instantánea). No requerida con Local Client API.
* **valorant-mcp** — <https://pypi.org/project/valorant-mcp/> — referencia de tooling sobre Henrik API (timeline narrativo por ronda).
* **yasuo.js** — <https://docs.yasuo.gg/api/val-match> — wrapper de VAL-MATCH-V1; útil como referencia de modelos (`roundResults`, `teams`, `players`).

**Foros y discusiones (acceso parcial/bloqueado):**

* **Reddit r/VALORANT** — hilo *"Live game data API and/or overlay system for OBS"* (<https://www.reddit.com/r/VALORANT/comments/10d53ny/>) — menciona herramientas que leen datos en vivo (Logitech/Touch Portal); Reddit bloqueó el fetch directo (acceso parcial vía buscador).
* **Reddit** — hilo sobre el programa de performance por jugador — confirma que el **lockfile** solo existe con VALORANT abierto y contiene credenciales del servidor local (no la contraseña Riot).
* **DeepWiki vRY** — <https://deepwiki.com/zayKenyon/VALORANT-rank-yoinker/3.2-lockfile-and-local-api> — análisis del flujo lockfile → REST local → WebSocket → GLZ/PD.
* **Stack Overflow** — etiquetas `valorant+api` — devolvió 403 al fetch directo; no accesible en esta sesión.
* **Búsqueda DuckDuckGo** — <https://duckduckgo.com/html/?q=valorant+in-game+live+round+kills+local+api+lockfile+overlay+reddit> — usada para descubrir vRY/Vantage/Valorant-Overlay y la doc del WebSocket local.
* **Google** — búsqueda directa bloqueada (redirect JS); se usó DuckDuckGo como alternativa.

**Conclusiones de la investigación** (detalle en `Arquitectura-inicial.md:20`):

1. La Local Client API (lockfile) cubre fases reales, roster, perfil, historial y rondas **sin API key ni RSO**.
2. `match-details` entrega el desglose por ronda **post-partida** (`PostGameDetails: null` en vivo).
3. Los kills dentro de la ronda en curso **no están expuestos por ninguna API** — solo OCR (frágil) o lectura de memoria (descartada por principios).

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

> El detector (`src/game/mod.rs:75`) solo distingue estas 3 señales. `LocalClientSource` añade fases finas solo tras un evento WebSocket inequívoco y las descarta después de 15 segundos sin actualización, para no presentar una fase antigua como actual.

Tests: `cargo test` — 158 pruebas para `config`, `cli`, `game`, `diagnostics`, `providers` (lockfile, REST local, WAMP, contexto propio en vivo, perfil/MMR, historial, postpartida y agregados por modo/mapa/agente), `analytics`, `cache`, `watch` y TUI.

## Experiencia de usuario final — visión

Estado objetivo una vez completado el desarrollo (Prioridades 2-5):

1. **Al iniciar el PC / abrir VALORANT** — VTracker (nombre final por definir) se inicia en segundo plano si `autostart = true` en `config.toml` (`src/config/mod.rs:1`). No hace polling innecesario; espera eventos del `Game Engine`.
2. **Perfil propio** — al arrancar con cliente disponible muestra tu perfil vinculado (`RIOT_ID` configurado en `config.toml` o `.env`) con stats derivadas (K/D, WR, HS%, ADR/ACS si la fuente los expone) calculadas por `Analytics Engine` desde `Raw Data` cacheada.
3. **Encontrando partida / Agent Select** — mostrar el roster y estadísticas disponibles y permitidos para esa fase, desde fuentes previamente validadas. No anticipar identidades ni datos que la fuente o sus restricciones oculten.
4. **En partida (`InMatch`) — función principal:** mostrar mapa, modo y roster de aliados/enemigos (diez jugadores en 5v5), con agentes, rangos y estadísticas históricas disponibles y permitidos. Actualmente solo está implementado el contexto propio; falta el roster.
5. **Al terminar la partida (`PostMatch`)** — en modos con rondas, desglose `Ronda | Resultado | Kills | Muertes`; en Deathmatch, Team Deathmatch o Escalation, resumen propio final de K/D/A y puntos. Todo desde `match-details` y sin exponer IDs.

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
- **Testing:** fixtures para analytics y postpartida, pruebas de colas/carga, configuración y renderizado; `cargo fmt`/`clippy` en CI.
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
vtracker config show|validate                   # ver/validar %APPDATA%\vtracker\config.toml
vtracker config edit --interval 5 --log-transitions true  # cambio atómico y explícito
vtracker autostart enable|disable|status        # gestionar inicio automático (requiere consentimiento)
vtracker player                                 # nivel y XP del perfil autenticado
vtracker match [id]                              # detalle de partida
vtracker history [--limit 1..20]                # últimas partidas propias, sin IDs
vtracker dashboard                              # interfaz interactiva (alias: tui)
vtracker stats [--limit 1..5]                   # K/D, KDA, win rate y desglose por modo/mapa/agente
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

MVP local y Prioridad 1 completados. **Implementado:** `LocalClientSource` lee el lockfile y valida `GET /help`, entitlements, sesión externa, región/locale y el handshake/suscripción WAMP, exclusivamente en `127.0.0.1`; su listener persistente acepta solo URIs de fase inequívocas y descarta payloads. Los tokens nunca se imprimen ni persisten. GLZ entrega contexto propio de partida y PD entrega perfil propio (nivel, XP, MMR/RR y cambios competitivos), historial propio, postpartida y agregados propios de las últimas 1–5 partidas por modo, mapa y agente. La base de `vtracker dashboard` usa Ratatui/Crossterm y solo renderiza cuando hay cambios. **Siguiente:** validar fuentes y permisos para el roster, implementar aliados/enemigos con rangos y estadísticas disponibles y permitidos (ADR-011), y validar transiciones reales. Nombre `VTracker` temporal hasta release.

**Compromiso ISO:** se adoptan principios **ISO 9001 (Calidad) + ISO 14001 (Ambiental) + ISO 27001 (Seguridad)** como sistema integrado PHVA desde el inicio (ver `docs/ISO.md` y `Arquitectura-inicial.md:21`). No es burocracia vacía: tests + `clippy`/`fmt` (calidad), `cargo` eficiente + caché (ambiental), `.env` + `doctor` enmascarado + RSO (seguridad). Certificación formal opcional a medio plazo.

## Desarrollo

```powershell
cargo run -- watch --once
cargo run -- player
cargo run -- history --limit 5
cargo run -- doctor
cargo test
cargo fmt
cargo check
cargo clippy
```
