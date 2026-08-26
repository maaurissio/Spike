# SPEC-LOCAL-API — Fuente primaria: Local Client API de VALORANT

> Estado: especificación pre-código (contrato). Investigación base: 2026-08-24 sobre `valapidocs.techchrism.me` y repos de la comunidad (ver `README.md:Fuentes`). **Todo lo aquí descrito funciona sin `RIOT_API_KEY` ni RSO** — la API oficial queda como mejora opcional.

## 0. Resumen y alcance

El `.exe` final de VTracker (nombre temporal) **no usa `RIOT_API_KEY` para su experiencia principal**. Lee la **Local Client API** que el cliente de VALORANT ya expone cuando está abierto: un servidor local en `127.0.0.1` con credenciales en un lockfile. Con esas credenciales obtiene tokens para los servidores remotos GLZ (partida en vivo) y PD (perfil/historial/rondas). Todo es solo-lectura, sin inyección ni memoria.

```
lockfile  ──► 127.0.0.1:{port} (Basic riot:{password}) ──► entitlement + bearer
                                                             │
                                     ┌───────────────────────┼───────────────────────┐
                                     ▼                       ▼                       ▼
                               GLZ (live)                PD (datos)              WebSocket local
                        Pre-Game / Current Game     match-history / match-details   eventos en vivo
```

## 1. Lockfile — puerta de entrada

**Ruta (Windows):** `%LocalAppData%\Riot Games\Riot Client\Config\lockfile`
**Formato:** `name:pid:port:password:protocol`
**Ejemplo:** `Riot Client:12345:21337:AbCdEfGhIjKlMnOpQrSt:Protocol`

| Campo | Uso |
|---|---|
| `port` | Puerto del servidor local (`127.0.0.1:{port}`) |
| `password` | Para Basic Auth: `riot:{password}` → base64. **Solo en memoria, nunca logueado ni persistido** (`doctor` muestra `***`). |
| `protocol` | `https` (certificado self-signed → hay que permitir inválidos) |
| `pid` | No se usa, solo diagnóstico |

**Reglas:**
* Solo existe si **VALORANT / Riot Client está abierto**. Si no existe → estado `ClientClosed` (no es error).
* Leer con manejo de UTF-8 y `Result` — antivirus o permisos pueden bloquear lectura → `doctor` explica: "VALORANT no está abierto o el antivirus bloquea el lockfile".
* No se cachea en disco; se relee cada arranque del tracker y ante `ConnectionRefused`.

**Referencias:** vRY, Vantage (`DOCUMENTATION.md:Cómo funciona`), `valapidocs: Local WebSocket` (describe el `wss://riot:{password}@127.0.0.1:{port}`).

## 2. Servidor local `127.0.0.1:{port}` — endpoints clave

Autenticación: `Authorization: Basic {base64("riot:{password}")}` + aceptar certificado self-signed.

| Endpoint | Método | Qué devuelve | Cuándo existe |
|---|---|---|---|
| `/help` | GET | Lista de endpoints locales (para descubrir `entitlements`, `sessions`) | Siempre que haya lockfile |
| `/entitlements/v1/token` | GET | `{ accessToken, token, issuer, subject }` — **tokens** para GLZ/PD (`token` es el entitlement JWT) | Siempre |
| `/product-session/v1/external-sessions` | GET | Sesión local (versión del cliente, región/shard en argumentos de arranque) | Siempre |
| `/riotclient/region-locale` | GET | Región y locale del Riot Client | Siempre |
| `wss://127.0.0.1:{port}` | WSS | Eventos en vivo (`OnJsonApiDoc`, `OnJsonPvpMatch`...) | Siempre |

**Versión del cliente (`X-Riot-ClientVersion`):** se obtiene de `/product-session/v1/external-sessions` o del log `%LocalAppData%\VALORANT\Saved\Logs\ShooterGame.log` o de `https://dash.valorant-api.com/endpoints/version`. El `X-Riot-ClientPlatform` es fijo base64 del JSON `{"platformType":"PC","platformOS":"Windows",...}` — Vantage lo hardcodea y funciona.

