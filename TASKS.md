# VTracker — Lista de tareas — nombre temporal

> **Nombre temporal:** `VTracker` es provisional hasta el final del desarrollo. El nombre final se definirá antes del release y se propagará a `Cargo.toml`, `README.md`, `Arquitectura-inicial.md` y docs. Evitar hardcodear el nombre en mensajes de usuario más allá de `VERSION`.

Este documento ordena el trabajo restante. Las integraciones con Riot u otros proveedores solo se implementarán después de confirmar permisos, términos de uso y límites aplicables.

> **Requisito principal confirmado (2026-08-28, ADR-011):** mostrar aliados y enemigos de la partida (diez jugadores en 5v5) con rangos y estadísticas disponibles y permitidos. La implementación de datos propios es una base parcial, no el alcance final. No considerar cumplido el objetivo principal hasta implementar y validar el roster.

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
- [x] Definir `LocalClientSource` de solo lectura para fases finas vía WebSocket local; faltan observaciones de transición reales para validar todo el mapeo.

> Evidencia P1 (2026-08-24): `vtracker watch --once` probado con `VTRACKER_STATE=closed|idle|game` y `doctor` real detectó `RiotClientServices.exe`/`RiotClientCrashHandler.exe` en `Idle` (ver `README.md:Procesos observados`). Lógica cubierta por 43 tests (`cargo test`) en `config`, `cli`, `game` y `diagnostics`; `src/game/mod.rs:95` expone `observation_from_process_list` testeable.

## Prioridad 2 — Fuente de estado y datos — Local Client API primero (agilizado)

> **Alcance confirmado 2026-08-28 (ADR-014):** continuar el roster local como función principal mediante lectura de GLZ/PD. ADR-013 conserva la advertencia oficial para distribución; no bloquea el prototipo. Nunca desanonimizar `Incognito` ni tocar memoria/input del juego.

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
- [x] **`LocalClientSource` (stream):** consumir `OnJsonApiEvent` en un listener persistente de solo lectura; los payloads se descartan y solo URIs inequívocas actualizan estado.
- [ ] **`LocalClientSource` (fases reales):** observar y validar las URIs de transición para completar el mapeo `Lobby→PreGame→AgentSelect→InMatch→PostMatch` event-driven (sin polling). El listener ya expira la fase tras 15 s sin evento para evitar estados obsoletos.
- [x] **Capability `LiveMatchSource` (base parcial):** tras el evento local `InMatch`, realiza un único `GET` a `Current Game Match` (GLZ) y muestra modo, mapa y agente propio. El roster y las estadísticas de otros jugadores siguen pendientes; esta base no cumple aún el requisito principal.
- [x] **Validación del roster (ADR-011/013):** matriz oficial en `docs/ROSTER-POLICY.md`; registro/auditoría y RSO/opt-in requeridos, identidades ocultas protegidas y *scouting* previo de rivales fuera de alcance.
- [x] **Modelo seguro de roster:** normaliza aliado/enemigo/participante, slots y estados disponibles/ocultos/ausentes, sin PUUID ni MatchID en la TUI.
- [x] **Proveedor real de roster base (ADR-014):** `Current Game Match` entrega equipos, agentes y rango disponible; Name Service resuelve una vez los nombres visibles y excluye `Incognito`. Si falla el enriquecimiento, conserva el roster anónimo.
- [x] **Estadísticas del roster:** consulta hasta cinco partidas de la cola relevante para cada participante —incluidos los `Incognito`—, deduplica detalles compartidos y limita la concurrencia a seis solicitudes. La TUI muestra K/D, HS% por impactos, KAST derivado, WR y forma reciente sin recibir PUUID ni MatchID; una falla degrada por jugador.
- [x] **Capability `MatchDetailSource`:** al entrar a `PostMatch`, usa el ID de una URI local reciente y ejecuta un único `GET` a `pd.{shard}.a.pvp.net/match-details/v1/matches/{id}` con tokens efímeros; normaliza rondas de los modos compatibles y, en Deathmatch/Team Deathmatch/Escalation, extrae únicamente el resumen propio K/D/A y puntos. No imprime secretos/IDs ni hace polling durante partida.
- [x] **Capability `PlayerProfileSource`:** `account-xp` y `mmr` para el jugador autenticado muestran nivel, XP, rango, RR y récord competitivo de la temporada activa. La fuente valida que `Subject` coincida con el PUUID local y no retiene IDs de partidas.
- [x] **Cambios competitivos propios:** `competitive-updates` muestra hasta las cinco variaciones recientes de RR y bono de rendimiento desde `vtracker player`, descartando MatchID antes de la UI.
- [x] Extender `vtracker doctor` para validar lockfile + salud de la Local Client API/proveedor, sin exponer secretos.
- [ ] API oficial Riot (`RIOT_API_KEY`) queda como mejora opcional posterior (leaderboards, contenido); no bloquea nada.

