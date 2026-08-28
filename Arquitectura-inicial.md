# VTracker — Arquitectura y especificación inicial — nombre temporal

> **Alcance vigente (2026-08-28):** el roster de aliados/enemigos con rangos y estadísticas disponibles y permitidos es el requisito principal, confirmado en `docs/DECISIONS.md`, ADR-011. La implementación de datos propios es parcial. Las hipótesis históricas de acceso técnico de este documento no sustituyen la validación de permisos, términos y consentimiento para datos de terceros.

> Estado: borrador de arquitectura.  
> Alcance: decisión y diseño inicial; **no es una implementación**.  
> Convención: **Decidido** refleja acuerdos ya tomados. **Propuesta** indica una dirección recomendada que debe validarse antes de construirla.  
> **Nombre:** `VTracker` es **temporal de desarrollo**. El nombre final se definirá antes del release y se propagará a `Cargo.toml`, binario, README y docs (ver `TASKS.md:Prioridad 6`).

## 1. Resumen del proyecto

VTracker será una aplicación de terminal (TUI) para observar el estado de VALORANT, consultar datos permitidos de jugadores y partidas, y presentar estadísticas de forma rápida, clara y con un consumo reducido de recursos.

Sus objetivos son:

- Detectar el ciclo de estado de VALORANT y reaccionar ante transiciones relevantes.
- Consultar datos mediante una capa de proveedores intercambiable.
- Evitar solicitudes repetidas y trabajo innecesario mediante caché y un gestor central de solicitudes.
- Separar los datos originales de las métricas calculadas.
- Ofrecer una TUI navegable para el seguimiento de partidas, equipo/jugadores, historial y **configuración**.
- Ser medible y operable: logs, diagnóstico y benchmarks desde el inicio.
- **Experiencia por fase:** al iniciar mostrar perfil propio; al encontrar partida mostrar stats del equipo; en partida mostrar stats generales del game — cada flujo activado por `GameStateSource` autorizado, no por procesos.
- **Operación desatendida:** apartado de configuración con **autostart** opt-in (iniciar al encender PC / al abrir VALORANT) con minimizado opcional.

### Límites de producto y cumplimiento

**Decidido:** la arquitectura no dependerá de inyección, lectura de memoria ni automatización que interfiera con el juego. El uso de APIs y datos debe validarse contra los términos y políticas vigentes de Riot y de cada proveedor antes de distribuir el producto.

**Pendiente:** Riot ha restringido casos de *scouting* previo de oponentes. Antes de definir una función pública de análisis de rivales, hay que confirmar que el caso de uso, permisos y consentimiento requerido estén permitidos. La primera versión debe centrarse en datos propios, de sesión y/o explícitamente autorizados.

**Flujo de secretos:** el password efímero del lockfile se lee únicamente para autenticar una petición al loopback `127.0.0.1`; los tokens derivados quedan solo en memoria del proceso y se descartan al cerrar o renovar la sesión. Nunca se escriben en `config.toml`, caché, logs o mensajes de `doctor`. `RIOT_API_KEY`, si se usa en una mejora oficial futura, vive solo en una variable de entorno o `.env` ignorado por Git.

## 2. Decisiones principales

| Tema | Decisión | Motivo |
|---|---|---|
| Lenguaje | **Rust** | Binario nativo, concurrencia segura, bajo consumo y gran desempeño en I/O. |
| Interfaz | **TUI con Ratatui + Crossterm** | Interfaz real de terminal sin el peso de una GUI. |
| Modelo de ejecución | **Orientado a eventos** | Reducir polling y mantener CPU en reposo cuando no hay cambios. |
| Datos | **Raw data separado de derived data** | Trazabilidad, recalculabilidad y menor acoplamiento. |
| Estadísticas | **Analytics Engine dedicado** | Los cálculos no pertenecen a la UI ni a los proveedores. |
| Integraciones | **Provider Layer** | Evita acoplar el dominio a Riot, Tracker u otro origen concreto. |
| Caché | **L1 RAM + L2 disco** | Respuestas rápidas y menos solicitudes de red. |
| Red | **Request Manager centralizado** | Dedupe, prioridades, límites, reintentos y cancelación coherentes. |
| Configuración | **TOML + env var + `.env`** | Separar secretos (`.env`) de opciones (`config.toml`); validar y mostrar en Settings/`doctor` sin exponer claves. |
| Autostart | **Opt-in con `auto-launch`** | Iniciar al encender/abrir VALORANT solo si el usuario lo habilita; desactivado por defecto. |
| Nombre | **Temporal `VTracker`** | Evita hardcodear marca antes del release. |

