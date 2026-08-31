# AGENTS — Roster completo vigente 2026

> Verificado 2026-08-25 contra `https://valorant-api.com/v1/agents?isPlayableCharacter=true` (29 agentes) + guías 2026-08 (gamingcy, esportsinsider). Uso en VTracker: mapear `CharacterID` (UUID de GLZ/PD) → nombre + rol para mostrar agente correcto en roster, sin adivinar. Lo vinculante es el UUID; el nombre es solo display.

## Resumen por rol

**29 agentes jugables** — 8 Duelistas, 7 Controladores, 7 Iniciadores, 7 Centinelas.

| Rol | Agentes (2026) | Trabajo en la ronda |
|---|---|---|
| **Duelist** (8) — entra primero, busca duelos | Iso, Jett, Neon, Phoenix, Raze, Reyna, Waylay, Yoru | Timing de entrada, tradeo, escape |
| **Controller** (7) — bloquea visión, controla espacio | Astra, Brimstone, Clove, Harbor, Miks, Omen, Viper | Humos a tiempo, presión de mapa |
| **Initiator** (7) — abre con info/disrupción | Breach, Fade, Gekko, KAY/O, Skye, Sova, Tejo | Limpiar ángulos, habilitar entrada |
| **Sentinel** (7) — ancla, antí-flanco, control tardío | Chamber, Cypher, Deadlock, Killjoy, Sage, Veto, Vyse | Trampas, anclaje, retake |

## Roster completo (orden alfabético, con UUID para `analytics::agent_from_uuid`) — 29 agentes

> Verificado directo contra `valorant-api.com` 2026-08-25 (ver `python fetch_agents.py` arriba).

| Agente | Rol | UUID |
|---|---|---|
| Astra | Controller | `41fb69c1-4189-7b37-f117-bcaf1e96f1bf` |
| Breach | Initiator | `5f8d3a7f-467b-97f3-062c-13acf203c006` |
| Brimstone | Controller | `9f0d8ba9-4140-b941-57d3-a7ad57c6b417` |
| Chamber | Sentinel | `22697a3d-45bf-8dd7-4fec-84a9e28c69d7` |
| Clove | Controller | `1dbf2edd-4729-0984-3115-daa5eed44993` |
| Cypher | Sentinel | `117ed9e3-49f3-6512-3ccf-0cada7e3823b` |
| Deadlock | Sentinel | `cc8b64c8-4b25-4ff9-6e7f-37b4da43d235` |
| Fade | Initiator | `dade69b4-4f5a-8528-247b-219e5a1facd6` |
| Gekko | Initiator | `e370fa57-4757-3604-3648-499e1f642d3f` |
| Harbor | Controller | `95b78ed7-4637-86d9-7e41-71ba8c293152` |
| Iso | Duelist | `0e38b510-41a8-5780-5e8f-568b2a4f2d6c` |
| Jett | Duelist | `add6443a-41bd-e414-f6ad-e58d267f4e95` |
| KAY/O | Initiator | `601dbbe7-43ce-be57-2a40-4abd24953621` |
| Killjoy | Sentinel | `1e58de9c-4950-5125-93e9-a0aee9f98746` |
| Miks | Controller | `7c8a4701-4de6-9355-b254-e09bc2a34b72` |
| Neon | Duelist | `bb2a4828-46eb-8cd1-e765-15848195d751` |
| Omen | Controller | `8e253930-4c05-31dd-1b6c-968525494517` |
| Phoenix | Duelist | `eb93336a-449b-9c1b-0a54-a891f7921d69` |
| Raze | Duelist | `f94c3b30-42be-e959-889c-5aa313dba261` |
| Reyna | Duelist | `a3bfb853-43b2-7238-a4f1-ad90e9e46bcc` |
| Sage | Sentinel | `569fdd95-4d10-43ab-ca70-79becc718b46` |
| Skye | Initiator | `6f2a04ca-43e0-be17-7f36-b3908627744d` |
| Sova | Initiator | `320b2a48-4d9b-a075-30f1-1f93a9b638fa` |
| Tejo | Initiator | `b444168c-4e35-8076-db47-ef9bf368f384` |
| Veto | Sentinel | `92eeef5d-43b5-1d4a-8d03-b3927a09034b` |
| Viper | Controller | `707eab51-4836-f488-046a-cda6bf494859` |
| Vyse | Sentinel | `efba5359-4016-a1e5-7626-b1ae76895940` |
| Waylay | Duelist | `df1cb487-4902-002e-5c17-d28e83e78588` |
| Yoru | Duelist | `7f94d92c-4234-0a36-9646-3a87eb8b5c89` |

