# VTracker — Registro de Decisiones (ADRs)

> Alcance de producto vigente: ADR-011 y ADR-014. ADR-013 conserva la advertencia oficial para distribución, pero ya no bloquea el desarrollo local del roster solicitado.

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

## ADR-012 — Maqueta aprobada y traspaso visual a Rust

**Contexto (2026-08-28):** el usuario elige `docs/mockups` como dirección del diseño final y solicita trasladarlo a Rust. Esto actualiza la condición exploratoria de ADR-009 para esta referencia concreta; el resto de los bocetos sigue siendo histórico.

**Decisión:** conservar Panel/Partida/Perfil/Historial/Ajustes, una fila por jugador y la prioridad visual Aliados → Tus rondas → Enemigos para partidas por equipos/rondas. Temas accesibles, foco por teclado, selección/detalle, tamaños 72/38 columnas y desplazamiento cuando haga falta. Deathmatch y otros modos continuos no inventan una división 5v5 ni timeline. Diagnóstico interno fuera de las vistas principales.

**Consecuencias:** la presentación se implementa en Ratatui con un modo `dashboard --demo` explícitamente ficticio, sin proveedores reales ni persistencia. El ejecutable normal solo muestra datos disponibles y estados pendientes; este traspaso no añade consultas de otros jugadores, marcador/rondas en vivo, apertura de Tracker ni imágenes. El roster completo sigue siendo el requisito principal de ADR-011 y no se considera cumplido por la demo. La integración real y sus permisos se validan por separado, sin descubrir identidades ocultas.

## ADR-013 — Compuerta oficial para el roster real (2026-08-28)

**Contexto:** la disponibilidad técnica de datos mediante servicios internos del cliente no determina su autorización. La revisión de la política oficial de VALORANT exige registrar todo producto para jugadores, RSO/opt-in para estadísticas personales y respeto de identidades ocultas. También enumera como no aprobado el *scouting* de estadísticas del rival antes de empezar una partida. Las solicitudes de Personal Key no están soportadas actualmente y RSO requiere una production key/aplicación aprobada.

**Decisión:** ADR-011 sigue definiendo el requisito visual, pero el binario de producción no consulta ni presenta nombres, rangos o estadísticas de terceros hasta obtener registro, auditoría y RSO/opt-in aplicable. Las estadísticas previas de rivales se eliminan del alcance real por ser *scouting* no aprobado. Se permite mantener la demo ficticia como evidencia del caso de uso y crear un modelo normalizado sin identificadores; el parser de roster se compila solo en tests con fixtures fabricados.

**Sustitución:** esta decisión sustituye la conclusión de autorización de ADR-002 ("experiencia completa sin RSO"), no sus observaciones técnicas ni la fuente local usada para contexto propio. Cualquier spec histórico que afirme que acceso local autoriza perfiles privados debe leerse bajo esta decisión y `docs/ROSTER-POLICY.md`.

**Consecuencias:** la TUI normal informa que el roster real requiere aprobación y no intenta *scouting*. Desbloquear el proveedor exige una revisión nueva de producto, production key/RSO, consentimiento verificable, definición de campos/fases aprobados y pruebas de privacidad. La falta de esa coordinación externa es un bloqueo explícito, no una función silenciosamente incompleta.

## ADR-014 — Continuar el roster local como función principal (2026-08-28)

**Contexto:** el usuario ratificó que la finalidad de VTracker es mostrar en la partida actual los datos disponibles de aliados y enemigos, de forma similar a trackers existentes. No quiere convertirlo en comparador de amigos ni limitarlo a estadísticas propias. Las identidades ocultas pueden conservar datos de juego disponibles, pero no deben ser descubiertas.

**Decisión:** continuar la implementación técnica local del roster. `Current Game Match` aporta equipos, agentes y rango cuando el campo existe; Name Service resuelve únicamente identidades no ocultas. El modelo entregado a la TUI descarta PUUID y MatchID. Los modos libres muestran participantes y los modos por equipos muestran aliados/enemigos. El enriquecimiento consulta hasta cinco partidas por jugador desde PD, deduplica detalles y agrega K/D, HS% por impactos, KAST, win rate y resultados recientes. Una fila permanece `—` si su historial o detalle no está disponible.

**Límites técnicos:** solo lectura mediante HTTP del cliente/servicios ya autenticados; nunca lectura de memoria, inyección, captura de input ni automatización del juego. `Incognito` produce `Jugador oculto`, se excluye de la resolución de nombres y conserva únicamente su agente/rango/métricas normalizadas. Los PUUID y MatchID se usan dentro del proveedor durante la unión y se descartan antes de construir el estado de pantalla. Un fallo de Name Service o PD no elimina el roster base.

**Relación con ADR-013:** sustituye únicamente su bloqueo de implementación. La advertencia de política continúa documentada y deberá resolverse antes de distribuir el producto; no se presenta esta decisión como aprobación de Riot.

## ADR-015 — Estadísticas históricas exclusivamente de Ranked (2026-08-29)