### Por qué Rust y no Python o C++

**Decidido: Rust.** Python permitiría iterar rápido, pero añade runtime y un perfil de memoria menos predecible para una herramienta que debe permanecer activa. C++ puede lograr un rendimiento similar, pero aumenta la complejidad y el riesgo de errores de memoria. En VTracker el cuello de botella esperado es el acceso al cliente, a disco y a red, no el cómputo puro; Rust ofrece rendimiento nativo y seguridad sin pagar la complejidad habitual de C++.

## 3. Stack propuesto

| Área | Tecnología | Rol |
|---|---|---|
| Lenguaje | Rust (edición vigente) | Núcleo de la aplicación. |
| Runtime async | Tokio | Tareas asíncronas, canales, temporizadores y cancelación. |
| HTTP | Reqwest | Solicitudes HTTP de proveedores. |
| Serialización | Serde + serde_json | Modelos tipados y JSON. |
| Configuración | TOML + crate `toml` + `dotenvy` | Archivo legible + `.env` para secretos; validación en `src/config`. |
| CLI | Clap | Subcomandos, flags y ayuda (`vtracker config`/`autostart`). |
| TUI | Ratatui | Layouts, tablas, gráficos y pantallas (Elm/TEA: Model/Message/Update/View). |
| Terminal/input | Crossterm | Backend de terminal y teclado. |
| Autostart | `auto-launch` | Registro en Run key / Startup folder (Windows), LaunchAgent/XDG en otros OS. |
| Observabilidad | Tracing + tracing-subscriber | Logs estructurados y diagnóstico. |
| Errores | `thiserror` + `anyhow` | Errores de dominio tipados y contexto en el borde de la app. |

**Propuesta futura:** usar SQLite solo si el historial persistente, índices o volumen de caché lo justifican. Para la primera fase puede bastar un caché de archivos versionados en disco.

## 4. Arquitectura general

```text
                               ┌─────────────────────────┐
                               │        VTRACKER         │
                               │    Application Core     │
                               └────────────┬────────────┘
                                            │
             ┌──────────────────────────────┼──────────────────────────────┐
             │                              │                              │
             ▼                              ▼                              ▼
      ┌──────────────┐               ┌──────────────┐               ┌──────────────┐
      │ Game Engine  │               │ Data Engine  │               │  UI Engine   │
      └──────┬───────┘               └──────┬───────┘               └──────┬───────┘
             │                              │                              │
   estado y transición              providers, caché,                   Ratatui
   del juego                         solicitudes, analytics              AppState
             │                              │                              │
             └───────────────┐      ┌──────┴──────┐      ┌───────────────┘
                             ▼      ▼             ▼      ▼
                                   Event Bus → AppState → renderizado
```

### Application Core

Coordina el arranque y apagado, carga de configuración, runtime async, inyección de dependencias, propagación de cancelación y ciclo principal. No debe contener reglas de negocio de estadísticas ni detalles de proveedores.

### Game Engine

Detecta el estado del cliente/juego y traduce cambios en eventos de dominio. Su responsabilidad es saber *qué está ocurriendo*, no consultar o calcular todas las estadísticas.

### Data Engine

Contiene la capa de proveedores, caché, Request Manager, normalización de respuestas y Analytics Engine. Convierte fuentes externas en información de dominio utilizable.

### UI Engine

Mantiene el loop de terminal, entrada de teclado, responsive layout y renderizado de una vista derivada de `AppState`. No realiza HTTP ni cálculos pesados.

## 5. Máquina de estados de VALORANT

**Decidido:** el Game Engine modelará estados explícitos y transiciones. Los nombres exactos deben ajustarse a las señales realmente disponibles durante la investigación técnica.

