# VTracker — repositorio de desarrollo

VTracker es una aplicación de terminal para consultar contexto de partida y estadísticas de VALORANT mediante una interfaz construida con Rust, Ratatui y Crossterm.

> [!WARNING]
> Este es el repositorio de desarrollo. Puede contener funciones experimentales, cambios incompatibles y código que todavía necesita validación en partidas reales. No representa una versión estable ni ofrece binarios soportados para usuarios finales.

El futuro repositorio público de distribución contendrá únicamente versiones revisadas, instaladores, notas de lanzamiento y artefactos estables. Hasta entonces, `master` en este repositorio significa “hito integrado”, no “release oficial”.

## Alcance del producto

El requisito principal es mostrar el roster de aliados y enemigos de la partida —diez jugadores en modos 5v5— con los rangos y estadísticas que estén disponibles y cuyo uso esté permitido. El perfil propio, el historial y la postpartida complementan ese objetivo.

La implementación actual incluye:

- dashboard interactivo con Resumen, Partida, Mi perfil, Historial, Ajustes y Logs;
- contexto propio, perfil competitivo y progreso de RR;
- hasta 20 Ranked propias, detalle postpartida y gráfico de variación de RR;
- prototipo técnico del roster durante selección de agente y partida;
- estadísticas históricas Ranked, premades e identidades visibles cuando la fuente las entrega;
- monitor local de CPU, memoria, uptime, picos y eventos sanitizados de VTracker;
- perfil opcional de Windows Terminal con Gruvbox y Fira Mono;
- comandos de diagnóstico y demostración sin una sesión real de VALORANT.

La validación completa del roster en partidas reales sigue pendiente. Las identidades ocultas se conservan como `Jugador N`; los datos ausentes se muestran como `—` y nunca se inventan. El prototipo de consultas a terceros no equivale a autorización para distribuir esa función: antes de una versión estable se deben resolver registro, términos, consentimiento y revisión aplicables.

VTracker funciona en modo de solo lectura. No lee memoria del juego, no inyecta código, no automatiza controles y no guarda credenciales de sesión.

## Estado del repositorio

| Rama | Uso |
|---|---|
| `dev` | Integración diaria del desarrollo activo |
| `master` | Hitos internos que compilan y pasan las verificaciones del proyecto |
| `codex/*` o ramas de función | Trabajo aislado antes de integrarlo en `dev` |

No se deben crear releases ni adjuntar ejecutables de distribución en este repositorio. El flujo previsto para una versión pública es promover un commit revisado desde aquí hacia un repositorio estable independiente.

El alcance pendiente y la evidencia de validación se mantienen en [TASKS.md](TASKS.md). Las decisiones técnicas y de producto están en [docs/DECISIONS.md](docs/DECISIONS.md).

## Requisitos de desarrollo

- Windows 10 u 11.
- Rust estable compatible con la edición 2024 (Rust 1.85 o posterior).
- Cargo.
- VALORANT y Riot Client para probar fuentes locales reales.
- Windows Terminal es opcional, pero recomendado para la experiencia visual.

No se requiere una API key para ejecutar el dashboard local. `RIOT_API_KEY` queda reservada para integraciones oficiales futuras y nunca debe incluirse en el repositorio.

## Compilar y ejecutar

```powershell
git clone https://github.com/maaurissio/vtracker.git
Set-Location vtracker
cargo build
cargo run
```

Compilación optimizada:

```powershell
cargo build --release
.\target\release\vtracker.exe
```

Demo con datos ficticios, sin abrir VALORANT:

```powershell
cargo run -- dashboard --demo
```

## Verificaciones antes de integrar

```powershell
cargo fmt --all -- --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

Una función conectada a datos reales no se considera terminada solo porque compile o funcione con fixtures. Debe distinguirse entre modelo, demo, integración y validación real.

## Comandos disponibles

```text
vtracker
vtracker dashboard [--demo]
vtracker watch [--once] [--interval SEGUNDOS]
vtracker player
vtracker history [--limit 1..20]
vtracker stats [--limit 1..5]
vtracker doctor
vtracker config show|validate
vtracker config edit [--interval SEGUNDOS] [--log-transitions true|false]
vtracker terminal install|status|launch|uninstall
```

Ejecuta `cargo run -- --help` para consultar la ayuda generada por la versión actual.

## Perfil de terminal opcional

VTracker puede registrar un perfil independiente de Windows Terminal con Gruvbox y Fira Mono:

```powershell
cargo run -- terminal install
cargo run -- terminal status
cargo run -- terminal launch
cargo run -- terminal uninstall
```

La instalación copia el ejecutable a `%LOCALAPPDATA%\VTracker`, crea una copia previa de la configuración de Windows Terminal y registra un GUID propio. No modifica los demás perfiles. Esto es una herramienta para desarrollo y no sustituye un instalador de la futura versión estable.

## Configuración y datos locales

- Configuración: `%APPDATA%\vtracker\config.toml`
- Historial normalizado: `%APPDATA%\vtracker\history-cache.json`
- Registro opcional de transiciones: `%APPDATA%\vtracker\watch.log`
- Sesión de Riot: credenciales efímeras leídas del lockfile local y mantenidas solo en memoria

Usa [config.example.toml](config.example.toml) como referencia. Las variables opcionales están documentadas en [.env.example](.env.example); el archivo `.env` y los logs están ignorados por Git.

## Arquitectura resumida

```text
Riot/VALORANT local
        │
        ▼
providers ──► models ──► analytics
    │                       │
    └────────► worker ◄─────┘
                  │
                  ▼
             Ratatui TUI
```

- `src/providers/`: procesos, lockfile, sesión local, perfil, historial, roster y postpartida.
- `src/models/`: datos normalizados sin identificadores internos para la interfaz.
- `src/analytics/`: métricas derivadas solo cuando existen datos suficientes.
- `src/tui/`: estado, worker, navegación, temas, métricas y renderizado.
- `src/diagnostics/`: comprobaciones técnicas sin revelar secretos.
- `src/cache/`: caché acotada en memoria; el historial propio usa además un snapshot sanitizado.

La TUI no realiza I/O remoto durante el renderizado: el worker obtiene y normaliza los datos, y la vista consume estados preparados.

## Documentación técnica

- [Guía de contribución](CONTRIBUTING.md)
- [Arquitectura inicial](Arquitectura-inicial.md)
- [Registro de decisiones](docs/DECISIONS.md)
- [Política del roster](docs/ROSTER-POLICY.md)
- [Contrato de la API local](docs/SPEC-LOCAL-API.md)
- [Diseño de la interfaz](docs/DESIGN-UI.md)
- [Apariencia de terminal](docs/TERMINAL-APPEARANCE.md)
- [Rendimiento](docs/PERFORMANCE.md)
- [Trabajo pendiente](TASKS.md)

## Seguridad y cumplimiento

No publiques lockfiles, tokens, PUUID, MatchID, Riot IDs privados, payloads completos ni archivos locales de configuración. Usa fixtures fabricados y sanitizados en pruebas y documentación.

VTracker no está afiliado con Riot Games. VALORANT y Riot Games son marcas registradas de Riot Games, Inc.

## Licencia

La licencia prevista es MIT. El archivo `LICENSE` se agregará cuando se confirme el nombre exacto del titular, antes de publicar el repositorio estable.