## Prioridad 3 — Roster, perfil e historial + Experiencia por fase

- [x] Definir modelos normalizados de jugador, partida, **ronda** (`src/models/mod.rs`: `Round`, `PlayerRoundStat`, `MatchRounds`, outcome y totales oficiales) con validación de secuencia/modo.
- [x] Implementar la capa base de providers por capacidades: perfil básico, historial propio, detalle de partida y rondas.
- [x] Implementar caché L1 en memoria con TTL (`src/cache/mod.rs`, `moka` TinyLFU/LRU; capacidad acotada, `get_with` anti-stampede, sin persistir secretos).
- [ ] Implementar caché L2 en disco con versión de esquema y expiración (solo si aporta valor; v1 puede ser solo RAM).
- [ ] Centralizar solicitudes con timeout, deduplicación y reintentos seguros (Requests Manager `dedupe`/backoff/bounded channels).
- [x] Añadir `vtracker history [--limit 1..20]`: una única consulta de historial propio que muestra modo y antigüedad, sin IDs de partidas.
- [x] **Flujo perfil básico:** al detectar `Idle`, muestra nivel y XP propios; la caché L1 conserva el último perfil durante 60 s si una consulta falla.
- [ ] **Flujo perfil ampliado:** añadir estadísticas agregadas y snapshot competitivo al perfil cacheado de `watch`.
- [ ] **Flujo equipo y enemigos (principal):** mostrar los diez jugadores en partidas 5v5, sus agentes, rangos y estadísticas históricas disponibles y permitidos; en selección de agente, mostrar solo lo que la fuente validada permita conocer en esa fase. Respetar identidades ocultas y no inventar datos faltantes.
- [x] **Flujo partida (base parcial):** al detectar `InMatch`, mostrar contexto propio de modo, mapa y agente en vivo. Falta ampliar la vista al roster de aliados y enemigos.
- [ ] **Verificación del requisito principal:** pruebas de equipos, campos ausentes y privacidad, más validación en partida real. Diferenciar estadísticas históricas de estadísticas de la partida en curso.
- [x] **Flujo rondas postpartida:** al terminar la partida, mostrar kills/muertes propias por ronda — tabla `Ronda | Resultado | Kills | Muertes` desde `MatchDetailSource`, sin mostrar IDs ni datos de otros jugadores. El timeline en vivo por frontera de ronda queda pendiente.

## Prioridad 4 — Estadísticas

- [x] Crear el módulo `analytics` separado de providers y UI.
- [x] Calcular K/D, KDA y win rate a partir de datos normalizados.
- [x] Calcular HS%, ADR y ACS solo cuando la fuente entregue los campos necesarios.
- [ ] Documentar tratamiento de empates, abandonos, overtime y modos no competitivos.
- [x] Añadir resumen de últimas N partidas propias (`vtracker stats --limit 1..5`): K/D, KDA y win rate desde historial + detalles finales, con IDs efímeros y sin roster.
- [x] Añadir desglose propio por modo dentro de `vtracker stats`.
- [x] Añadir desgloses propios por agente y mapa.
- [x] Cubrir las fórmulas con fixtures y pruebas reproducibles.

## Prioridad 5 — TUI completa + Apartado Configuración

