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
- Historial de hasta 20 Ranked con resultado, marcador, mapa, agente, K/D/A, HS%, ACS, ADR, cambio de RR y gráfico de barras por partida. El último snapshot seguro permanece visible con VALORANT cerrado.
- Detalle de la última partida y rendimiento por ronda cuando VALORANT entrega esos datos.
- Vista Logs estilo monitor con gráficos de CPU y memoria, valores actuales, promedios, picos máximos de la sesión, uptime y hasta 100 eventos sanitizados.

Los nombres ocultos permanecen como `Jugador 1`, `Jugador 2`, etc. La única excepción son integrantes de tu propia premade: si el cliente ya te permite verlos por compartir grupo, VTracker usa esa misma identidad. Si un dato no está disponible, muestra `—` en vez de inventarlo.

## Cómo iniciar VTracker

VTracker requiere Windows, VALORANT instalado y una terminal moderna como Windows Terminal.

La apariencia recomendada usa Gruvbox Dark y Fira Mono en un perfil independiente de Windows Terminal:

```text
vtracker terminal install
vtracker terminal status
vtracker terminal launch
vtracker terminal uninstall
```

La instalación copia VTracker a `%LOCALAPPDATA%\VTracker`, no cambia los demás perfiles y guarda una copia previa de la configuración antes de registrar su GUID. Si Windows Terminal estaba abierto, ciérralo normalmente una vez antes del primer `terminal launch`.

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

La ayuda contextual aparece integrada en el borde inferior con el formato `[tecla→acción]`. Todas las listas usan `↑/↓` para navegar.

## Estado actual

VTracker ya permite consultar el roster de la partida, estadísticas Ranked, perfil propio, historial y resumen postpartida. La detección y disponibilidad de algunos campos depende de la fase y de la información que entregue el cliente.

Las kills y muertes de cada ronda se obtienen de los resultados finales. La aplicación todavía no muestra un timeline completo de rondas mientras la partida está en curso.

## Privacidad y seguridad

- Solo realiza consultas de lectura.
- No lee memoria del juego.
- No inyecta código ni automatiza controles.
- No guarda credenciales ni identificadores internos en la interfaz.
- Respeta los nombres ocultos fuera de tu propia party y los datos no disponibles.

VTracker no está afiliado con Riot Games. VALORANT y Riot Games son marcas registradas de Riot Games, Inc.