```mermaid
stateDiagram-v2
    [*] --> Unknown
    Unknown --> Idle: cliente detectado
    Idle --> PreGame: cola / pre-partida detectada
    PreGame --> AgentSelect: selección de agente
    AgentSelect --> InMatch: partida iniciada
    InMatch --> PostMatch: partida finalizada
    PostMatch --> Idle: resultados procesados
    Unknown --> ClientClosed: cliente no disponible
    Idle --> ClientClosed: cliente cerrado
    PreGame --> ClientClosed: cliente cerrado
    AgentSelect --> ClientClosed: cliente cerrado
    InMatch --> ClientClosed: cliente cerrado
    PostMatch --> ClientClosed: cliente cerrado
    ClientClosed --> Unknown: reintento / relanzamiento
```

Estados iniciales previstos:

- `Unknown`: no hay señal fiable aún.
- `Idle`: cliente disponible, sin partida activa.
- `PreGame`: contexto de pre-partida detectado.
- `AgentSelect`: selección de agente / roster disponible cuando la fuente lo permita.
- `InMatch`: partida en curso.
- `PostMatch`: partida terminada; momento de recoger y procesar resultados.
- `ClientClosed`: cliente no accesible.

## 6. Event Bus y eventos

**Decidido:** los cambios relevantes se propagan con eventos, no con pantallas consultando cada componente continuamente. Un watcher puede usar sondeos muy acotados si la fuente no ofrece notificaciones, pero solo para detectar cambios; el resto se activa por eventos.

```text
Game Engine ──► Event Bus ──► Data workflows ──► AppState ──► TUI
                    │                 │
                    ├──► tracing      └──► Cache / Providers / Analytics
                    └──► lifecycle / cancelación
```

Eventos candidatos:

| Familia | Eventos |
|---|---|
| Ciclo del cliente | `ClientDetected`, `ClientUnavailable`, `GameStateChanged` |
| Partida | `PreGameDetected`, `AgentSelectStarted`, `MatchStarted`, `MatchEnded` |
| Datos | `RosterAvailable`, `RawDataLoaded`, `StatsComputed`, `ProviderFailed` |
| Aplicación | `RefreshRequested`, `ConfigReloaded`, `ShutdownRequested` |

**Propuesta:** emplear eventos de dominio tipados; los errores recuperables viajan como resultados y se reflejan en estado/UI, sin derribar todo el proceso.

## 7. Pipeline de datos y separación de datos

```text
 Riot / Tracker / futuro proveedor
               │
               ▼
        Provider Layer
               │
               ▼
        Request Manager
               │
               ▼
       Cache L1 / Cache L2
               │
               ▼
     Raw Data normalizado y versionado
               │
               ▼
        Analytics Engine
               │
               ▼
       Derived Data / PlayerStats
               │
               ▼
            AppState → TUI
```

### Raw data

Datos recibidos de una fuente, conservados con origen, instante de obtención, versión de esquema y caducidad. Ejemplos: respuestas de historial de partidas, rondas, daños, rango, roster o resultados.

### Derived data

Resultados producidos por reglas propias: K/D, win rate, ACS, rachas, desgloses por agente y mapas. Deben guardar metadatos como intervalo de partidas, versión del cálculo y huella/fecha de los datos de origen.

Esta separación permite recalcular métricas cuando cambie una fórmula sin volver a solicitar todo, y ayuda a explicar de dónde proviene cada número.

## 8. Analytics Engine

**Decidido:** Analytics Engine/Stats Engine será el único módulo responsable de transformar datos de partidas en métricas. Proveedores entregan datos normalizados; UI solo presenta resultados.

```text
MatchData[] → parser/normalizador → calculadoras por partida
                                      │
                       ┌──────────────┼──────────────┐
                       ▼              ▼              ▼
                    Combat          Results       Performance
                       │              │              │
                       └──────────────┴──────────────┘
                                      │
                                      ▼
                                Aggregator
                                      │
                                      ▼
                                 PlayerStats
```

Métricas previstas (las definiciones exactas deben documentarse por fuente y modo de juego):

| Grupo | Métricas |
|---|---|
| Combate | K/D, KDA, HS%, ADR, ACS, KAST, kills/deaths/assists, daño total. |
| Resultados | victorias, derrotas, WR, rachas, forma reciente. |
| Contexto | rendimiento por agente, mapa, cola/modo, lado y periodo. |
| Agregados | totales, promedio, mediana, últimos N partidos y comparativas temporales. |

Fórmulas base ilustrativas:

```text
K/D  = kills / deaths                         (definir manejo de deaths = 0)
HS%  = headshot_kills / kills × 100           (si la fuente entrega headshot kills)
WR   = wins / (wins + losses) × 100
ADR  = total_damage / total_rounds
ACS  = suma de combat score / total_rounds    (solo si los campos de fuente lo soportan)
KAST = rondas con Kill, Assist, Survived o Traded / rondas totales × 100
```

**Pendiente:** confirmar qué campos expone cada proveedor y fijar definiciones consistentes para KAST, ACS, empates, abandonos, overtime y modos no competitivos. Nunca se deben inventar campos ausentes ni presentar métricas incomparables como equivalentes.

## 9. Caché de dos niveles

**Decidido:** se usará un diseño L1/L2 con TTL por tipo de dato.

```text
Request → L1 RAM
             ├─ hit ──► respuesta
             └─ miss → L2 Disk
                           ├─ hit ──► promover a L1 → respuesta
                           └─ miss → Request Manager → Provider → guardar L2/L1
```

- **L1 (RAM):** objetos recientemente usados, muy baja latencia y capacidad limitada.
- **L2 (disco):** persistente entre ejecuciones; inicialmente archivos, con migración posible a SQLite.
- **TTL y política:** cada recurso define duración y estrategia de invalidación. Datos de partida activa y perfil histórico no tienen la misma frescura.
- **Metadatos:** clave, origen, obtenido en, expiración, versión de esquema, y opcionalmente ETag/huella.

**Propuesta:** servir datos caducados solo como “último dato conocido” cuando el usuario lo vea claramente y sea preferible a una pantalla vacía.

## 10. Request Manager

**Decidido:** toda solicitud remota pasa por un único Request Manager. Ningún módulo de UI o cálculo hace HTTP directamente.

Responsabilidades:

- Dedupe/coalescing: solicitudes idénticas concurrentes comparten una operación.
- Prioridades: información necesaria para una pantalla o transición antes que prefetch no esencial.
- Rate limiting: límites por proveedor, endpoint y credencial cuando corresponda.
- Timeouts: límite explícito por solicitud.
- Retries: solo para fallos transitorios e idempotentes.
- Backoff: preferiblemente exponencial con jitter.
- Cancelación: descartar tareas que ya no interesan al cambiar de pantalla/estado o al cerrar.
- Métricas: duración, cache hit/miss, reintentos, errores y cuota.

```text
Intento de consulta
       │
       ▼
¿Caché válida? ── sí ──► responder
       │ no
       ▼
¿Ya hay la misma solicitud? ── sí ──► unirse al resultado en vuelo
       │ no
       ▼
cola priorizada → rate limiter → provider → timeout/retry/backoff → caché → resultado
```

## 11. Provider Layer y capabilities

La aplicación consulta capacidades, no implementaciones concretas. Así el dominio no depende de una API específica.

```text
Provider traits/interfaces
       │
       ├── RiotProvider
       ├── TrackerProvider
       └── FutureProvider
```

Capacidades posibles:

| Capacidad | Ejemplo de uso |
|---|---|
| `GameStateSource` | Consultar señal de estado de cliente/juego. **Implementado** (`src/providers/capabilities.rs`) con `GamePhase` fino/grueso. |
| `LiveMatchSource` | Roster en vivo (Pre-Game/Current Game, GLZ vía tokens locales): ranks, nivel, agente de las 10 personas. |
| `PlayerProfileSource` | Perfil, identidad o rango (PD vía tokens locales; API oficial opcional). |
| `MatchHistorySource` | Partidas y resultados (PD `match-history`/`competitive-updates`). |
| `MatchDetailSource` | **Rondas** (`roundResults[]`: kills/deaths/resultado por ronda) para Analytics — post-partida. |

**Decisión 2026-08-24 (agilización):** la fuente primaria es la **Local Client API** de VALORANT — lockfile (`%LocalAppData%\Riot Games\Riot Client\Config\lockfile`) + REST local (`127.0.0.1:{port}`) + WebSocket (`wss://riot:{password}@127.0.0.1:{port}`) + servidores GLZ/PD con los tokens locales. **No requiere API key de producción ni RSO** para: fases reales (`Lobby/PreGame/AgentSelect/InMatch/PostMatch` event-driven), roster en vivo, perfil propio, historial y rondas post-partida. La API oficial (`RIOT_API_KEY`) queda opcional para mejoras. La lectura es solo-lectura (sin inyección ni memoria), misma técnica que herramientas consolidadas (vRY, Vantage); el password del lockfile vive solo en memoria y `doctor` lo enmascara.