## 3. Tokens para GLZ y PD

De `/entitlements/v1/token`:
* `accessToken` → header `Authorization: Bearer {accessToken}`
* `token` → header `X-Riot-Entitlements-JWT: {token}`
* Siempre acompañados de `X-Riot-ClientVersion` y `X-Riot-ClientPlatform`.

**Duración:** corta (la documentación comunitaria indica alrededor de 1 hora). Ante `401/403` se refrescan releyendo `/entitlements/v1/token`. Nunca se persisten.

## 4. GLZ — partida en vivo (sin API key)

**Host:** `https://glz-{region}-1.{shard}.a.pvp.net`

Región/shard se derivan del log (`https://glz-(.+?)-1.(.+?).a.pvp.net`) o de la tabla fija: `na` → latam/br/na, `eu` → eu, `ap` → ap, `kr` → kr. `shard` también sale de `Riot Geo` con los tokens.

| Endpoint GLZ | Qué da | Cuándo |
|---|---|---|
| `GET /pre-game/v1/players/{puuid}` | Estado del jugador en Pre-Game | Solo en `PreGame`/`AgentSelect` |
| `GET /pre-game/v1/matches/{preGameMatchId}` | **Roster completo en Agent Select**: 10 jugadores con `Subject(puuid)`, `CharacterID` (agente), `PlayerIdentity` (card/title), `SeasonalBadgeInfo` (rank) | Solo en `AgentSelect` |
| `GET /core-game/v1/players/{puuid}` | Estado del jugador en partida | Solo en `InMatch` |
| `GET /core-game/v1/matches/{matchId}` | **Roster en partida** + `MapID`, `ModeID`, `ProvisioningFlow`, `Players[]` con `TeamID`, `CharacterID` | Solo en `InMatch` (`PostGameDetails: null` en vivo) |

**Uso en VTracker:**
* Detectar fase: `PreGame → AgentSelect → InMatch` vía WebSocket o polling de `Pre-Game`/`Current Game`.
* Mostrar roster: ranks/nivel/agente de los 10 en Agent Select y partida — **incluye perfiles privados** (el cliente ya los recibe; la local API no filtra por privacidad).

## 5. PD — datos del jugador y rondas

**Host:** `https://pd.{shard}.a.pvp.net`

Todos requieren los 4 headers de §3. `shard` = `na`/`eu`/`ap`/`kr`/`pbe`.

| Endpoint PD | Qué da | Cuándo |
|---|---|---|
| `GET /account-xp/v1/players/{puuid}` | Nivel de cuenta | Siempre (con puuid propio) |
| `GET /mmr/v1/players/{puuid}` | MMR/rank, historial, peak | Siempre |
| `GET /match-history/v1/history/{puuid}?start=0&end=20` | **Historial**: `History[]` con `MatchID`, `GameStartTime`, `QueueID` | Siempre |
| `GET /competitiveupdates/v1/competitiveupdates/{puuid}?start=0&end=20` | **Progreso competitivo** con cambio de RR | Siempre |
| `GET /match-details/v1/matches/{matchId}` | **Desglose por ronda** `roundResults[]` | **Post-partida (garantizado)** |
| `GET /name-service/v2/players` (PUT) | Resolución puuid → `gameName#tagLine` | Siempre |

**Desglose por ronda — modelo `roundResults[]`:**

```json
{
  "roundNum": 1,
  "roundResult": "Eliminated | Bomb detonated | Bomb defused | Surrendered | Round timer expired",
  "roundResultCode": "Elimination | Detonate | Defuse | Surrendered | ''",
  "roundCeremony": "CeremonyAce | CeremonyClutch | CeremonyFlawless | ... | ''",
  "winningTeam": "Blue | Red",
  "playerStats": [{
    "subject": "puuid",
    "kills": [{ "killer": "puuid", "victim": "puuid", "gameTime": 12345, "roundTime": 2345, "finishingDamage": {"damageType":"Weapon"} }],
    "score": 0,
    "kills": 2
  }],
  "playerEconomies": [{"subject":"puuid","loadoutValue":3900,"weapon":"...","armor":"Heavy Shields","spent":3900}],
  "playerScores": [{"subject":"puuid","score": 450}]
}
```