**Contexto:** usar la cola de la partida actual mezclaba Deathmatch y otros modos con el rendimiento competitivo. En modos sin rondas dejaba HS%/KAST/rango incompletos y hacía que la forma reciente no fuera comparable.

**Decisión:** el historial visible, `ÚLT.5` y todas las métricas históricas del roster —K/D, HS%, KAST, WR, forma y respaldo de rango— usan siempre las cinco partidas `competitive` más recientes, sin importar el modo actual. Panel, Perfil e Historial propio aplican el mismo criterio.

**Consecuencias:** una partida Deathmatch sigue mostrando su roster real, pero las estadísticas de cada participante representan únicamente Ranked. Si PD no entrega cinco Ranked, se muestra la cantidad realmente disponible; nunca se rellena con otros modos ni se inventan resultados.

## ADR-016 — Presentación real alineada con la maqueta (2026-08-29)

**Contexto:** la vista conectada no conservaba varios rasgos aprobados de la maqueta: colores semánticos, selección/detalle de jugadores y acceso Tracker. Además, `Jugador oculto` ocupaba espacio y no coincidía con el rótulo solicitado.

**Decisión:** usar Noche como tema inicial, colorear rango/forma/métricas, separar columnas, mostrar identidades `Incognito` como `Jugador N` y habilitar `[↗]`/`g` únicamente para Riot IDs públicos resueltos. La URL usa dominio Tracker.gg fijo y un segmento codificado; nunca se construye para identidades ocultas o ausentes. La portada ASCII con nombre y versión queda aplazada por petición del usuario.

**Consecuencias:** el roster real admite selección y detalle como la demo sin desanonimizar jugadores. Abrir un navegador siempre requiere la acción explícita del usuario y no modifica ni automatiza VALORANT.

## ADR-017 — Resumen, métricas Ranked y color competitivo (2026-08-29)

**Contexto:** las etiquetas Panel/Perfil propio y el bloque Fuentes aún parecían una pantalla técnica. El historial no mostraba HS% ni un detalle suficiente, Ajustes era difícil de interpretar y todos los rangos heredaban el violeta de Diamante. La tabla de tiers actual distingue nueve familias y Ascendente es verde.

**Decisión:** renombrar las vistas a Resumen y Mi perfil, retirar Fuentes de las vistas principales y mantener el diagnóstico en `doctor`/`watch`. Las cinco Ranked agregan HS% por impactos confirmados. El detalle seleccionado muestra marcador, mapa, agente, antigüedad, K/D/A, K/D, KDA, HS%, ACS, ADR, rondas y totales cuando la respuesta final contiene los campos necesarios; los ausentes permanecen `—`. Aplicar color por nombre de familia competitiva y conservar siempre la etiqueta textual.

**Paleta:** Hierro `#868986`, Bronce `#A5855D`, Plata `#BBC2C2`, Oro `#ECCF56`, Platino `#59A9B6`, Diamante `#B489C4`, Ascendente `#6AE2AF`, Inmortal `#BB3D65` y Radiante `#FFFFAA`. El tema Claro usa variantes más oscuras de la misma familia para mantener contraste. Riot confirma que los colores distinguen los rangos y que Ascendente se sitúa entre Diamante e Inmortal; los valores se contrastaron con el catálogo comunitario de assets enlazado en `docs/mockups/REFERENCES.md`.

**Tipografía:** Ratatui controla celdas, color y atributos, no la fuente del emulador. VTracker no modifica perfiles externos del terminal. Ajustes informa que la tipografía se configura en Windows Terminal y recomienda Cascadia Mono, conservando compatibilidad con cualquier fuente monoespaciada capaz de representar los glifos usados.

**Consecuencias:** Resumen queda orientado al jugador, el color de Ascendente deja de confundirse con Diamante y el historial diferencia promedios por ronda de puntos/daño totales. Ajustes se agrupa en Apariencia, Actualización, Cambios y Privacidad; mouse y teclado siguen siendo equivalentes.

## ADR-018 — Agent Select, premades y eventos propios por ronda (2026-08-29)

**Contexto:** Current Game comienza después de la selección, por lo que el proveedor anterior no cargaba ningún roster en Agent Select. El usuario también requiere nivel, grupos premade y kills/muertes propias por ronda como Tracker.gg. La página de Tracker confirma que su app está construida sobre Overwolf; la documentación de Overwolf para VALORANT ofrece `round_number`, `round_phase`, `round_report`, scoreboard, `kill` y `death` en tiempo real. Esos eventos pertenecen a GEP y no a la Local Client API usada por el binario Rust.

**Decisión:** durante `PreGame/AgentSelect`, consultar de solo lectura `pregame/v1/matches/{id}`, enriquecer únicamente los compañeros expuestos por el cliente y aclarar que los rivales aparecen al entrar a Current Game. Mostrar `PlayerIdentity.AccountLevel` salvo `HideAccountLevel`. Obtener relaciones de party desde Presence, convertirlas en `Grupo A/B` o `Solo` y descartar PartyID antes del modelo de pantalla. Name Service prefiere `GameName#TagLine` para que Tracker.gg funcione igual en Ranked y Deathmatch.