**Decidido:** una pantalla pregunta por una capability requerida y recibe un modelo normalizado o una indisponibilidad explicable.  
**Pendiente:** validar oficialmente las APIs, autenticación, RSO/consentimiento, límites y términos de Riot/Tracker antes de implementar cada adaptador.

## 12. AppState y diseño de TUI

`AppState` es el estado en memoria que la UI consume y que los flujos actualizan. Debe contener solo datos de presentación y operación, no clientes HTTP ni lógica de negocio.

Ejemplos de áreas de estado:

- estado del juego y último cambio;
- vista activa, foco y tamaño de terminal;
- roster/match actual y estado de carga;
- perfiles y estadísticas derivadas;
- avisos, errores recuperables y salud de proveedores;
- configuración efectiva y estado de caché.

Pantallas previstas:

| Vista | Propósito |
|---|---|
| **Dashboard** | Estado actual, perfil propio con stats, resumen de sesión y señales. |
| **Match** | Contexto de la partida en curso (mapa/modo/composición) y stats generales del game. |
| **Team / Player** | Roster de la partida encontrada (PreGame/AgentSelect) + stats del equipo; perfil individual y detalle. |
| **History** | Historial propio autorizado, filtros, tendencia y desglose por periodo. |
| **Settings** | **Apartado de configuración:** `config.toml` (intervalo, `profile.riot_id/region`, `autostart.enabled/minimized`, TTL, apariencia), estado de `.env` (`***`), y diagnóstico de providers. |

Navegación propuesta: pestañas o teclas directas; flechas/`j`/`k` para listas, `Enter` para detalle, `r` para actualizar de forma controlada, `q` para salir y ayuda contextual. Los atajos finales deben ser configurables después del MVP.

La TUI debe adaptarse al tamaño disponible:

- terminal pequeña: columnas esenciales y vistas compactas;
- terminal grande: detalles, tendencias y tablas completas;
- nunca depender de una resolución fija.

## 13. Estructura de carpetas Rust propuesta

```text
vtracker/  # nombre temporal — se renombrará antes del release
├── Cargo.toml
├── README.md
├── config.example.toml
├── .env.example
└── src/
    ├── main.rs              # CLI y ciclo principal
    ├── cli/mod.rs           # parsing testeable (actual)
    ├── autostart/mod.rs     # enable/disable/status (auto-launch crate)
    ├── app/
    │   ├── mod.rs
    │   ├── lifecycle.rs
    │   └── state.rs         # AppState con profile/team/match + autostart
    ├── core/
    │   ├── mod.rs
    │   ├── player.rs
    │   ├── match.rs
    │   └── game_state.rs
    ├── game/
    │   ├── mod.rs           # detection local + GameState
    │   ├── detector.rs
    │   └── watcher.rs
    ├── events/
    │   ├── mod.rs
    │   └── bus.rs           # Event Bus → AppState → TUI
    ├── providers/
    │   ├── mod.rs
    │   ├── capabilities.rs  # GameStateSource, PlayerProfileSource, RosterSource...
    │   ├── riot.rs
    │   └── tracker.rs
    ├── requests/
    │   ├── mod.rs
    │   └── manager.rs
    ├── cache/
    │   ├── mod.rs
    │   ├── memory.rs        # L1
    │   └── disk.rs          # L2
    ├── analytics/
    │   ├── mod.rs
    │   ├── combat.rs
    │   ├── performance.rs
    │   ├── aggregates.rs
    │   └── calculator.rs
    ├── ui/
    │   ├── mod.rs
    │   ├── terminal.rs
    │   ├── views/
    │   │   ├── dashboard.rs  # perfil propio + estado
    │   │   ├── match.rs      # stats generales del game
    │   │   ├── player.rs
    │   │   ├── history.rs
    │   │   └── settings.rs   # apartado configuración (autostart, perfil, TTL)
    │   └── components/
    ├── config/mod.rs        # TOML + env var, validación, tests
    └── diagnostics/mod.rs   # doctor (sin exponer secretos)
```

Es una organización objetivo, no una exigencia para crear todos los módulos vacíos desde el día uno. El MVP debe introducir solo los límites que necesite.