Reglas para VTracker (ADR-008):
* `deaths` por ronda: `0..=2` (Clove self-revive y Sage res permiten 2; Phoenix ult no genera kill/death — "matar a la nada").
* K/D agregado de partida sale de `players[].stats.kills/deaths` del scoreboard oficial, **no** de contar `kills[]` (evita sobreconteo por Phoenix).
* Si `match-details` responde a mitad de partida (raro, a verificar en 2C): el timeline se llena columna por columna. Si 404 → degradación a post-partida.

## 6. WebSocket local — eventos en vivo

**URL:** `wss://127.0.0.1:{port}` — Basic Auth `riot:{password}`, self-signed.

Suscribirse a eventos vía `/help` → `OnJsonApiDoc` y filtrar `OnJsonApiDoc` / `OnJsonPvpMatch` / `OnJsonPreGameMatch`. Librerías de referencia: `valorant-websocket-logger` y `valorant-websocket-log-viewer` (techchrism).

**Uso en VTracker:** transiciones `Lobby → PreGame → AgentSelect → InMatch → PostMatch` **event-driven** (sin polling). Fallback: polling cada N segundos si el WebSocket no está disponible.

## 7. Flujo de autenticación paso a paso (implementable)

```
1. ¿Existe lockfile?  no → ClientClosed (mensaje: "Abre VALORANT")
2. Leer port/password → Basic Auth
3. GET 127.0.0.1:{port}/entitlements/v1/token → accessToken + token(entitlement)
4. GET 127.0.0.1:{port}/product-session/v1/external-sessions → ClientVersion + argumentos de arranque
5. GET 127.0.0.1:{port}/riotclient/region-locale → región/locale cuando esté disponible
6. Con tokens → GLZ Pre-Game/Current Game (si hay partida) + PD (perfil/historial)
7. Ante 401/403 → repetir 3 (refresh). Ante ConnectionRefused → volver a 1.
```

El password del lockfile y los tokens **solo en memoria**; `doctor` los comprueba como `presente/ausente (*** )`.

## 8. Disponibilidad por fase del juego

| Fase | Lockfile | Local 127.0.0.1 | GLZ Pre-Game | GLZ Current Game | PD match-details |
|---|---|---|---|---|---|
| Cliente cerrado | ❌ | ❌ | ❌ | ❌ | ❌ |
| `Idle` / `Lobby` | ✅ | ✅ | ❌ | ❌ | ✅ (post-partida) |
| `AgentSelect` | ✅ | ✅ | ✅ | ❌ | — |
| `InMatch` | ✅ | ✅ | ❌ | ✅ | 404* |
| `PostMatch` | ✅ | ✅ | ❌ | ❌ (PostGameDetails) | ✅ completo |

*404 a mitad de partida es el comportamiento documentado; se intenta igual y se degrada con elegancia.

## 9. Región, shard y plataforma (detalles finos)

* **Región** en el log: `https://glz-eu-1.eu.a.pvp.net` → región `eu`. Alternativa: `PUT /riot/geo` con tokens de `Cookie Reauth`.
* **Shard:** tabla `na→na/latam/br`, `eu→eu`, `ap→ap`, `kr→kr`, `pbe→na`. También deducible del mismo URL del log.
* **ClientPlatform (base64):** `ew0KCSJwbGF0Zm9ybVR5cGUiOiAiUEMiLA0KCSJwbGF0Zm9ybU9TIjogIldpbmRvd3MiLA0KCSJwbGF0Zm9ybU9TVmVyc2lvbiI6ICIxMC4wLjE5MDQyLjEuMjU2LjY0Yml0IiwNCgkicGxhdGZvcm1DaGlwc2V0IjogIlVua25vd24iDQp9` funciona en todos los casos (Vantage).
* **ClientVersion:** de `/product-session/v1/external-sessions` o `https://dash.valorant-api.com/endpoints/version`.

