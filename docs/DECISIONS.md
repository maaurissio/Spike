# VTracker — Registro de Decisiones (ADRs)

> Alcance de producto vigente: ADR-011. Las afirmaciones históricas sobre acceso técnico sin RSO no constituyen validación de permisos para mostrar datos de terceros.

> Cada decisión relevante se registra con contexto, decisión y consecuencias. Numeradas y inmutables: una decisión nueva que revoca otra crea un nuevo ADR que la referencia. Detalle técnico en los specs (`docs/SPEC-*.md`).

## ADR-001 — Nombre temporal `VTracker`
**Contexto:** el producto necesita un nombre durante el desarrollo, pero la marca final aún no está elegida.
**Decisión:** usar `VTracker` como nombre provisional. Evitar hardcodear la marca en mensajes de usuario más allá de `VERSION`. El renombrado final ocurre en Prioridad 6 (release).
**Consecuencias:** los docs usan el nombre con la nota "temporal"; el renombrado toca `Cargo.toml`, binario, README y docs.

## ADR-002 — Fuente primaria: Local Client API (lockfile)
**Contexto:** la API oficial de Riot para VALORANT no expone estado en tiempo real y requiere production key + RSO (aprobación lenta). La investigación (2026-08-24, ver README:Fuentes) demostró que la API local del cliente (lockfile + REST `127.0.0.1` + WebSocket `wss` + tokens para GLZ/PD) cubre fases reales, roster en vivo, perfil, historial y rondas post-partida.
**Decisión:** la fuente primaria es la Local Client API. Sin API key de producción ni RSO para la experiencia completa. La API oficial queda como mejora opcional posterior.
**Consecuencias:** desbloquea el desarrollo inmediato; lectura solo-lectura (sin inyección ni memoria); password del lockfile solo en memoria, enmascarado en `doctor`.

## ADR-003 — Desacople por capabilities (`GameStateSource`)
**Contexto:** la app no debe depender de una fuente concreta (procesos, local API, mock, Riot oficial).
**Decisión:** trait `GameStateSource` (`src/providers/capabilities.rs`) con `GamePhase` fino/grueso, `Confidence`, `ProviderError` y `resolve_with_fallback`. Implementaciones: `ProcessGameStateSource` (procesos), `MockGameStateSource` (TUI/tests), futuro `LocalClientSource`.
**Consecuencias:** la TUI se construye contra mocks en paralelo a los providers reales; el contrato ya está congelado y testeado (58 tests).

## ADR-004 — Alcance del tracking de rondas
**Contexto:** no todos los modos tienen rondas (DM/TDM/Escalation son continuos). El requisito del usuario es el desglose por ronda.
**Decisión:** el RoundTimeline aplica **solo** a Unrated, Competitivo (con OT) y Personalizadas en formato estándar/competitivo. **Fuera de alcance:** Swiftplay, Deathmatch, Team Deathmatch, Escalation — en esos modos la vista live muestra roster/stats y al final un resumen básico sin rondas.
**Consecuencias:** `ModeID` + `ProvisioningFlow` (en vivo, vía Current Game Match) deciden si la vista de rondas existe.

## ADR-005 — Snapshot por frontera de ronda (datos certeros)
**Contexto:** actualizar en caliente produce datos a medio cocinar. El requisito del usuario: "estoy en la ronda 6, mato a 3 y muero — se actualiza cuando empieza la ronda 7".
**Decisión:** los datos de ronda se toman como snapshot en la frontera (fin de ronda → inicio de la siguiente), nunca en caliente. Rutas en orden de preferencia: (1) `match-details` en la frontera si responde a mitad de partida; (2) OCR opt-in (top HUD para detectar frontera + scoreboard solo cuando el usuario abre TAB — nunca simulamos input); (3) post-partida garantizada.
**Consecuencias:** cero condiciones de carrera; degradación elegante; el timeline en vivo muestra solo rondas completadas.

## ADR-006 — Sin OCR por defecto, sin lectura de memoria, sin input simulado
**Contexto:** kills en vivo dentro de la ronda no están expuestos por ninguna API. Existen OCR del killfeed y lectura de memoria.
**Decisión:** lectura de memoria: **nunca** (contra los principios del proyecto y riesgo de ban). Input simulado (apretar TAB por el usuario): **nunca**. OCR: **opt-in, desactivado por defecto**, y solo lectura de pantalla (top HUD siempre visible + scoreboard cuando el usuario lo abre).
**Consecuencias:** el comportamiento garantizado es post-partida; lo en vivo es best-effort con degradación elegante.

