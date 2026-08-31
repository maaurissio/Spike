# Contribuir a VTracker

Este repositorio contiene el desarrollo activo de VTracker. Las contribuciones deben preservar el alcance principal, la privacidad de los jugadores y la separación entre prototipo técnico y función validada.

## Antes de comenzar

1. Lee [TASKS.md](TASKS.md) para conocer prioridades y trabajo pendiente.
2. Revisa [docs/DECISIONS.md](docs/DECISIONS.md), especialmente ADR-011 y las decisiones que la sustituyen o complementan.
3. Consulta [docs/ROSTER-POLICY.md](docs/ROSTER-POLICY.md) si el cambio afecta jugadores, identidades o estadísticas de terceros.
4. No amplíes el acceso a datos ni agregues un proveedor externo sin documentar fuente, términos, consentimiento y degradación segura.

## Flujo de ramas

Parte desde `dev` y crea una rama pequeña para cada cambio:

```powershell
git switch dev
git pull --ff-only origin dev
git switch -c codex/descripcion-breve
```

Integra primero en `dev`. `master` se reserva para hitos internos verificados; no es la rama de distribución pública.

## Criterios técnicos

- Mantén el renderizado de Ratatui libre de red y operaciones bloqueantes.
- Normaliza los datos antes de entregarlos a la TUI.
- No expongas PUUID, MatchID, tokens, puertos o contraseñas en la interfaz, logs o errores.
- Representa los datos ausentes explícitamente; no uses valores inventados como fallback.
- Respeta nombres ocultos y limita cualquier excepción a información que el propio cliente ya revela legítimamente al usuario.
- Evita dependencias nuevas si la biblioteca estándar o una dependencia existente cubre el caso.
- Añade pruebas para parsers, normalización, privacidad y estados de error.
- Usa datos ficticios o sanitizados en fixtures.

## Verificación local

Antes de solicitar integración:

```powershell
cargo fmt --all -- --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

Si el cambio toca la TUI, comprueba también al menos un tamaño reducido y uno amplio mediante `cargo run -- dashboard --demo`. Si toca una fuente real, registra en `TASKS.md` qué se verificó realmente y qué continúa pendiente.

## Commits y documentación

Usa commits pequeños con una intención clara, por ejemplo:

```text
feat: add session peak metrics
fix: preserve hidden player identity
docs: clarify roster validation state
test: cover malformed lockfile input
```

Actualiza la documentación cuando cambien comandos, configuración, persistencia, arquitectura, comportamiento visible o decisiones de producto. Una decisión con consecuencias futuras debe registrarse como ADR en `docs/DECISIONS.md`.

## Qué no incluir

- Credenciales, archivos `.env`, lockfiles o configuración personal.
- Datos reales que permitan identificar jugadores o partidas.
- Ejecutables compilados, contenido de `target/` o releases de usuario final.
- Código que lea memoria, inyecte procesos, capture input o automatice VALORANT.

La licencia para contribuciones quedará formalizada antes de abrir el futuro repositorio estable. Hasta entonces, coordina contribuciones externas con el propietario del proyecto.