## 14. Ciclo de vida de la aplicación

```text
Start
  → cargar .env (dotenvy) + config.toml (TOML + validación)
  → iniciar tracing
  → inicializar caché L1/L2
  → crear Request Manager y proveedores habilitados (solo si hay API key en env)
  → iniciar Game Engine y Event Bus
  → si autostart.enabled, registrar en sistema (auto-launch) solo si el usuario lo pidió
  → entrar al loop de TUI/watch (Dashboard con perfil → Team al encontrar partida → Match en game)
  → reaccionar a eventos y actualizar AppState
  → cancelar tareas y restaurar terminal al salir
```

Requisitos de apagado:

- Restaurar correctamente el modo de terminal, incluso ante error.
- Cancelar solicitudes/tareas que sigan en vuelo.
- Persistir caché y configuración de forma segura cuando corresponda.
- Registrar la causa de salida y errores relevantes mediante `tracing`.

## 15. Interfaz de línea de comandos prevista

| Comando | Propósito inicial |
|---|---|
| `vtracker watch` | Ejecutar la TUI y observar el estado del juego (perfil → equipo → partida). |
| `vtracker player <riot-id>` | Consultar un jugador mediante providers disponibles (requiere RSO/opt-in). |
| `vtracker match [id]` | Mostrar o abrir detalle de una partida. |
| `vtracker history [player]` | Mostrar historial y agregados. |
| `vtracker cache <subcomando>` | Inspeccionar, limpiar selectivamente o diagnosticar caché. |
| `vtracker config <subcomando>` | **Apartado configuración:** `show`/`edit`/`validate` de `%APPDATA%\vtracker\config.toml` (intervalo, profile, autostart, TTL). |
| `vtracker autostart <enable\|disable\|status>` | Gestionar inicio automático al encender/abrir VALORANT (auto-launch, opt-in). |
| `vtracker doctor` | Comprobar entorno, cliente, red, proveedores, configuración, autostart y caché — sin exponer secretos. |

**Propuesta futura:** `vtracker benchmark` para medir arranque, memoria, CPU inactiva, caché y parsing. No debe convertirse en optimización ficticia: primero métricas reproducibles.

## 16. Estrategia de desarrollo por fases

| Fase | Resultado | Incluye |
|---|---|---|
| 0 — Validación | Viabilidad y cumplimiento | Confirmar fuentes permitidas, auth (RSO), límites, señales de estado y datos disponibles. |
| 1 — MVP de detección | `watch` útil y estable | CLI testeable, configuración, tracing, detección local, máquina de estados y TUI mínima. **✓ Completado** |
| 2 — Datos base | Perfil/roster/historial normalizados | `GameStateSource` desacoplado, primer provider real (con `.env` protegido), Request Manager básico, L1/L2 y AppState con flujo perfil→equipo→partida. |
| 3 — Analytics | Métricas reproducibles | Modelos de partida, K/D, WR, HS%, ADR y agregados iniciales (fixtures + tests). |
| 4 — TUI completa | Navegación, pantallas y **configuración** | Dashboard (perfil), Match (stats generales), Team/Player (roster), History, **Settings (apartado configuración + autostart)**, responsive layout (Elm/TEA). |
| 5 — Robustez | Operación confiable | Doctor extendido (sin exponer secretos), retries, rate limits, cancelación, autostart (`auto-launch`), tests, benchmarks y observabilidad (`tracing`). |
| 6 — Distribución y nombre final | Release | Binarios, instalador con autostart opt-in, auditoría de secretos, y renombrar `VTracker` al nombre final. |

El MVP no debe prometer todas las estadísticas: primero demuestra que detecta el estado correctamente y que puede actualizar la interfaz sin consumo innecesario.

## 17. Principios de optimización

- **Event-driven primero:** no recomputar ni solicitar datos si no cambió el estado relevante.
- **Polling mínimo y justificado:** si una integración lo exige, usar intervalos adaptativos, backoff y detección de cambios.
- **Caché antes de red:** buscar L1/L2 antes de programar una solicitud.
- **Dedupe y cancelación:** no mantener trabajo duplicado o ya irrelevante.
- **Cálculos incrementales:** recalcular estadísticas solo si cambia el conjunto/huella de partidas.
- **UI liviana:** renderizar desde `AppState`, no hacer I/O desde componentes visuales.
- **Medir antes de optimizar:** RAM, CPU idle, latencia de caché, parsing, arranque, solicitudes y errores.
- **Benchmarks y profiling:** establecer escenarios reproducibles antes de hacer cambios de rendimiento.
- **Doctor como herramienta de soporte:** diagnosticar configuración, conectividad, acceso a fuentes, caché y salud del entorno.