## ADR-007 — K/D agregado desde scoreboard oficial, no del killfeed
**Contexto:** el killfeed (`kills[]` en match-details) puede diferir del scoreboard en casos especiales (ver ADR-008).
**Decisión:** K/D, KDA y agregados de partida salen de `players[].stats` (scoreboard oficial). El killfeed se usa para el desglose por ronda y eventos, con las diferencias documentadas como caso borde.
**Consecuencias:** los agregados siempre coinciden con lo que muestra el juego.

## ADR-008 — Reglas de muerte por ronda: 0, 1 o 2
**Contexto:** mecánicas de reviva afectan el conteo: Clove (self-revive) y Sage (res) permiten morir 2 veces en una ronda — ambas muertes cuentan. Phoenix (Run It Back): morir dentro de la ult NO genera kill para el enemigo NI death para Phoenix ("matar a la nada"); si Phoenix mata en ult, la kill SÍ cuenta. Pendiente verificar: KAY/O (estado downed en ult).
**Decisión:** `deaths` por ronda es **conteo (0-2)**, no booleano. El K/D total puede dar muertes > rondas jugadas (revives) — se usa el conteo real, igual que el juego. La diferencia killfeed-vs-scoreboard por Phoenix se documenta como caso borde del desglose por ronda.
**Consecuencias:** KAST "Survived" requiere definición cuidadosa con revives (pendiente, ver SPEC-ROUNDS).

## ADR-009 — Documentación distribuida en specs
**Contexto:** README y Arquitectura-inicial no deben colapsar con detalle de features.
**Decisión:** los detalles viven en `docs/` (`SPEC-ROUNDS.md`, `DESIGN-UI.md`, `ISO.md`, este archivo). README mantiene visión general + índice; Arquitectura mantiene decisiones estructurales. **Todo mockup/diseño de UI en los specs es CONCEPTUAL y exploratorio — no es el diseño final**; lo vinculante son los requisitos, modelos de datos y reglas de comportamiento. El diseño visual final se define en implementación (P5).
**Consecuencias:** cada feature tiene su spec navegable; el código referencia specs por ruta; ninguna decisión visual queda cerrada antes de probar en pantalla real.

## ADR-010 — Sistema integrado ISO 9001/14001/27001 proporcional
**Contexto:** el proyecto quiere calidad auditable, impacto ambiental medido y seguridad de la información desde el inicio, sin burocracia de gran empresa.
**Decisión:** adoptar los tres sistemas como sistema integrado PHVA proporcional (ver `docs/ISO.md`). Certificación formal opcional a medio plazo.
**Consecuencias:** tests+clippy+fmt (calidad), medición de recursos (ambiental), `.env`+enmascaramiento+auditorías (seguridad) son parte del flujo normal.

## ADR-011 — Roster de la partida como requisito principal (2026-08-28)
**Contexto:** el usuario confirmó que la idea principal es ver a los diez jugadores de la partida con sus rangos y estadísticas. La restricción a datos propios introducida en `TASKS.md` y partes del README contradice ese objetivo; no representa una exclusión aprobada por el usuario.
**Decisión:** el producto debe mostrar aliados y enemigos en partida (diez jugadores en modos 5v5; tamaño real del roster en otros modos), sus agentes, rangos y estadísticas históricas cuando estén disponibles y su uso esté permitido. Perfil propio, historial, rondas y resumen postpartida complementan esta función. Esta decisión sustituye las exclusiones de roster del alcance, pero no elimina las restricciones de privacidad ni autoriza consultas sin validar sus condiciones.
**Límites:** validar por fuente los permisos, términos, consentimiento y campos disponibles antes de implementar consultas de terceros. Respetar identidades ocultas y controles de acceso; indicar `no disponible` sin inferir identidades ni inventar estadísticas. No prometer K/D de la partida en curso a partir de estadísticas históricas. La disponibilidad de tokens locales no demuestra autorización de uso.
**Estado:** pendiente de implementación y validación real. El código actual muestra solo contexto propio; no cumple todavía este requisito principal.
**Consecuencias:** priorizar validación de fuentes, modelo de roster y presentación de aliados/enemigos. Si una restricción impide parte del requisito, documentarla y consultarla con el usuario, sin redefinir silenciosamente el producto. Solo considerar terminado el requisito con pruebas de normalización, privacidad y ausencia de datos, más evidencia en una partida real.
