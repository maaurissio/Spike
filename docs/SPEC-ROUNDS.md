# SPEC-ROUNDS — Tracking y visualización de rondas

> Estado: especificación en refinamiento (pre-código). Decisiones estructurales en `docs/DECISIONS.md`. Este documento define el modelo de datos, reglas de negocio, rutas de datos y casos borde del feature de rondas.

## 1. Alcance por modo (ADR-004)

| Modo | ¿RoundTimeline? | Notas |
|---|---|---|
| Unrated (Normal) | ✅ | Primero a 13 |
| Competitivo | ✅ | Primero a 13 + OT (rondas OT continúan numeración: R25, R26…) |
| Personalizada formato estándar/competitivo | ✅ | `ProvisioningFlow: CustomGame` + modo base estándar |
| Swiftplay | ❌ | Fuera de alcance |
| Deathmatch / Team Deathmatch / Escalation | ❌ | Continuos; resumen básico post-partida sin rondas |

Detección en vivo: `Current Game Match` → `ModeID` + `ProvisioningFlow`. Si el modo no está en la lista → la vista de rondas no existe.

## 2. Modelo de datos (normalizado, fuente-agnóstico)

```rust
// Borrador — sujeto a refinamiento antes de implementar

pub struct MatchRounds {
    pub match_id: String,
    pub mode: GameMode,            // solo variantes con ronda llegan aquí
    pub rounds: Vec<Round>,
}

pub struct Round {
    pub round_num: u32,            // 1..=n, OT continúa la numeración
    pub winning_team: Team,        // Blue | Red
    pub round_result: RoundResult, // Eliminated | Detonate | Defuse | Surrendered | TimerExpired
    pub ceremony: Option<RoundCeremony>, // Ace | TeamAce | Clutch | Flawless | Thrifty | Closer | Default
    pub players: Vec<PlayerRoundStat>,
}

pub struct PlayerRoundStat {
    pub puuid: String,
    pub kills: u8,                 // 0..=5
    pub deaths: u8,                // 0..=2  (ADR-008: Clove/Sage permiten 2)
    pub score: Option<u32>,        // combat score de la ronda si la fuente lo entrega
    pub damage: Option<u32>,       // daño total de la ronda si la fuente lo entrega
}
```

Reglas:
* `deaths` es **conteo (0-2)**, nunca booleano (ADR-008).
* El K/D de partida sale de `players[].stats` del scoreboard oficial (ADR-007); puede dar muertes > rondas jugadas por revives.
* Campos `Option<>` = la fuente puede no entregarlos; nunca se inventan (principio de honestidad de datos).

## 3. Rutas de datos (orden de preferencia — ADR-005)

1. **`match-details` en frontera de ronda** — al detectar fin de ronda, snapshot de `pd.{shard}.a.pvp.net/match-details/v1/matches/{id}`. Si responde a mitad de partida: timeline se llena columna por columna en vivo (comportamiento ideal).
2. **OCR opt-in (futuro, desactivado por defecto)** — top HUD del juego (número de ronda siempre visible, franja fija) para detectar la frontera R6→R7 exacta; scoreboard solo cuando el usuario abre TAB. Nunca se simula input (ADR-006).
3. **Post-partida (garantizado)** — `match-details` completo al terminar; la vista PostMatch siempre tiene el desglose íntegro.

Degradación: si (1) responde 404 a mitad de partida → silenciosamente se espera a (3). Si OCR está desactivado o falla → no afecta (2)/(3).

## 4. Detección de la frontera de ronda

* **Vía datos:** comparación de `roundResults.len()` entre snapshots sucesivos (si (1) funciona, la frontera es gratis).
* **Vía OCR opt-in:** cambio del número de ronda en el top HUD.
* **Vía WebSocket local:** si existen eventos de transición de estado del juego (a verificar empíricamente en 2C; si solo marca inicio/fin de partida, sirve para (3)).

## 5. Visualización — RoundTimeline

