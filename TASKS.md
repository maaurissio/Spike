# VTracker — Lista de tareas — nombre temporal

> **Nombre temporal:** `VTracker` es provisional hasta el final del desarrollo. El nombre final se definirá antes del release y se propagará a `Cargo.toml`, `README.md`, `Arquitectura-inicial.md` y docs. Evitar hardcodear el nombre en mensajes de usuario más allá de `VERSION`.

Este documento ordena el trabajo restante. Las integraciones con Riot u otros proveedores solo se implementarán después de confirmar permisos, términos de uso y límites aplicables.

## Hecho: MVP local

- [x] Crear proyecto Rust y documentación de arquitectura.
- [x] Implementar `vtracker watch`.
- [x] Detectar procesos locales de Riot/VALORANT sin acceso a memoria.
- [x] Mostrar estados honestos: cliente cerrado, cliente disponible y juego abierto con modo no confirmado.
- [x] Registrar transiciones recientes en el panel.
- [x] Añadir configuración de intervalo y `--once`.
- [x] Añadir `vtracker doctor` para diagnosticar procesos y configuración.
- [x] Añadir pruebas del motor de transiciones.
- [x] Separar el MVP en módulos de configuración, detección, observación, diagnóstico y UI.

## Prioridad 1 — Base fiable

- [x] Probar `watch` con el cliente cerrado, abierto y el juego abierto en un equipo real.
- [x] Registrar una tabla de los procesos observados en cada situación para evitar falsos positivos.
- [x] Añadir un log opcional a archivo con fecha, estado y transición.
- [x] Mejorar el parser de configuración y avisar claramente de valores inválidos.
- [x] Añadir pruebas para configuración, argumentos CLI y salida de `doctor`.
- [x] Revisar la política y APIs oficiales de Riot para el primer proveedor.
- [ ] Definir una fuente autorizada que distinga lobby, selección, partida y postpartida; la API oficial publicada no expone ese estado en tiempo real.

> Evidencia P1 (2026-08-24): `vtracker watch --once` probado con `VTRACKER_STATE=closed|idle|game` y `doctor` real detectó `RiotClientServices.exe`/`RiotClientCrashHandler.exe` en `Idle` (ver `README.md:Procesos observados`). Lógica cubierta por 43 tests (`cargo test`) en `config`, `cli`, `game` y `diagnostics`; `src/game/mod.rs:95` expone `observation_from_process_list` testeable.

## Prioridad 2 — Fuente de estado y datos — Local Client API primero (agilizado)

> **Investigación 2026-08-24 (ver `docs/ISO.md` refs y `Arquitectura-inicial.md:22`):** la **Local Client API** de VALORANT (lockfile en `%LocalAppData%\Riot Games\Riot Client\Config\lockfile` + `wss://riot:{password}@127.0.0.1:{port}`) da tokens (entitlement/bearer) para GLZ/PD **sin API key de producción y sin RSO**. Esto desbloquea la experiencia completa (fases reales, roster, perfil, historial, rondas) sin esperar aprobación de Riot. La API oficial pasa a ser opcional/mejora posterior.

### 2A — Seguridad y secretos (ajustado a lockfile)

- [x] Proteger secretos en repo: `.env` en `.gitignore` (`/.gitignore:4`), `.env.example` como plantilla y `config.example.toml` documentado sin secretos.
- [x] **Lockfile:** leer `%LocalAppData%\Riot Games\Riot Client\Config\lockfile` (`name:pid:port:password:protocol`); password **solo en memoria**, nunca logueado ni persistido (`doctor` muestra solo `auth=presente/ausente`).
- [ ] `RIOT_API_KEY` en `.env` queda **opcional** (solo para mejoras con API oficial futura).
- [x] Documentar flujo de secretos en `README.md` y `Arquitectura-inicial.md` (límites de producto y cumplimiento; solo-lectura, sin inyección ni memoria).

### 2B — Diseño de proveedores (hecho para interfaces; fuentes locales siguientes)

