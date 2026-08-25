# DESIGN-UI — Especificación de interfaz (TUI)

> Estado: en refinamiento (pre-código). Define las vistas, layouts adaptativos y navegación. Principios base en `Arquitectura-inicial.md:12` (Elm/TEA, `AppState` solo presentación). Specs relacionados: `SPEC-ROUNDS.md`, `DECISIONS.md`.

## 1. Vistas

| Vista | Propósito | Estado |
|---|---|---|
| **LIVE MATCH** | Partida en curso: rosters + timeline de rondas + sesión | Este spec |
| **PostMatch** | Resumen completo al terminar: timeline íntegro + stats finales | Este spec |
| **Dashboard** | Perfil propio + estado del cliente + resumen | Futuro |
| **History** | Historial propio con filtros | Futuro |
| **Settings** | Configuración (intervalo, autostart, perfil, TTL) | Futuro |

## 2. Vista LIVE MATCH — composición

```
╭─────────────────────────────┬─────────────────────────────╮
│  LIVE MAP · Ascent · Comp   │  RONDAS (tú: yo#LAS)        │
│                             │      K                      │
│  TU EQUIPO                  │      K  K                   │
│  yo#LAS      ASC 2  1.21 …  │   K  K  K  K     K          │
│  pepe#LAS    IMM 1  1.34 …  │  ───────────────────────    │
│  juan#LAS    ASC 3  1.08 …  │   R1 R2 R3 R4 R5 R6 R7 …    │
│  gato#LAS    DIA 3  0.97 …  │  ───────────────────────    │
│  xXkiller    ASC 1  1.18 …  │   D  D         D            │
│                             │  [← →] bloques · [t] ocultar│
│  ENEMIGOS                   │                             │
│  Enemy1#LAS  IMM 2  1.41 …  │                             │
│  Enemy2#LAS  ASC 3  1.15 …  │                             │
╰─────────────────────────────┴─────────────────────────────╯
  sesión: 3 partidas (2W-1L) · K/D sesión 1.14        [q] salir
```

### Componentes

**Header** — mapa y modo en vivo (`Current Game Match` → `MapID`/`ModeID`). En modos sin rondas (ADR-004) el timeline no existe y el layout es solo rosters.

**Roster (x2: equipo/enemigos)** — columnas: `Player | Rank | K/D | Last 5 | TRACKER`.
* `K/D` = **histórico pre-partida** (lo que la fuente da en vivo; el K/D de la partida actual solo existe post-partida — honestidad de datos).
* `Last 5` = W/L de últimas 5 partidas.
* `TRACKER` = enlace a perfil web externo (opcional, configurable).
* Navegación por flechas para seleccionar jugador (ver §5).

**Timeline de rondas** — especificación completa en `SPEC-ROUNDS.md:5`. Solo en modos con ronda.

**Barra de sesión** — acumulado de partidas terminadas hoy (W/L, kills, muertes, K/D sesión) desde `match-history` + caché. Requisito explícito del usuario.

## 3. Layouts adaptativos (sin resolución fija)

| Terminal | Layout |
|---|---|
| Ancha (≥ ~140 cols) | 2 columnas: rosters izquierda, timeline derecha (mockup §2) |
| Mediana (~70-140) | Apilado vertical: header → equipo → enemigos → timeline |
| Angosta (< ~70) | Rosters compactos (solo `Player | Rank`), timeline **colapsado** a 1 línea: `R7 · 6K 4D · [enter] expandir` |
| Muy angosta | Timeline oculto; disponible íntegro en PostMatch |

Reglas:
* El layout se recalcula **cada frame** con el área que Ratatui entrega — redimensionar la terminal re-acomoda al instante.
* El timeline usa bloques con wrap horizontal (`SPEC-ROUNDS.md:5`) y paginación vertical.
* Leyenda compacta (`K=kill D=death ★=ace`) que desaparece si no hay espacio.

## 4. Vista PostMatch

Al terminar la partida (detectado por WebSocket local / fin de core-game):
1. Polling de `match-details` → datos completos garantizados.
2. Timeline a espacio completo (todos los bloques navegables), seleccionable por jugador.
3. Stats finales: scoreboard oficial (ADR-007) — K/D/A, ACS si la fuente lo entrega, HS% si hay campos.
4. Resultado de partida + score de rondas.

## 5. Navegación (borrador)

| Tecla | Acción |
|---|---|
| `↑/↓` o `j/k` | Mover selección en roster (aliados; enemigos si disponible) |
| `Enter` | Expandir timeline colapsado / fijar jugador seleccionado en timeline |
| `←/→` | Paginar bloques de rondas |
| `t` | Ocultar/mostrar timeline (libera espacio para rosters) |
| `r` | Refresh controlado (re-snapshot) |
| `q` | Salir |

Los atajos finales se confirman al implementar P5; configurables después del MVP.

## 6. Estados de la vista

| Estado | Muestra |
|---|---|
| Cargando | Spinner/`…` por sección independiente (rosters y timeline cargan por separado) |
| Sin datos de ronda aún | Timeline vacío con `R1 R2…` atenuadas |
| Provider falló | Último dato conocido + aviso recuperable en la barra inferior (nunca pantalla vacía) |
| Modo sin rondas | Sin sección de rondas (ADR-004) |

## 7. Preguntas abiertas

1. ¿Colores configurables (tema) desde Settings o fijos en v1?
2. ¿El roster de enemigos muestra las mismas columnas que aliados o reducidas?
3. ¿Auto-scroll del timeline al bloque actual durante partida en vivo?
4. ¿La vista live se actualiza con polling adaptativo o puramente por eventos WebSocket?
