# DESIGN-UI — Especificación de interfaz (TUI)

> **Prioridad de producto (2026-08-28, ADR-011):** el roster de aliados y enemigos (diez jugadores en 5v5), con agentes, rangos y estadísticas históricas disponibles y permitidos, es la función principal; no es opcional. La implementación real ya conecta el roster y sus cinco Ranked históricas, pero todavía requiere revalidación en partidas reales tras cada corrección. La demo visual en Rust no cumple por sí sola este requisito. Representar identidades ocultas y datos ausentes sin eludir restricciones.

> **Actualización 2026-08-30:** la referencia visual aprobada partió de [`mockups/README.md`](mockups/README.md) y evolucionó en `src/tui/view.rs`. En la implementación actual: `1–6` cambia vista, `Tab` mueve foco, `t` cambia tema y `Esc` vuelve; Aliados → Tus rondas → Enemigos también en ancho grande. La sexta vista Logs monitoriza únicamente VTRACKER. La demo es ficticia y aislada; no implica integración del roster o rondas en vivo.

> **Bocetos históricos:** los diagramas y atajos anteriores que aparecen más abajo son exploratorios y quedan subordinados a la maqueta aprobada y ADR-012. Los requisitos de datos y privacidad mantienen su vigencia.

> Estado: implementado parcialmente y en refinamiento con pruebas reales. Define las vistas, layouts adaptativos y navegación. Principios base en `Arquitectura-inicial.md:12` (Elm/TEA, `AppState` solo presentación). Specs relacionados: `SPEC-ROUNDS.md`, `DECISIONS.md`.

## 1. Vistas

**Criterio transversal confirmado:** todas las vistas deben parecer y comportarse como una terminal, no como una web con tipografía monoespaciada. Usar filas y columnas de caracteres, bordes textuales, una línea por jugador, atajos visibles y densidad adecuada para ventanas no maximizadas. La maqueta actual explora 72 y 38 columnas. Los controles HTML son solo un mecanismo de demostración: se representan como texto y deberán implementarse con eventos de terminal en Rust.

**Color e imágenes (revisión 2026-08-28):** la maqueta aplica una paleta semántica con temas Sistema/Noche/Claro/Sin color y mantiene rótulos en texto. Imágenes opcionales en Perfil/detalle son una propuesta pendiente; no forman parte del requisito base ni del render actual. Compatibilidad, referencias y limitaciones de tamaño en [mockups/REFERENCES.md](mockups/REFERENCES.md).

**Formato compacto vigente:** el usuario sustituyó las letras apiladas por cantidades (`1K`, `4K`, `0D`, `2D`). El timeline entre equipos ocupa tres líneas: kills, número de ronda y muertes. Sin tarjetas, separadores extra ni grandes resúmenes. La ronda actual lleva `*` y `—K / —D` hasta disponer de datos confirmados; cero significa cero confirmado. Priorizar el espacio de una terminal no maximizada; las partidas largas deberán paginar por bloques según el ancho.

**Acceso a Tracker.gg:** columna textual `TRK` con `[↗]` y atajo `g` para el jugador real seleccionado; también desde su detalle. Abre el perfil externo únicamente por acción del usuario y cuando exista un Riot ID completo y visible. El dominio es fijo y el Riot ID se codifica como segmento; `Jugador N` y datos ausentes no generan enlace. En la demo continúa deshabilitado por usar nombres ficticios. `t` conserva el cambio de tema.

**Distribución solicitada el 2026-08-28 (prevalece sobre los mockups anteriores):** en la vista de partida, el orden es **Aliados → Tus rondas → Enemigos**. Mostrar kills y muertes propias por ronda completada y su acumulado; identificar la ronda en curso como pendiente hasta contar con un snapshot confirmado. El timeline no debe quedar relegado a postpartida. La maqueta HTML refleja este orden; obtener los datos durante una partida real sigue pendiente de validación. Sin datos confirmados, mostrar indisponibilidad, no estadísticas inventadas. Esta decisión de presentación no habilita por sí sola una nueva fuente de datos.