- [x] Crear una interfaz `GameStateSource` (`src/providers/capabilities.rs`) para que el resto de la app no dependa de un proveedor concreto.
- [x] Definir estructura `src/providers/` con `mod.rs`, `capabilities.rs`, `process.rs`, `mock.rs` según `Arquitectura-inicial.md:11`.
- [x] Modelar estados `Lobby`, `PreGame`, `AgentSelect`, `InMatch` y `PostMatch` + `GamePhase` y `Confidence`; `GameOpen` preserva honestidad de detección por procesos.
- [x] Implementar `ProcessGameStateSource` (wrapper honesto de `game::detect`) y `MockGameStateSource` para validar TUI sin red.
- [x] Resolver con `resolve_with_fallback` y mostrar último estado conocido cuando una fuente falle (retryable → fallback, auth → no fallback).
- [x] Investigar fuentes: Local Client API (lockfile + WebSocket + GLZ/PD) cubre fases reales, roster, perfil, historial y rondas; documentado en `Arquitectura-inicial.md:22`.

### 2C — Implementación Local Client (nueva ruta, sin API key)

- [x] **`src/providers/lockfile.rs`:** parsear lockfile (puerto/password), con tests sobre contenido simulado.
- [x] **`src/providers/local.rs` (`LocalClientSource`, base):** implementa `GameStateSource`, lee el lockfile, valida `/help`, entitlements, sesión externa y región/locale con Basic Auth en memoria; usa HTTPS local con timeout y degrada al detector de procesos sin exponer secretos.
- [x] **WebSocket local (contrato):** handshake TLS, subprotocolo WAMP y suscripción `OnJsonApiEvent` verificados contra un cliente VALORANT real el 2026-08-26, sin guardar payloads ni credenciales.
- [ ] **`LocalClientSource` (fases reales):** consumir el stream persistente y mapear eventos observados a transiciones `Lobby→PreGame→AgentSelect→InMatch→PostMatch` event-driven (sin polling).
- [ ] **Capability `LiveMatchSource`:** `Pre-Game Match` y `Current Game Match` (GLZ) → roster con ranks/nivel/agente de las 10 personas (incluye perfiles privados) para stats de equipo en Agent Select/partida.
- [ ] **Capability `MatchDetailSource`:** `pd.{shard}.a.pvp.net/match-details/v1/matches/{id}` post-partida → `roundResults[]` con kills/deaths/resultado **por ronda** (desglose completo en `PostMatch`/`History`). Opcional: intentar polling durante partida y degradar con elegancia si aún no está disponible.
- [ ] **Capability `PlayerProfileSource`:** perfil propio + historial (`match-history`, `competitive-updates`, `mmr`) con tokens locales.
- [x] Extender `vtracker doctor` para validar lockfile + salud de la Local Client API/proveedor, sin exponer secretos.
- [ ] API oficial Riot (`RIOT_API_KEY`) queda como mejora opcional posterior (leaderboards, contenido); no bloquea nada.

## Prioridad 3 — Datos propios e historial + Experiencia por fase

- [x] Definir modelos normalizados de jugador, partida, **ronda** (`src/models/mod.rs`: `Round`, `PlayerRoundStat`, `MatchRounds`, outcome y totales oficiales) con validación de secuencia/modo.
- [ ] Implementar una capa de providers por capacidades: perfil, historial, detalle de partida y rondas.
- [x] Implementar caché L1 en memoria con TTL (`src/cache/mod.rs`, `moka` TinyLFU/LRU; capacidad acotada, `get_with` anti-stampede, sin persistir secretos).
- [ ] Implementar caché L2 en disco con versión de esquema y expiración (solo si aporta valor; v1 puede ser solo RAM).
- [ ] Centralizar solicitudes con timeout, deduplicación y reintentos seguros (Requests Manager `dedupe`/backoff/bounded channels).
- [ ] Añadir el comando `vtracker history` para consultar el historial propio (tokens locales).
- [ ] **Flujo perfil:** al detectar `Idle`, mostrar perfil propio con stats cacheadas; si provider falla, último dato conocido y error recuperable.
- [ ] **Flujo equipo:** al detectar `PreGame`/`AgentSelect` vía `LocalClientSource`, consultar `LiveMatchSource` y mostrar ranks/WR del equipo en vivo.
- [ ] **Flujo partida:** al detectar `InMatch`, mostrar contexto (mapa, modo, composición) en vivo.
- [ ] **Flujo rondas (requisito explícito):** al terminar cada ronda (o al terminar la partida si la fuente no entrega antes), mostrar kills/muertes propias por ronda — tabla `Ronda | Resultado | Kills | ¿Moriste?` desde `MatchDetailSource` + `analytics` (fixture de 5 rondas para tests).

## Prioridad 4 — Estadísticas