## 18. Resumen de lo definido

Ya acordado:

- **Nombre temporal `VTracker`** hasta el release; evitar hardcodear marca.
- VTracker se construirá en Rust, no Python ni C++.
- La interfaz será una TUI basada en Ratatui/Crossterm con patrón Elm/TEA (Model/Message/Update/View) y separación dominio/UI.
- La arquitectura se divide en Application Core, Game Engine, Data Engine y UI Engine.
- El Game Engine modela estados explícitos de VALORANT y emite eventos.
- El flujo de datos es proveedor → caché → raw data → analytics → derived data → AppState → TUI.
- Analytics Engine calcula métricas y conserva la separación respecto a proveedores y UI.
- Existirá caché L1 RAM/L2 disco y un Request Manager central.
- La capa de providers se diseñará por capacidades para admitir Riot, Tracker y futuros orígenes.
- **Configuración:** `config.toml` para opciones + `.env` para secretos, con `.gitignore` estricto y `doctor` que enmascara claves.
- **Apartado configuración:** vista Settings y comandos `config`/`autostart` para gestionar perfil, intervalo, TTL y autostart.
- **Autostart opt-in** con `auto-launch` (Windows Run key / Startup folder), desactivado por defecto, solo por acción explícita del usuario.
- **Experiencia por fase:** perfil propio (Idle) → stats de equipo (PreGame/AgentSelect) → stats generales del game (InMatch), siempre vía `GameStateSource` autorizado.
- El desarrollo se enfocará primero en detección fiable y un `watch` mínimo antes de ampliar estadísticas y pantallas.

## 19. Pendientes para la siguiente fase (actualizado)

1. Verificar las fuentes de datos que sean técnicamente disponibles y permitidas, con sus requisitos de autenticación (RSO), consentimiento y límites (Prioridad 2B).
2. Precisar qué señal o interfaz alimentará cada transición de la máquina de estados (`GameStateSource` autorizado, no procesos).
3. Definir el modelo canónico de jugador, partida, ronda y datos crudos.
4. Elegir el primer provider concreto y el contrato de sus capabilities (`PlayerProfileSource`, `RosterSource`, `MatchHistorySource`).
5. Fijar definiciones de métricas —en especial ACS/KAST/HS%— según los campos verificables de la fuente.
6. Diseñar el archivo de configuración (`config.toml` + `profile.riot_id/region`, `autostart.enabled/minimized`), política de TTL e información sensible/secretos en `.env` (`.env.example` como plantilla).
7. Acordar la TUI del MVP y la navegación completa: Dashboard (perfil), Team (equipo), Match (game), History, **Settings (configuración + autostart)**, comportamiento sin datos y mensajes de error (Elm/TEA).
8. Diseñar el flujo de autostart (`auto-launch` crate, Windows Run key/Startup folder, comandos `autostart enable|disable|status`, `doctor` con estado) y el instalador con consentimiento explícito.
9. Establecer objetivos de rendimiento medibles para arranque, RAM, CPU idle y latencia (benchmarks reproducibles antes de optimizar).
10. Planificar renombrado de `VTracker` al nombre final y propagación a `Cargo.toml`/binario/docs antes del release.

## 20. Fuentes de datos — investigación 2026-08-24 (Local Client API)

Verificado contra `valapidocs.techchrism.me` (documentación de la API interna del cliente), repos de la comunidad (vRY 563★, Vantage, RumbleMike/ValorantClientAPI) y foros:

| Dato | Fuente | Disponibilidad |
|---|---|---|
| Fase real (Lobby/PreGame/AgentSelect/InMatch/PostMatch) | REST local + WebSocket local (eventos) | **En vivo**, event-driven |
| Roster de las 10 personas (ranks, nivel, agente, perfiles privados incluidos) | `glz-{region}-1.{shard}.a.pvp.net` Pre-Game/Current Game con tokens locales | **En vivo** |
| Perfil propio, MMR, historial | `pd.{shard}.a.pvp.net` con tokens locales | En vivo/post |
| **Desglose por ronda** (`roundResults[]`: `roundNum`, `winningTeam`, `roundResult`, `playerStats[].kills`) | `pd.../match-details/v1/matches/{id}` | **Post-partida** |
| Kills en vivo dentro de la ronda actual | No expuesto por ninguna API | ❌ Solo OCR del killfeed (frágil) o lectura de memoria (descartada por principios) |