| Vista | Propósito | Estado |
|---|---|---|
| **LIVE MATCH** | Partida en curso: rosters + timeline de rondas + sesión | Este spec |
| **PostMatch** | Resumen completo al terminar: timeline íntegro + stats finales | Este spec |
| **Resumen** | Mi perfil + estado de partida + rendimiento reciente | Implementado; salud detallada permanece exclusivamente en `doctor` |
| **Agent Select** | Compañeros disponibles + rango, nivel, premade y cinco Ranked | Implementado; rivales aparecen al terminar la selección |
| **History** | Resultado, score, mapa, agente, K/D/A, modo y antigüedad | Implementado; filtros pendientes |
| **Settings** | Intervalo, registro y tema con lenguaje de usuario | Base implementada; autostart/perfil/TTL pendientes |

La TUI Rust acepta mouse opcional para pestañas, selección de historial/ajustes y rueda. No depende de él: todos los flujos conservan atajos de teclado y la captura se deshabilita al salir.

El binario sin argumentos abre la TUI. Durante los primeros tres segundos muestra una portada enmarcada con el logotipo ASCII `VTracker`, el texto provisional `blablabla` y `https://github.com/maaurissio/vtracker`; el trabajo local comienza en segundo plano y el logo tiene una variante compacta para terminales angostas. Si al terminar el splash aún falta el perfil, la misma portada comunica la búsqueda de Riot Client o la carga de perfil e historial. En cuanto el perfil queda disponible —o falla de forma recuperable— aparece Resumen. Fuera de partida, Resumen prioriza rango, nivel, barra de RR y rendimiento reciente antes del estado de partida. Las fuentes y el diagnóstico interno no aparecen en estas vistas; `doctor` y `watch` conservan esa información técnica.

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
* `TRACKER` = acceso al perfil web externo mediante `g`; deshabilitado si falta una identidad verificada o está oculta fuera de la propia party. En Windows se abre mediante la asociación HTTPS nativa del sistema.
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
3. Stats finales: scoreboard oficial (ADR-007) — K/D/A, ACS, ADR y HS% cuando la fuente entrega rondas, daño e impactos.
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
| Inicio sin observación | Portada centrada: búsqueda de Riot Client; todavía no muestra pestañas ni datos vacíos |
| Perfil inicial | Después del splash de tres segundos, la portada informa que carga perfil/rango; el historial no bloquea la entrada a Resumen |
| Carga de partida | Gauge modal alimentado por etapas reales del worker: sesión local, partida/resultado detectado y preparación de roster o postpartida |
| Carga posterior | Mensaje por sección independiente; roster e historial cargan por separado |
| Sin datos de ronda aún | Timeline vacío con `R1 R2…` atenuadas |
| Provider falló | Último dato conocido + aviso recuperable en la barra inferior (nunca pantalla vacía) |
| Modo sin rondas | Sin sección de rondas (ADR-004) |

### Historial propio y controles persistentes

La vista Historial carga hasta 20 partidas Ranked propias. En terminales amplias, un gráfico de barras muestra el RR ganado o perdido en cada partida, desde la más antigua hasta la reciente, y su encabezado indica el neto; la tabla conserva selección, detalle y RR por partida. En terminales pequeñas se prioriza la tabla. El último snapshot normalizado permanece disponible con VALORANT cerrado y se identifica como dato guardado con su antigüedad.

La navegación superior usa cinco botones visualmente separados por espacio y fondo, sin contornos laterales (`1: Resumen` … `5: Ajustes`). El pie vive dentro del borde inferior y presenta acciones contextuales como `[tecla→acción]`; en anchos compactos conserva solo los controles esenciales. La navegación vertical usa siempre `↑/↓`, y el contador de desplazamiento se alinea a la derecha sin repetir el icono. El marco principal se titula `VTRACKER`; `DEMO` solo se añade cuando corresponde para distinguir datos ficticios.

## 7. Preguntas abiertas

1. ¿Colores configurables (tema) desde Settings o fijos en v1?
2. ¿El roster de enemigos muestra las mismas columnas que aliados o reducidas?
3. ¿Auto-scroll del timeline al bloque actual durante partida en vivo?
4. ¿La vista live se actualiza con polling adaptativo o puramente por eventos WebSocket?
