# PERFORMANCE — RAM, cache sin BD y mínimo uso de CPU (solo durante la partida)

> El roster enriquecido está habilitado por ADR-014 y vive solo en memoria durante la consulta, sin PUUID ni MatchID en el estado de pantalla. Cada carga consulta como máximo 12 jugadores × 5 partidas, ejecuta hasta 6 solicitudes simultáneas y deduplica `match-details` compartidos antes de agregarlos.

> Principio: sin base de datos, sin persistencia innecesaria. Todo vive **solo mientras dura la sesión de juego**. Inspirado en RATATUI #1338, Zellij, `moka-rs`, `cache-rs`, `CacheKit` (2026), `rustfaq: binary size` y `val-local-api` SSE. Presupuestos: **<10 MB RAM, <1% CPU idle, <100ms arranque, <5 MB binario**.

## 1. Por qué sin BD

* Sesión efímera: perfil + roster del lobby actual + historial de 5/20 partidas + detalle de la última. No hay histórico a largo plazo que justifique SQLite.
* Arranque <100ms: sin migrar, sin file lock.
* Si el usuario quiere historial persistente tras cerrar: export `vtracker export --hoy --csv` (generación bajo demanda).

## 2. Gestión de RAM — caché en memoria (sin disco)

### 2.1 Capas

```
PD/GLZ → L1 (moka::future::Cache, en memoria, TTL) → Raw Data → Analytics → Derived → AppState → TUI
         └─ No L2 en disco en v1 (se añade solo si el usuario pide historial offline)
```

* **Sin `RwLock` global:** cada caché interna ya es concurrente (moka es lock-free; cache-rs `concurrent` usa Mutex pero `get` requiere &mut lógico — moka evita esto).
* **Claves:** `puuid:shard:queue:matchId` + `version`.

### 2.2 Política por tipo de dato (TTL vigente, ajustable en `config.toml: [cache]`)

| Tipo | TTL | Capacidad | Evicción | Razón |
|---|---|---|---|---|
| Fases (`Lobby/PreGame/InMatch`) | — (evento) | 1 | Sobrescribe | Volátil por WebSocket |
| Roster live (10 jugadores en lobby) | 10s | 1 | Sobrescribe | Solo durante lobby |
| Perfil propio (MMR/history 20) | 60s (idle) | 1-20 | `moka` TTL | Cambia lento |
| Match details (rondas) | 300s | 20 | LRU | Post-partida, relectura rara |
| Nombres puuid→Riot ID | 900s | 100 | LRU | Muy estable |
| Content (agentes/mapas) | 86400s | 1 catálogo | FIFO | Manual post-parche |

Todas con `time_to_idle` 60s para sesiones: si no tocas el historial en 1 min, se evicta.

### 2.3 Implementación Rust vigente (2026)

**Opción A — `moka` (recomendada, producción):**
```rust
use moka::future::Cache;
use std::time::Duration;

Cache::builder()
    .max_capacity(100)
    .time_to_live(Duration::from_secs(300))
    .time_to_idle(Duration::from_secs(60))
    .eviction_listener(|k, v, cause| tracing::debug!(%k, ?cause, "evicted"))
    .build()
```
* `get_with(key, async { fetch().await })` evita stampede (un solo fetch por key).
* Soporte `future` para Tokio, `sync::Cache` y `sync::SegmentedCache` para alta contención.

**Opción B — `cache-rs` (si quieres algoritmo intercambiable):**
* 5 algoritmos tras misma API: `LruCache` (~887ns get), `SlruCache`, `LfuCache`, `LfudaCache`, `GdsfCache` (size-aware, 50-90 puntos más de hit cuando objetos varían). Benchmark 2026: `cache-rs` >47M req muestran que elección de algoritmo puede ir de 30% a 90% hit.
* Útil si luego añades GDSF para `match-details` de tamaño variable. `Concurrent` usa Mutex porque `get` muta metadata (LRU mueve al frente).

**Opción C — `CacheKit` (2026-01):** FIFO/LRU/LRU-K/2Q/LFU + tiered (RAM→disco), >9M ops/s, 0.81-1.02µs latencia, métricas Prometheus. Para cuando quieras L1/L2 sin reescribir: tiered L1 en RAM secundario a disco.

**No hacer:** `HashMap<RwLock>` sin evicción → leak a 80% RAM a las 3am. Siempre TTL + `max_capacity`.