> **Configuración es requisito explícito del usuario** — debe ser completa antes del release.

- [x] Añadir Ratatui y Crossterm con un dashboard inicial, navegación por teclado y dirty-flag (sin I/O durante render).
- [x] Portar la composición de `docs/mockups` a Rust: cinco vistas, Aliados → Tus rondas → Enemigos, foco, selección/detalle y modo `dashboard --demo` aislado de red/configuración personal.
- [x] Temas Sistema/Noche/Claro/Sin color, previsualización con `t` y persistencia explícita con `s`; compatibilidad con archivos de configuración anteriores.
- [ ] Crear Dashboard con estado, salud de fuentes y resumen de sesión (perfil propio destacado).
- [ ] Crear vistas Match, Player, History y **Settings (Configuración)**.
- [ ] Conectar datos reales confirmados al timeline compacto entre aliados y enemigos; su presentación de tres filas (`1K/4K`, `R1/R2`, `0D/2D`), ronda pendiente y paginación ya existe en la demo Rust.
- [ ] Acceso externo a Tracker.gg del jugador visible seleccionado: construir URL HTTPS solo desde un Riot ID ya resuelto; mantener deshabilitado para `Jugador oculto`. La acción de la maqueta permanece ficticia por ahora.
- [ ] **Settings debe permitir:** ver/editar `config.toml` (intervalo, `log_transitions`, `profile.riot_id`, `profile.region`, `autostart.enabled/minimized`), gestionar `.env` (solo estado `***`/`no configurada`), TTL de caché y apariencia.
- [x] **Settings básico:** editar intervalo (1–60 s) y registro con borrador, guardado explícito, descarte y aplicación en la sesión sin reiniciar.
- [x] Añadir `vtracker config show|edit|validate` y persistencia atómica de `config.toml`, sin mostrar secretos.
- [x] Añadir navegación por teclado, ayuda de atajos, desplazamiento de tablas y estados de carga/error con último dato disponible.
- [x] Corregir retroceso de temas con `-` y conservar la partida seleccionada al actualizar el historial; cerrar el detalle si deja de estar disponible. Cubierto con regresiones.
- [x] Adaptar layouts a terminales pequeñas y grandes (mínimo 38×10; demo completa 72×24/38×26, selección visible, scroll y paginación del timeline).
- [x] Conectar el roster real base a la TUI (ADR-014): equipos/participantes, nombre visible, agente y rango disponible; marcador, estadísticas históricas y rondas en vivo siguen pendientes.
- [x] Mantener las consultas y escrituras fuera del hilo de interfaz: trabajador con colas acotadas, solicitudes duplicadas suprimidas y descarte de respuestas de fases anteriores. El estado de pantalla no retiene el roster postpartida.

> Verificación TUI/modelos: 169 pruebas globales, incluyendo roster real normalizado, identidad visible/oculta, rango, métricas históricas, Deathmatch sin equipos ficticios, ausencia de IDs, aislamiento de demo, privacidad, celdas Unicode, tamaños objetivo, temas y navegación. Las pruebas usan fixtures y no consultan VALORANT. Validación real del nuevo roster sigue pendiente.

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

1. **Hecho (2B — diseño):** `GameStateSource` + `GamePhase`/`ProviderError`/`StateInfo` + `Process`/`Mock` + `resolve_with_fallback`.
2. **Hecho (2C base):** `LocalClientSource` + WebSocket, contexto propio, `MatchDetailSource`, `PlayerProfileSource` e `HistorySource` propios. El acceso técnico local no sustituye autorización de producto.
3. **Hecho (ADR-014 base):** roster real con equipos/participantes, nombres visibles, identidades ocultas anónimas, agentes y rango disponible, sin IDs en la TUI.
4. **Hecho (enriquecimiento):** historial acotado del roster con K/D, HS%, KAST, WR y últimas cinco; incluye filas anónimas por agente y deduplicación de partidas.
5. **Ahora:** validar latencia/campos con partidas reales y completar marcador/rondas, manteniendo degradación parcial cuando PD no entregue historial.
