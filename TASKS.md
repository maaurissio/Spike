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

## Prioridad 2 — Fuente de estado y datos (API) — ¿cuándo entra la API?

> La API entra AQUÍ, después de cerrar Prioridad 1. Ya está planificada con estructura segura.

### 2A — Seguridad y secretos (hacer PRIMERO, antes de pedir cualquier key)

- [x] Proteger secretos en repo: `.env` en `.gitignore` (`/.gitignore:4`), `.env.example` como plantilla y `config.example.toml` documentado sin secretos.
- [ ] Crear `.env` local desde `.env.example` (`Copy-Item .env.example .env`) y nunca commitear claves reales.
- [ ] Cargar secretos solo desde variables de entorno en runtime (no hardcodear). `doctor` debe mostrar `***` o `no configurada`, nunca el valor.
- [ ] Documentar flujo de secretos en `README.md` y `Arquitectura-inicial.md:10` (límites de producto y cumplimiento).

### 2B — Diseño de proveedores (sin implementar red hasta validar)

- [ ] Validar la documentación, autenticación, consentimiento, límites y políticas del primer proveedor (Riot Developer Portal).
- [x] Crear una interfaz `GameStateSource` (`src/providers/capabilities.rs`) para que el resto de la app no dependa de un proveedor concreto.
- [x] Definir estructura `src/providers/` con `mod.rs`, `capabilities.rs`, `process.rs`, `mock.rs` según `Arquitectura-inicial.md:11`.
- [x] Modelar estados `Lobby`, `PreGame`, `AgentSelect`, `InMatch` y `PostMatch` + `GamePhase` y `Confidence` solo para fuentes autorizadas; `GameOpen` preserva honestidad de detección por procesos.
- [x] Implementar `ProcessGameStateSource` (wrapper honesto de `game::detect`) y `MockGameStateSource` para validar TUI sin red.
- [x] Resolver con `resolve_with_fallback` y mostrar último estado conocido cuando una fuente falle (retryable → fallback, auth → no fallback).

### 2C — Implementación autorizada (solo tras validar 2B)

- [ ] Implementar el primer adaptador autorizado para estado de cliente/partida (requiere `RIOT_API_KEY` en `.env`).
- [ ] Extender `vtracker doctor` para validar la disponibilidad del proveedor, sin exponer secretos.

## Prioridad 3 — Datos propios e historial + Experiencia por fase

- [ ] Definir modelos normalizados de jugador, partida, ronda y resultado.
- [ ] Implementar una capa de providers por capacidades: perfil, historial y detalle de partida.
- [ ] Implementar caché L1 en memoria con TTL.
- [ ] Implementar caché L2 en disco con versión de esquema y expiración.
- [ ] Centralizar solicitudes con timeout, deduplicación y reintentos seguros.
- [ ] Añadir el comando `vtracker history` para consultar el historial propio autorizado.
- [ ] **Flujo perfil:** al detectar `Idle`/`Cliente disponible`, mostrar perfil propio (Riot ID configurado) con stats cacheadas; si provider falla, mostrar último dato conocido y error recuperable.
- [ ] **Flujo equipo:** al detectar `PreGame`/`AgentSelect` vía `GameStateSource` autorizado, consultar `RosterSource` y mostrar stats del equipo para esa partida.
- [ ] **Flujo partida:** al detectar `InMatch`, mostrar stats generales del game en curso (mapa, modo, composición) sin inferir de procesos locales.

## Prioridad 4 — Estadísticas

- [ ] Crear el módulo `analytics` separado de providers y UI.
- [ ] Calcular K/D, KDA y win rate a partir de datos normalizados.
- [ ] Calcular HS%, ADR y ACS solo cuando la fuente entregue los campos necesarios.
- [ ] Documentar tratamiento de empates, abandonos, overtime y modos no competitivos.
- [ ] Añadir desgloses por agente, mapa, modo y últimas N partidas.
- [ ] Cubrir las fórmulas con fixtures y pruebas reproducibles.

## Prioridad 5 — TUI completa + Apartado Configuración

> **Configuración es requisito explícito del usuario** — debe ser completa antes del release.

- [ ] Añadir Ratatui y Crossterm (Arquitectura Elm/TEA: Model/Message/Update/View).
- [ ] Crear Dashboard con estado, salud de fuentes y resumen de sesión (perfil propio destacado).
- [ ] Crear vistas Match, Player, History y **Settings (Configuración)**.
- [ ] **Settings debe permitir:** ver/editar `config.toml` (intervalo, `log_transitions`, `profile.riot_id`, `profile.region`, `autostart.enabled/minimized`), gestionar `.env` (solo estado `***`/`no configurada`), TTL de caché y apariencia.
- [ ] Añadir `vtracker config show|edit|validate` y persistencia atómica de `config.toml`.
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

1. **Hecho (2B — diseño):** `GameStateSource` + `GamePhase`/`ProviderError`/`StateInfo` + `ProcessGameStateSource`/`MockGameStateSource` + `resolve_with_fallback` (58 tests, `src/providers/*`).
2. **Ahora:** validar documentación/autenticación/límites del primer proveedor (Riot Developer Portal, RSO/opt-in) — única tarea pendiente de 2B.
3. **Después (2C):** implementar `src/providers/riot.rs` con `RIOT_API_KEY` protegida y extender `doctor` sin exponer secretos. No inferir estado de procesos locales.