## 3. Mínimo uso de CPU — TUI ultra-ligera

**Presupuesto Ratatui real (issue #1338, 2024-2026):** sin optimización, 7% single-core a 60 FPS continuos. Con dirty-flag → ~1% idle, ~0% en contenido estático. `Symbol width caching` (PR #1339) ya dio 17%.

### 3.1 Patrones críticos (validados en Zellij + native-cli-ai)

* **Dirty flag (crítico):**
```rust
if app.is_dirty() { terminal.draw(|f| app.render(f))?; app.clear_dirty(); }
```
No renderizar a 60 FPS si nada cambió. VTracker solo marca dirty en: WebSocket `phase`, llegada de GLZ/PD, tick de 1s para reloj, input.

* **Widgets pre-construidos fuera de `draw`:** cachear `ListItem<'static>` y solo rebuild si data cambia.
* **Canales acotados:** `tokio::sync::mpsc::channel(100)` con backpressure (Zellij: bounded previene bloat de 50 msg). Nunca `unbounded_channel`.
* **Futures pequeños:** `Arc<Mutex<AppState>>` en `spawn`, no clonar estado grande. `async-trait` alloca Box por llamada — solo en bordes, no en hot path.
* **Polling mínimo:** WebSocket event-driven; fallback polling local cada 2s con jitter. Sin polling a GLZ/PD salvo refresh `r`.
* **Batch de eventos SSE:** coalescar `LogWatcher` (`round_ended` + `player_died` en mismo tick) en un solo `AppState` update.

### 3.2 Tokio

* `JoinSet` para tracking, `select! biased;` para priorizar input sobre timer.
* Overhead runtime ~12-18% bajo carga — medible con `cargo flamegraph`.
* Evitar `large futures >1KB` (copia en stack al spawn).

### 3.3 Medición antes de optimizar (orden)

```bash
cargo flamegraph --root
/usr/bin/time -v target/release/vtracker
ls -lh target/release/vtracker
cargo bench  # cache-rs/moka
```
Metas VTracker v1 (tomadas de sysmon/kite/btop, Rust 2026): `idle CPU <1%`, `active typing <5%`, `mem 5-10 MB`, `cold start <100ms`, `history 60 puntos` (sysmon). Referencia Zellij: 50%→1% con dirty-flag.

## 4. Tamaño del binario (.exe para amigos, sin cargo)

Sin BD y sin Overwolf, el binario debe ser pequeño para compartir por Discord.

**Perfil release mínimo (verificado 2026-04 `rustfaq: reduce binary size`):**
```toml
[profile.release]
opt-level = "z"      # tamaño > velocidad; z agresivo, s más seguro
lto = true
codegen-units = 1
strip = true
panic = "abort"
```
*Ganancias:* `z` 25-30%, `lto` 5-10%, `strip` 3-8%, `abort` 2-5%, combinado 40-50% (4.2MB→1.3MB en ejemplo). Tradeoff: compila más lento — solo en release/CI.

**Extras no necesarios para VTracker:** `no_std` (solo embebidos), UPX (30-50% más pero delay al arrancar). Mantener `std` (Tokio/Ratatui lo necesitan).

**Regla:** medir `wc -c target/release/vtracker.exe` en cada release; objetivo <5 MB.

## 5. Flujo durante la partida (sin BD, concreto)

```
Usuario abre VALORANT
  → lockfile existe → connect() auto (val-local-api)
  → WebSocket event "pregame" → GLZ pre-game/match (roster en 10s TTL)
  → PD history/MMR/RoundResults (TTL 60-300s) → TUI con perfil+rosters+timeline
  → LogWatcher SSE "round_ended" → muta AppState round N → dirty=true → draw
  → Sin DB: al cerrar VTracker todo se libera. Siguiente arranque: re-fetch (o usa últimos datos en memoria si sigue abierto, degradación elegante).
```

**Cache stampede:** usar `moka::cache.get_with` + early refresh probabilístico (20% TTL restante → 25% chance, 5% → 100%) para que un solo request refresque antes de que todos fallen a la vez.

**Conclusión exhaustiva:** con `moka` TTL+TTI, dirty-flag rendering, canales acotados, polling 0 y release `z/lto/strip`, VTracker corre una sesión completa (detección→roster→post-match rondas) en <10 MB, <1% idle, arranque instantáneo, sin disco — exactamente lo pedido.

---
*Teams: CC0 1, Tiempo total de documentación: 2026-08-25*
