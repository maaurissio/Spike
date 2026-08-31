![Spike.](assets/screenshots/demo-inicio.png)

---------

Spike es una aplicación de terminal para VALORANT creada con Rust, Ratatui y Crossterm. Reúne el contexto disponible de la partida, progreso competitivo, historial Ranked y métricas locales en una interfaz rápida y navegable con teclado, creada para consumir el menor rendimiento posible.

> [!NOTE]
> Spike está en desarrollo constante, sin versión estable oficial aún.

---------

# Vista previa

> [!NOTE]
> Las siguientes capturas fueron generadas con `spike dashboard --demo` y usan el tema [Gruvbox](https://github.com/morhetz/gruvbox).

<table>
  <tr>
    <td><img src="assets/screenshots/demo-1.png" width="400"/></td>
    <td><img src="assets/screenshots/demo-2.png" width="400"/></td>
  </tr>
  <tr>
    <td><img src="assets/screenshots/demo-3.png" width="400"/></td>
    <td><img src="assets/screenshots/demo-4.png" width="400"/></td>
  </tr>
  <tr>
    <td><img src="assets/screenshots/demo-5.png" width="400"/></td>
    <td><img src="assets/screenshots/demo-partida.png" width="400"/></td>
  </tr>
</table>

La demo funciona sin abrir VALORANT y permite recorrer la interfaz con datos ficticios:

```powershell
cargo run -- dashboard --demo
```

---------

## Qué hace

- Muestra un resumen de perfil, rango y progreso de RR.
- Consulta hasta 20 partidas Ranked propias y representa las ganancias y pérdidas de RR con un gráfico de barras.
- Presenta el detalle postpartida disponible, incluida la información propia por ronda cuando la fuente la entrega.
- Habilita la vista contextual **Partida** durante selección de agente, partida o postpartida.
- Muestra el roster disponible de aliados y enemigos, sus rangos y estadísticas cuando la fuente lo permite.
- Incluye **Logs** con CPU, RAM, uptime, picos de consumo y actividad sanitizada de la sesión.
- Usa un perfil dedicado de Windows Terminal con tema Gruvbox y Fira Mono.

> [!IMPORTANT]
> El objetivo principal de Spike es presentar el roster de la partida actual —aliados y enemigos en modos 5v5— junto con los datos disponibles y permitidos. El perfil, historial y postpartida lo complementan.

---------

## Instalación

Por ahora Spike se ejecuta desde el código fuente en Windows.

### Requisitos

- Windows 10 u 11.
- [Rust](https://www.rust-lang.org/tools/install) estable con Cargo.
- VALORANT y Riot Client para consultar datos locales reales.
- [Windows Terminal](https://apps.microsoft.com/detail/9n0dx20hk701) instalado.
- [Fira Mono](https://fonts.google.com/specimen/Fira+Mono) instalada para el usuario actual.

> [!IMPORTANT]
> **Windows Terminal y Fira Mono son requisitos obligatorios de Spike.** La interfaz está diseñada, probada y distribuida únicamente con ese perfil. No se admite ni documenta la ejecución en la consola clásica de Windows ni con otra fuente.

```powershell
git clone https://github.com/maaurissio/Spike.git
Set-Location Spike
cargo build --release
.\target\release\spike.exe terminal install
.\target\release\spike.exe terminal launch
```

`terminal install` verifica ambos requisitos, instala el perfil aislado **SPIKE**, aplica Gruvbox y copia el ejecutable a `%LOCALAPPDATA%\Spike`. Si falta Windows Terminal o Fira Mono, se detiene con un error claro: instálalos antes de continuar.

Para comprobar que el entorno quedó correcto:

```powershell
.\target\release\spike.exe terminal status
```

> [!WARNING]
> Aún no hay instalador ni binarios oficiales para usuarios finales. Compila el proyecto solo si quieres probar la versión de desarrollo.

---------

## Uso

| Comando | Descripción |
|---|---|
| `spike` | Abre el dashboard. |
| `spike dashboard --demo` | Abre la demo local con datos ficticios. |
| `spike watch --once` | Comprueba el estado local una vez. |
| `spike player` | Muestra el perfil competitivo propio disponible. |
| `spike history --limit 1..20` | Consulta el historial Ranked propio. |
| `spike stats --limit 1..5` | Resume estadísticas propias recientes. |
| `spike doctor` | Ejecuta diagnósticos locales sin exponer secretos. |

Dentro de la interfaz, las teclas `1` a `5` cambian entre las vistas persistentes. Cuando hay una partida activa, aparece un acceso contextual a **Partida** en la parte superior derecha.

## Perfil obligatorio de Windows Terminal

La instalación crea un perfil separado llamado **SPIKE** sin modificar los demás perfiles. Es el único entorno compatible de ejecución:

```powershell
.\target\release\spike.exe terminal install
.\target\release\spike.exe terminal launch
```

Para comprobarlo o quitarlo:

```powershell
.\target\release\spike.exe terminal status
.\target\release\spike.exe terminal uninstall
```

Consulta el historial de cambios en [CHANGELOG.md](CHANGELOG.md).

## Privacidad, datos y límites

Spike funciona en modo de solo lectura: no lee memoria del juego, no inyecta código, no automatiza controles y no guarda credenciales de sesión.

> [!CAUTION]
> La disponibilidad técnica de datos locales no equivale a autorización para distribuirlos. Antes de un lanzamiento estable se deben validar términos, consentimiento, privacidad y las fuentes utilizadas. Las identidades ocultas y los datos no disponibles se respetan: Spike no intenta inferirlos ni inventarlos.

## Desarrollo

```powershell
cargo fmt --all -- --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

---------

## Licencia

Distribuido bajo la [licencia MIT](LICENSE).

Spike no está afiliado con Riot Games. VALORANT y Riot Games son marcas registradas de Riot Games, Inc.
