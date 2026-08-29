# Cómo funcionan los trackers (y qué toma VTracker de cada uno)

> Nota 2026-08-28: ADR-014 habilita el desarrollo local del roster; ADR-013 conserva la advertencia para distribución. VTracker nunca desanonimiza `Incognito`.

> Investigación exhaustiva 2026-08-25 sobre Tracker.gg (Overwolf), Blitz.gg, Instalock y Vantage + val-local-api. Objetivo: entender arquitectura real vigente y qué aplicar/simplificar en un `.exe` sin BD, solo durante la partida.

## 1. Tracker.gg / Valorant Tracker (Overwolf) — 230M+ jugadores

**Arquitectura vigente 2026:**
* Desktop app + Overwolf client (3.7MB + app). Overwolf es la plataforma que provee **overlays, rastreo de eventos en tiempo real y monetización** sin tocar FPS (promesa: "Never mess up your FPS", "Always compliant" vía trabajo directo con Riot).
* Funciones: live match, stats por agente/mapa, historial, party analysis (premade), K/D, ADR, HS%, win-rate, arma/mapa, leaderboards, Discord activity (fase, playlist, score, mapa, rango, party size).
* Datos: ingame overlay + 2º monitor. Modalidades: Ranked/Unrated/Custom/Spike Rush. Premium quita ads.
* Problemas recientes (feedback tracker.gg 2025-2026): cursor oculto, "Live match has ended", login fallido — clásico race de cliente/overwolf.

**Qué adopta VTracker:**
* Idea de **Party analysis** y **Live match intel** (mostrar dúo/trío/5-stack).
* No adopta: Overwolf. VTracker usa Local Client directo (ver `SPEC-LOCAL-API.md`) — menos footprint, sin 2 runtimes.

**Limitaciones de su API para terceros:** Tracker.gg no publica API pública; Parse ofrece proxy comercial (Free 5 req/min, 200 créditos/mes → Company 500 req/min, 100k créditos). No se usará: VTracker no re-hostea stats de otros.

## 2. Blitz.gg — 240M+ jugadores, 3B+ partidas, overlay pulido

**Qué hace vigente:**
* Overlays separados: **Agent Select** (ranks de últimas partidas del acto), **Loading Screen**, **Post Match Insights**, **Dynamic Stats** (métricas de combate en tiempo real). Ajuste con `Alt+Shift`.
* Enfoque: trackear métricas clave (rank, agente, arma) con feedback inmediato; análisis post-partida con benchmarking.
* Instalación pesada (~500 MB instalación reportada por usuarios Instalock) + auto-updater.

**Qué adopta VTracker:**
* Separación de **overlays por fase** (Live vs PostMatch) inspiró `DESIGN-UI.md` (LIVE MATCH / PostMatch).
* Benchmark "tiempo real en partida + análisis profundo post" → `watch` (event-driven) vs `history`/`doctor`.

## 3. Instalock — web sin instalación (QR Riot Mobile)

**Cómo funciona (vigente julio 2026):**
* No Overwolf, no extensión. Pasos: abrir `instalock.net` → escanear QR con Riot Mobile → valorant debe estar en la misma PC donde navegas. Lee **tu lobby local** y muestra a los 10 con rank, K/D, HS%, main agent, historial reciente **antes de los 30s** de agent select.
* RAM comparativa Instalock: ~60 MB pestaña navegador vs Tracker Overwolf 200+ MB, Blitz 500+ MB, Valofessor 250+ MB. Tabla `docs/TRACKERS` inspira presupuesto de VTracker (<10 MB).
* Incognito: el cliente local ya tiene los datos → Rangos privados visibles en lobby (misma razón que VTracker puede mostrarlos).

**Qué adopta VTracker:**
* La prueba de que **el cliente local ya tiene roster completo + rank privado** → VTracker hace lo mismo pero nativo Rust, sin QR ni navegador.
* Filosofía: "live lobby data your client already has" — nuestro `LiveMatchSource`.

## 4. Vantage — referencia técnica directa para VTracker

**Stack 2026:** Rust + Tokio, Tauri v2 + React + Tailwind — casi espejo de VTracker (Rust + Ratatui). Doc `DOCUMENTATION.md` describe el pipeline que VTracker ya adopta:

```
lockfile → Basic riot:{password} (self-signed) → /entitlements/v1/token → GLZ/PD
```

Val-local-api (`ccjakje/val-local-api`, MIT, final commit 2026-02-27) es un wrapper Rust ya listo: auto-lockfile, SSL bypass, `ValorantClient::connect()`, `pregame_player/match`, `coregame_player/match`, `match_history/details/mmr/names`, y **LogWatcher SSE** (`round_ended`, `bomb_interaction`, `player_died`). VTracker puede reusar su patrón o su código como librería (dual: Rust lib + server `127.0.0.1:9922`).

**Roadmap Vantage validado:** CLI → Tauri GUI → Agent Select overlay (auto) → In-game overlay (keybind) → Tab overlay per-round (solo al mantener TAB) → Post-game summary (ACS, KAST, KDA, HS%, MVP). El **Tab overlay per-round** confirma el requisito del usuario: K kills / HS% / Damage por ronda, configurable en `config.toml` (`tab_overlay.show = ["kills","hs_percent","damage"]`).

## 5. Mapa de decisión para VTracker (sin BD, durante la partida)

| Necesidad | Cómo lo resuelven los trackers pesados | VTracker (ligero) |
|---|---|---|
| Detección lobby/partida | Overwolf Game Events + polling | **Local WebSocket** (`wss://riot:{password}@localhost:{port}`) event-driven + fallback polling local |
| Rosters + premade | GLZ Pre-Game/Current Game (Tracker/Blitz) / Instalock QR | **GLZ directo** con tokens locales (ver `SPEC-LOCAL-API.md`) |
| Ranks privados | Tracker.gg no los ve web; Instalock sí vía cliente | **Sí, vía cliente** |
| Historial/post-partida | PD match-history/details (todos) | **PD** con caché corta en RAM (moka/cache-rs) |
| Rondas por ronda | Vantage Tab overlay (SSE `round_ended`) | **LogWatcher SSE** + `match-details` (ver `SPEC-ROUNDS.md`) |

**Resultado:** VTracker replica la funcionalidad **core** de los 240M-usuarios sin su peso: <10 MB RAM, <1% CPU idle (objetivos en `docs/PERFORMANCE.md`), binario <5 MB, sin Overwolf, sin key.

## 6. Límites que los trackers también sufren (y VTracker respeta)

* Rate limits de Riot (Tracker.gg lo menciona: delays, parsing errors cuando Riot está caído). Solución: caché + backoff (ver `PERFORMANCE.md`).
* Solo personal vs agregado: sin opt-in no se muestra perfil histórico de otro (política Riot).
* Store tracking: no existe API (política lo confirma).
* Oficiales: `PublicContentCatalog` se actualiza manual tras parche — no confiar en inmediatez.

---
*Fuentes: AllValorant.GG (2024-12-16 tracker guide), Overwolf app pages, Blitz.gg/overlays, Instalock instalock.net + blog "Without Overwolf" (2026-07-10), Vantage README/DOCUMENTATION, DeepWiki vRY, val-local-api README, Entry Gaming wiki trackers (2026-03-02).*