> **Estrategia actual de VTracker:** la vista en vivo contiene una tabla local mínima UUID→nombre para los agentes de esta versión. No añade consultas de red durante una partida y, si llega un UUID nuevo, muestra `no disponible` en vez de adivinar. Un catálogo actualizable en runtime queda como mejora posterior.

### Forma segura (recomendada para VTracker)

```rust
// Al iniciar la sesión, sin bloquear la TUI:
let agents: Vec<Agent> = reqwest::get("https://valorant-api.com/v1/agents?isPlayableCharacter=true")
    .await?.json::<ApiEnvelope>().await?.data;
// agents[i].uuid, agents[i].displayName, agents[i].role.displayName
// Guardar en L1 (moka) TTL 24h. Si falla, degradar a IDs recortados.
```

Estrategia adoptada en `val-local-api` y Vantage: nunca fijar UUIDs en el binario; mapear dinámico y si un agente nuevo (ej. Miks) no está en el mapa, mostrar UUID corto + advertencia.

## Detalle por agente (v1, exhaustivo pero no final)

Cada agente tiene 4 habilidades: 2 básicas (compra, 100-250 créditos), 1 firma (recarga por tiempo/kill) y 1 definitiva (6-9 puntos). El detalle fino (coste, duración, daño) cambia por parche — por eso VTracker **no hardcodea** valores de balance; solo necesita `nombre`, `rol`, `uuid` e `icono` para el roster. Los detalles de habilidad se consultan en `valorant-api.com/v1/agents/{uuid}` cuando se necesiten (tooltip futuro).

*Ejemplo (verificable):*
* **Brimstone (Controller, USA):** Incendiary (molly), Stim Beacon (buff), Sky Smokes (3 humos globales), Orbital Strike (ult láser)
* **Clove (Controller, Scotland — no binario, primer agente no binario del roster):** smokes tras morir, revive self-ult (ver `SPEC-ROUNDS.md: deaths 0-2`)
* **Miks (Controller, Croatia):** 29º, 18-mar-2026, kit de sonido/ritmo (audio como arma) — ver gamingcy 2026-08-03
* **Waylay (Duelist):** una de las 8 Duelists recientes

Lista completa y assets (iconos, voiceLines): `https://valorant-api.com/v1/agents`.

## Distribución y curiosidades vigentes

* **Reparto 8/7/7/7:** Duelist es el rol con más opciones (presión de entry), Controllers e Initiators equilibrados, Sentinels anclan.
* **Género (referencial):** 12 femeninas (Viper, Killjoy, Sage, Jett, Reyna, Raze, Skye, Astra, Neon, Fade, Deadlock, Vyse, Waylay) — Clove no binario — resto masculinos (gamingcy). No afecta mecánicas.
* **Agente 8 (VP-08):** nunca lanzado; el roster salta de VP-07 a VP-09 — por eso Veto es VP-29 pero solo hay 29 jugables.
* **Release:** beta 10 agentes (Brimstone, Viper, Omen, Cypher, Sova, Sage, Phoenix, Jett, Raze, Breach), luego Reyna (11, 2020-06-02) → Killjoy, Skye, Yoru ... → Miks (29, 2026-03-18).

## Uso en VTracker (sin perderse)

* **Fase `AgentSelect`:** `GLZ pre-game/match` da `CharacterID` (UUID de agente ya lockeado o vacío si no ha elegido) → mapear a nombre/rol/icono vía el mapa dinámico. Si `CharacterSelectionState=""` → "eligiendo..."
* **Fase `InMatch`:** `GLZ core-game` da agente fijo; cachear para no re-resolver.
* **Post-partida:** `Match Details` da `characterId` final (puede ser distinto si hubo dodge).
* **Tests:** fixture `agents.json` recortado (3 agentes) para `agent_from_uuid`.

## Referencias verificadas 2026-08-25

* `valorant-api.com/v1/agents?isPlayableCharacter=true` — **fuente de verdad** (29 agentes, vigente, sin auth). Verificado con `python -c` arriba.
* `dash.valorant-api.com` — dashboard/docs del mismo servicio (Agents, Maps, Weapons...).
* `gamingcy.com/blog/valorant-agents-guide-tier-list` (2026-08-03) — 29 agentes, roles 8/7/7/7, Miks 29º, Clove no binario, tier Clove 55.06%.
* `esportsinsider.com/valorant-agents` (2026-03-05) — roles 6C/8D/7S/7I (pre-Miks: 28 agentes, Agent 8 misterioso, Agent 30 croata especulado).
* `playvalorant.com` / `valorantpy.readthedocs.io` — descripciones históricas (11 agentes) para contraste.

---
*Última actualización: 2026-08-25 — se completará mapeando UUIDs exactos contra el JSON real antes de implementar `agent_from_uuid`.*