> **⚠️ CONCEPTUAL — NO es el diseño final.** El arte ASCII y las reglas visuales de esta sección son exploratorios (validan el concepto del usuario). Lo vinculante: el modelo de datos (§2), las rutas de datos (§3) y las reglas de negocio. El render final se decide en implementación (P5).

Concepto del usuario (prototipo validado): kills apilados hacia arriba, muertes hacia abajo, una columna por ronda.

```
          K
          K
   K      K  K      K
   K  K   K  K  K   K
  ─────────────────────────
   R1 R2  R3 R4 R5  R6 …
  ─────────────────────────
   D  D         D   D
   D
```

Lectura del ejemplo: R1 = 3K + 2D (revivido), R2 = 1K + 1D, R3 = 2K, R4 = 0K, R5 = 4K + 1D (★ si fue ACE/CLUTCH), R6 = 1K + 1D.

Reglas de render:
* **Colores:** K verde, D rojo; etiqueta `R#` verde si ganamos la ronda, rojo si la perdimos; rondas futuras atenuadas; `★` sobre la columna si `ceremony` = Ace/Clutch.
* **Elástico:** columnas por bloque = ancho disponible / 3-4 chars, recalculado cada frame (Ratatui entrega el área por frame). Bloques apilados verticalmente si no caben todas las rondas.
* **Alto de bloque fijo:** 5 filas K + separador + etiquetas + separador + 2 filas D ≈ 10 filas.
* **Paginación:** si los bloques no caben en alto → `←/→` o auto-scroll al bloque actual.
* **Colapso:** en terminal angosta, el timeline colapsa a una línea (`R7 · 6K 4D · [enter] expandir`).
* **Convive con rosters** (ver `DESIGN-UI.md`): dos columnas en terminal ancha; apilado/colapsado en angosta.

## 6. Casos borde y pendientes de definición

| Caso | Estado | Decisión pendiente |
|---|---|---|
| Clove self-revive: 2 muertes en ronda | ✅ Definido (conteo 0-2) | — |
| Sage res: 2 muertes en ronda | ✅ Definido (conteo 0-2) | — |
| Phoenix ult: sin kill para enemigo, sin death para Phoenix; kills de Phoenix en ult SÍ cuentan | ✅ Definido (ADR-008) | Cómo marcar en timeline (¿columna sin D aunque hubo "muerte" visual?) |
| KAY/O downed en ult | ⚠️ Verificar empíricamente | ¿Cuenta como muerte? |
| KAST "Survived" con revives | ⚠️ Pendiente | Definir con datos reales de fixture |
| Diferencia killfeed vs scoreboard por ronda (Phoenix) | ⚠️ Pendiente | ¿Nota al pie en la columna o aceptar diferencia? |
| Rendición (surrender) a mitad de ronda | ⚠️ Verificar | ¿Cómo la reporta `roundResults`? |
| Partida abandonada (reconnect) | ⚠️ Verificar | Rondas ausente: `wasAfk`/`wasPenalized` existen en la fuente |

## 7. Fixtures de prueba (definición temprana)

* `fixture_5_rounds`: el ejemplo del usuario — 5 rondas, muere en R1/R2/R5, mata 2 en R1/R3/R4/R5 → valida conteo, colores y wrap.
* `fixture_ot`: partida 13-11 con OT → valida numeración continua y paginación.
* `fixture_clove`: ronda con 2 muertes → valida pila de D.
* `fixture_phoenix`: kill event en ult sin death en scoreboard → valida diferencia killfeed/scoreboard.
* `fixture_narrow`: render en 40 cols → valida wrap a bloques.

## 8. Preguntas abiertas para siguiente iteración

1. ¿El timeline muestra también a otros jugadores seleccionables desde el roster (aliados/enemigos) o solo al usuario?
2. ¿Fila opcional de daño por ronda (configurable en Settings)?
3. ¿Auto-scroll al bloque actual durante partida en vivo o manual?
4. Confirmar empíricamente si `match-details` responde a mitad de partida (paso 2C) — define si la ruta (1) existe o todo va a (3).