## 10. Modelos normalizados (contrato interno)

Todo GLZ/PD se normaliza a modelos propios, versionados y con caché L1/L2 (`Arquitectura:9`):

* `Player { puuid, gameName, tagLine, teamId, characterId, rank, accountLevel, peerPublic: bool }` (perfiles privados marcados pero con rank visible vía local)
* `Round { round_num, winning_team, round_result, ceremony, players: Vec<PlayerRoundStat { puuid, kills, deaths, score, damage }> }`
* `MatchRounds { match_id, mode, rounds: Vec<Round> }` — solo para modos con ronda (ADR-004).

## 11. Errores y degradación elegante

| Error | Significado | Acción |
|---|---|---|
| Lockfile no existe | Cliente cerrado | `ClientClosed`, mensaje humano |
| `ConnectionRefused` / certificado | VALORANT recién cerrado o antivirus | Reintentar con backoff |
| `401/403` GLZ/PD | Tokens expirados | Refrescar vía `/entitlements/v1/token` |
| `404` match-details a mitad de partida | Aún no hay rondas | Mostrar "disponible al terminar la partida" |
| `429` PD/GLZ | Rate limit (raro en local) | Backoff exponencial, servir caché |

## 12. Comparativa con API oficial (por qué queda opcional)

| Aspecto | Local Client API (primaria) | API oficial Riot (`RIOT_API_KEY`) |
|---|---|---|
| Autenticación | Lockfile + tokens locales (sin key) | Production key + RSO |
| Ranks privados | ✅ Visibles (cliente los recibe) | Bloqueados sin opt-in |
| Fases en vivo | ✅ WebSocket event-driven | ❌ No expuesto en tiempo real |
| Rondas | ✅ Post-partida (match-details) | ✅ Post-partida (VAL-MATCH-V1, mismo dato) |
| Aprobación | Ninguna (solo-lectura local) | Pitch + validación de caso de uso |
| Riesgo | Gris pero solo-lectura (vRY/Vantage sin bans) | Oficial y estable |

## 13. Seguridad y privacidad

* Lectura **solo en memoria**, nunca se escribe lockfile ni se persiste el password.
* El WebSocket escucha en `127.0.0.1` — no hay exposición a red.
* `doctor` comprueba presencia de lockfile/tokens como `***` / `no disponible`.
* Solo el **usuario local** ve sus datos y el roster de su partida (datos que el cliente ya muestra). No hay exfiltración a servidores de VTracker.

## 14. Referencias (complemento a `README.md:Fuentes`)

Toda la investigación en `README.md:Fuentes consultadas` (techchrism, vRY, Vantage DOCUMENTATION.md, RumbleMike, HenrikDev). Endpoints locales listados en `valapidocs: Local WebSocket`, `Current Game Match`, `Match Details`, `Local Help`; implementación de referencia en `valorant-websocket-logger`. Si una fuente devolvió 403/404 en la sesión (Stack Overflow, Google, Riot Dev Portal raíz), se documenta para reintentar.

## 15. Preguntas abiertas para verificación empírica (2C)

1. ¿`match-details` responde a mitad de partida en algún caso? Define si la ruta de frontera en vivo existe o todo va a post-partida.
2. ¿Qué eventos exactos emite el WebSocket local en cada transición de fase? Validar nombres en 127.0.0.1 `/help`.
3. Confirmar mapeo `RoundResult`/`RoundCeremony` y `ModeID`/`MapID` con datos reales (fixture).
4. Verificar caso KAY/O downed en ult (ADR-008) con match real.
5. Confirmar que `CompetitiveUpdates` cubre el `Last 5` sin polling adicional.

---
*Última actualización: 2026-08-24 — contrato pre-código para `src/providers/lockfile.rs` → `LocalClientSource`.*