**Decisión de producto:** el tracker de rondas se actualiza **al terminar la partida** (tabla `Ronda | Resultado | Kills | ¿Moriste?`); si `match-details` llegara a responder a mitad de partida se muestra progreso incremental, con degradación elegante si responde 404. No se implementa OCR ni lectura de memoria.

**Autenticación local:** lockfile `name:pid:port:password:protocol` → Basic Auth `riot:{password}` (certificado self-signed) → `/entitlements/v1/token` → bearer + entitlement JWT para GLZ/PD. El password nunca se persiste ni loguea.

## 21. Buenas prácticas y referencias aplicadas (2026)

- **TUI Rust:** Elm/TEA (Model/Message/Update/View) con `ratatui` + `tokio::select!`; separar dominio de presentación; `cargo fmt`/`clippy`/`test` en cada cambio (ver `README.md:Principios técnicos`).
- **Event-driven:** polling mínimo, caché antes de red, dedupe y cancelación de requests, cálculos incrementales.
- **Autostart:** usar `auto-launch`/`tauri-plugin-autostart` como referencia; nunca auto-registrar sin `Config::autostart.enabled` explícito; respetar Run key vs Startup folder (MITRE T1547.001).
- **Seguridad:** `.env` en `.gitignore`, `dotenvy` solo en runtime, `doctor` enmascara (`***`), `cargo audit`, validar RSO/opt-in antes de mostrar datos de otro jugador (política Riot: datos personales solo con consentimiento).
- **Config:** TOML versionado, validación estricta (`src/config/mod.rs:19`), `config.example.toml` sin secretos, `config.toml` real fuera del repo (`%APPDATA%/vtracker/`).
- **Calidad/Ambiental/Seguridad de la información:** ver `docs/ISO.md` — sistema integrado ISO 9001/14001/27001 proporcional a VSE.

## 22. Sistema Integrado de Gestión — ISO 9001 / 14001 / 27001 (compromiso)

> **Decidido:** VTracker (nombre temporal) adopta principios ISO 9001 (Calidad), ISO 14001 (Ambiental) e ISO 27001 (Seguridad) como **sistema integrado PHVA**, proporcional a un proyecto pequeño. Detalle completo en `docs/ISO.md`. La certificación formal es objetivo a medio plazo, no requisito para el MVP.

| Norma | Enfoque en VTracker | Práctica clave |
|---|---|---|
| **ISO 9001:2015 SGC** | Calidad repetible (ISO 90003 + ISO/IEC 29110 VSEs) | `TASKS.md` como registro de riesgos, 43 tests + `cargo fmt/clippy` como inspección, Raw/Derived separados para trazabilidad, `watch.log` como registro. |
| **ISO 14001:2015 SGA** | Green coding — minimizar energía (CPU/mem/red) | Rust event-driven, caché L1/L2, `minimized=true`, medir arranque/CPU idle/binario antes de optimizar (`docs/BENCHMARKS.md` futuro). |
| **ISO 27001:2022 SGSI** | CIA de secretos y datos de jugadores (93 controles, 8 clave: 8.25-8.29, 8.32, 8.8, 5.9) | `.env` en `.gitignore:7`, `dotenvy` runtime, `doctor` enmascara `***`, `cargo audit`+SBOM futuro, RSO/opt-in obligatorio. SoA y `docs/RISK.md` ligero. |

**Integración PHVA:** Planificar (`TASKS.md`/`Arquitectura`) → Hacer (`src/` + tests) → Verificar (`doctor` + `cargo audit` + benchmarks) → Actuar (nuevos tests/fixes). Documentación viva versionada.

---

Este documento es la línea base para iniciar la fase de validación y el MVP. Cualquier decisión que dependa de APIs externas o políticas vigentes debe tratarse como pendiente hasta verificarse con documentación oficial actualizada. Ver `docs/ISO.md` para el SGC/SGA/SGSI completo.
