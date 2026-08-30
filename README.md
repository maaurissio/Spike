# VTracker

VTracker es una aplicación de terminal para consultar tu partida y tus estadísticas de VALORANT desde una pantalla compacta, rápida y fácil de navegar.

La aplicación funciona en modo de solo lectura. No controla el juego, no simula acciones y no accede a la memoria de VALORANT.

> VTracker está en desarrollo. El nombre y algunos elementos visuales todavía pueden cambiar antes de la primera versión estable.

## ¿Qué muestra?

### Durante una partida

- Aliados y enemigos de la partida.
- Agente, rango y nivel cuando el dato está disponible.
- K/D, porcentaje de headshots, KAST, win rate y resultados de las últimas cinco partidas Ranked.
- Premades mediante un `•` de color antes del nombre; los integrantes del mismo grupo comparten color.
- Acceso al perfil de Tracker.gg cuando el jugador tiene un Riot ID público disponible.

### En selección de agente

- Los compañeros que VALORANT hace visibles en esa fase.
- Agente seleccionado, rango, nivel, premades y estadísticas Ranked disponibles.
- Los rivales aparecen después, cuando comienza la partida.

### Fuera de partida

- Tu nivel, rango competitivo y progreso de RR.
- Resumen de tus últimas cinco Ranked.
- Historial con resultado, marcador, mapa, agente, K/D/A, HS%, ACS y ADR.
- Detalle de la última partida y rendimiento por ronda cuando VALORANT entrega esos datos.

Los nombres ocultos permanecen ocultos y se muestran como `Jugador 1`, `Jugador 2`, etc. Si un dato no está disponible, VTracker muestra `—` en vez de inventarlo.

## Cómo iniciar VTracker

VTracker requiere Windows, VALORANT instalado y una terminal moderna como Windows Terminal.

Si compilas el proyecto desde el código fuente:

```powershell
cargo build --release
.\target\release\vtracker.exe
```

Para abrir una demostración con datos ficticios, sin necesidad de entrar a VALORANT:

```powershell
.\target\release\vtracker.exe dashboard --demo
```

## Controles

| Tecla | Acción |
|---|---|
| `1`–`5` | Cambiar entre Resumen, Partida, Mi perfil, Historial y Ajustes |
| `Tab` / `Shift+Tab` | Cambiar el foco de navegación |
| `↑` / `↓` | Seleccionar jugadores, partidas u opciones |
| `Enter` | Abrir el elemento seleccionado |
| `g` | Abrir Tracker.gg para el jugador seleccionado |
| `r` | Actualizar los datos |
| `Esc` | Cerrar un detalle o volver |
| `q` | Salir de VTracker |

También puedes usar el mouse en pestañas, filas y opciones compatibles.

## Estado actual

VTracker ya permite consultar el roster de la partida, estadísticas Ranked, perfil propio, historial y resumen postpartida. La detección y disponibilidad de algunos campos depende de la fase y de la información que entregue el cliente.

Las kills y muertes de cada ronda se obtienen de los resultados finales. La aplicación todavía no muestra un timeline completo de rondas mientras la partida está en curso.

## Privacidad y seguridad

- Solo realiza consultas de lectura.
- No lee memoria del juego.
- No inyecta código ni automatiza controles.
- No guarda credenciales ni identificadores internos en la interfaz.
- Respeta los nombres ocultos y los datos no disponibles.

VTracker no está afiliado con Riot Games. VALORANT y Riot Games son marcas registradas de Riot Games, Inc.