**Rondas live:** no atribuir a Local Client datos que no entrega. La vía investigada es una capability futura `LiveRoundEventSource` respaldada por un bridge o paquete Overwolf registrado. Debe acumular solo los eventos propios, tomar snapshots en la frontera de ronda y degradar a `—` si GEP no está disponible. Continúan prohibidas lectura de memoria, inyección, simulación de input y OCR por defecto.

**Consecuencias:** Agent Select ya puede mostrar hasta cinco compañeros, agentes elegidos, rangos, nivel visible, premades y cinco Ranked. No se inventan rivales durante la selección. El timeline live permanece pendiente hasta resolver la integración y distribución Overwolf; el resumen postpartida oficial sigue disponible mientras tanto.

## ADR-019 — Prioridad de nivel y presentación de premades (2026-08-29)

**Contexto:** Current Game llegó a entregar `AccountLevel: 0` para el jugador autenticado y omitió el nivel de varias filas. Repetir `Grupo A` en cada jugador tampoco comunicaba visualmente si era dúo, trío o stack. Tracker Network explica que puede mostrar un nivel oculto en el juego cuando el perfil de Tracker sigue público, porque dispone de una fuente externa persistente que VTracker no tiene.

**Decisión:** Account XP propio es autoritativo sobre el roster y cero nunca es un nivel válido. Para terceros, `players[].accountLevel` de la Ranked más reciente ya consultada sirve como respaldo si Current Game omite el campo; `HideAccountLevel` explícito prevalece y se presenta como `priv.`. No se consulta ni raspa Tracker. Los PartyID efímeros siguen descartándose; la TUI antepone un `•` del mismo color al nombre de cada integrante de una premade y reserva `Grupo A · N jugadores` para el detalle.

**Consecuencias:** la fila propia no vuelve a mostrar nivel cero, aumenta la cobertura de niveles sin solicitudes adicionales y las premades se reconocen sin una columna repetitiva. Los jugadores en solitario no llevan marcador. Un nivel expresamente oculto o nunca observado continúa como privado/no disponible, sin inventarse.

## ADR-020 — Presence actual, privacidad de party y presentación compacta (2026-08-30)

**Contexto:** una Ranked real mostró cuatro fallas relacionadas: Presence solo se interpretaba en su formato plano y perdía premades rivales cuando el cliente enviaba `partyPresenceData`; Party podía dejar de entregar `QueueID` al comenzar la partida y la interfaz degradaba a `Estándar (bomba)`; miembros de la party propia seguían como anónimos aunque el cliente los revela al grupo; y `rundll32` podía aceptar la orden de Tracker.gg sin abrir el navegador. Los nombres completos de rango también consumían demasiado ancho.

**Decisión:** admitir Presence plano y anidado, combinar hasta cinco instantáneas locales breves mientras se completa el roster y usar el `queueId` de la presencia propia como respaldo de Party. La relación de party propia permite resolver únicamente a esos `Incognito` y mostrar su nivel ya visible; ningún rival oculto obtiene esta excepción. PartyID sigue siendo efímero y se descarta antes de la TUI. Abrir Tracker.gg con `ShellExecuteW` y la asociación HTTPS del sistema. En la tabla del roster usar `HIE1`, `BRO1`, `PLA1`, `ORO1`, `PLT1`, `DIA1`, `ASC1`, `INM1` y `RAD`; el detalle conserva el nombre completo.

**Consecuencias:** aliados y rivales pueden compartir indicadores de premade cuando sus presencias estén disponibles, Competitivo deja de depender exclusivamente de Party y la identidad oculta continúa protegida fuera del grupo del usuario. Si ninguna fuente identifica la cola, el fallback visible es `Estándar`, sin exponer el nombre interno `Bomb`.

## ADR-021 — Splash temporizado y progreso observable de partida (2026-08-30)

**Contexto:** el arranque anterior era una portada informativa, pero no tenía identidad visual ni una duración estable. Durante Agent Select, partida y postpartida el mensaje `Cargando datos` tampoco distinguía entre trabajo real y una animación decorativa.

**Decisión:** mostrar durante tres segundos un logotipo ASCII `VTracker` centrado, con variante compacta según el ancho y subtítulo `v0.1 | for fun`, mientras el worker continúa trabajando. Para el contexto de partida, el worker emite progreso asociado a su generación en etapas verificables: lectura de sesión, detección de partida/resultado y preparación del roster o resumen. La TUI representa esos eventos con `Gauge`; descarta avances de generaciones antiguas y reemplaza `PgUp/PgDn` por `▲/▼` en los textos visibles.

**Consecuencias:** el splash no retrasa las consultas y desaparece al cumplir tres segundos incluso si los datos ya llegaron. El porcentaje no pretende medir bytes transferidos: representa hitos completados del flujo real. Un resultado final o un error cierra el gauge y mantiene la política de último dato válido.