- [x] Crear el módulo `analytics` separado de providers y UI.
- [x] Calcular K/D, KDA y win rate a partir de datos normalizados.
- [x] Calcular HS%, ADR y ACS solo cuando la fuente entregue los campos necesarios.
- [ ] Documentar tratamiento de empates, abandonos, overtime y modos no competitivos.
- [ ] Añadir desgloses por agente, mapa, modo y últimas N partidas.
- [x] Cubrir las fórmulas con fixtures y pruebas reproducibles.

## Prioridad 5 — TUI completa + Apartado Configuración

> **Configuración es requisito explícito del usuario** — debe ser completa antes del release.

- [ ] Añadir Ratatui y Crossterm (Elm/TEA; patrones dirty-flag, widgets pre-construidos y canales acotados ver `docs/PERFORMANCE.md:3.1`).
- [ ] Crear Dashboard con estado, salud de fuentes y resumen de sesión (perfil propio destacado).
- [ ] Crear vistas Match, Player, History y **Settings (Configuración)**.
- [ ] **Settings debe permitir:** ver/editar `config.toml` (intervalo, `log_transitions`, `profile.riot_id`, `profile.region`, `autostart.enabled/minimized`), gestionar `.env` (solo estado `***`/`no configurada`), TTL de caché y apariencia.
- [x] Añadir `vtracker config show|edit|validate` y persistencia atómica de `config.toml`, sin mostrar secretos.
- [ ] Añadir navegación por teclado, ayuda y estados de carga/error.
- [ ] Adaptar layouts a terminales pequeñas y grandes.
- [ ] Mantener I/O y cálculos fuera del renderizado; UI solo consume `AppState`.

## Prioridad 6 — Robustez, autoinicio y distribución + ISO

- [ ] Añadir logs estructurados y niveles configurables (`tracing`).
- [ ] Medir arranque, CPU en reposo, memoria y eficacia de caché (evidencia ISO 9001/14001).
- [ ] Añadir pruebas de integración para CLI, caché y providers simulados.
- [ ] **Autoinicio (autostart):** crear `src/autostart/mod.rs` con crate `auto-launch` (`AutoLaunchBuilder`). En Windows usa Run key / Startup folder (`HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`). Comandos `vtracker autostart enable|disable|status`, `doctor` muestra estado. Desactivado por defecto, requiere consentimiento explícito; nunca auto-registrarse sin acción del usuario. Opcional: iniciar al detectar VALORANT si `autostart.enabled = true`.
- [ ] Añadir `vtracker cache` (inspeccionar/limpiar L1/L2) y pulir `vtracker config`.
- [ ] Preparar binarios de release e instrucciones de instalación (`cargo build --release`, instalador que opcionalmente registra autostart).
- [ ] Revisar seguridad de credenciales y privacidad antes de distribuir (auditoría de que ninguna key se loguea, `cargo audit`, RSO/opt-in validado).
- [ ] Renombrar `VTracker` al nombre final y propagar a todos los docs y binario.
- [ ] **ISO 9001 (Calidad):** mantener control documental (`TASKS.md`, `docs/ISO.md`), trazabilidad Raw/Derived, `cargo test`/`clippy`/`fmt` como inspección y `watch.log` como registro. Ver `docs/ISO.md:1`.
- [ ] **ISO 14001 (Ambiental):** medir y documentar impacto (CPU idle, memoria, binario, red evitada por caché) en `docs/BENCHMARKS.md`; política de green coding (event-driven, L1/L2). Ver `docs/ISO.md:2`.
- [ ] **ISO 27001 (Seguridad):** mantener SGSI proporcional (8 controles clave 8.25-8.29/8.32/8.8/5.9), SoA ligero, `RISK.md`, `cargo audit`+SBOM, `doctor` sin exponer secretos. Ver `docs/ISO.md:3`. Certificación formal opcional a medio plazo.

## Siguiente tarea recomendada

1. **Hecho (2B — diseño):** `GameStateSource` + `GamePhase`/`ProviderError`/`StateInfo` + `Process`/`Mock` + `resolve_with_fallback` (58 tests).
2. **Ahora (2C — Local Client):** `src/providers/lockfile.rs` → `LocalClientSource` con WebSocket para fases reales → `LiveMatchSource` (roster en vivo) → `MatchDetailSource` (rondas post-partida). **Sin API key ni RSO.**
3. **Después:** analytics por ronda (kills/muertes por ronda, fixture de 5 rondas) y vista en `PostMatch`/`History`.
4. **Opcional:** API oficial Riot con `RIOT_API_KEY` solo para mejoras (leaderboards/contenido).
