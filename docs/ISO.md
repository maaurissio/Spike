# VTracker — Sistema Integrado: ISO 9001 + ISO 14001 + ISO 27001

> **No es estúpido — es ambicioso y correcto.** Adoptar los tres sistemas desde el inicio te da credibilidad para competir por contratos y demuestra madurez, aunque la certificación formal llegue después. Este documento traduce cada norma a prácticas **proporcionales a un proyecto pequeño Rust/TUI** (no burocracia de gran empresa).

> **Nombre temporal:** `VTracker` se usa en todo el repo hasta el release. El SGC/SGA/SGSI referenciará el nombre final sin tener que reescribir procedimientos.

## Resumen ejecutivo

| Norma | Qué controla | Para qué sirve en VTracker |
|---|---|---|
| **ISO 9001:2015** (SGC) | Sistema de Gestión de la Calidad | Entregar software que cumple requisitos del usuario de forma repetible y mejorable. |
| **ISO 14001:2015** (SGA) | Sistema de Gestión Ambiental | Reducir impacto ambiental del software (uso de CPU/memoria/red) y de su ciclo de vida. |
| **ISO 27001:2022** (SGSI) | Sistema de Gestión de Seguridad de la Información | Proteger secretos (`RIOT_API_KEY`, `Riot ID`), datos de jugadores y la cadena de suministro (SBOM). |

Las tres comparten la estructura **Annex SL** (cláusulas 4-10) y el ciclo **PHVA (Planificar-Hacer-Verificar-Actuar)**, por lo que se implementan como **sistema integrado**, no tres sistemas separados. Para software pequeño aplica **ISO/IEC 29110 (VSEs)** e **ISO/IEC 90003** como guía para llevar ISO 9001 al código.

---

## 1. ISO 9001 — Calidad (SGC)

**Objetivo:** que el usuario reciba lo que espera, cada vez, y que los errores se detecten antes que él.

### Cómo se aplica aquí (sin burocracia innecesaria)

* **4 Contexto / 5 Liderazgo:** `README.md:Objetivos` y `Arquitectura-inicial.md:1` definen alcance y límites (sin inyección/memoria, cumplimiento Riot). El responsable decide alcance y comunica la política.
* **6 Planificación:** `TASKS.md` es el registro de riesgos y oportunidades. Decidir `Provider Layer` desacoplado y caché L1/L2 evita retrabajo.
* **7 Soporte / 8 Operación:** 
  * Configuración versionada (`config.example.toml`, `.env.example`) + validación estricta en `src/config/mod.rs:19` (intervalo 1..60, claves desconocidas → error).
  * Control documental: todo cambio pasa por PR/commit con `cargo fmt/clippy/test` (actualmente 43 tests en `config/cli/game/diagnostics/watch`).
  * Trazabilidad: Raw Data separado de Derived Data (`Arquitectura-inicial.md:7`) — permite recalcular sin volver a pedir a la API.
* **9 Evaluación / 10 Mejora:** `cargo test` + `cargo check` + benchmarks futuros (arranque, CPU idle) como métricas objetivas; `vtracker doctor` como auditoría interna técnica. No conformidades → nuevo test que lo reproduce.

### Evidencia mínima para auditoría ligera

* `TASKS.md` y `README.md` versionados (control documental).
* Tests automáticos y `cargo fmt --check` como inspección.
* `watch.log` opcional como registro de transiciones.
* `CHANGELOG` o `git log --oneline` como historial de cambios.

> **ISO 90003:** guía específica para aplicar ISO 9001 a software — cubre ciclo de vida, gestión de requisitos, diseño, pruebas y gestión de configuración. Se usa como referencia para no trasladar burocracia fabril al código.

---

## 2. ISO 14001 — Ambiental (SGA)

**Objetivo:** reducir el impacto ambiental del producto a lo largo de su ciclo de vida. En software el impacto es **energía consumida** por CPU, memoria, red y almacenamiento.

### Por qué aplica a una TUI Rust

VTracker está diseñado para quedarse en segundo plano. Cada milisegundo de CPU desperdiciado es calor/energía y batería. **Green coding** es la contraparte de ISO 14001 en software.

### Prácticas ya implementadas y por implementar

* **Diseño eficiente (Planificar):** Rust nativo sin runtime pesado, binario pequeño, modelo **event-driven** (no polling), `tokio::select!` y backoff. Ya medido: `GameState::Unknown` no hace polling continuo, solo `watch` cada 3s configurable.
* **Implementación (Hacer):**
  * Evitar loops innecesarios, usar algoritmos eficientes y `L1/L2` para evitar red repetida (ver `Arquitectura-inicial.md:9`).
  * `auto-launch` con `minimized=true` para no robar foco ni forzar render.
  * Compilar `release` con `lto`/optimizaciones y medir `target` incremental.
* **Verificación (Verificar):** medir en `Prioridad 6` → arranque (<100ms objetivo), CPU idle (<1%), memoria RSS, latencia de caché, tamaño binario. Registrar en `docs/BENCHMARKS.md` futuro.
* **Mejora (Actuar):** optimizar primero lo medido, no suponer. Política de "no recalcular si no cambió la huella de partidas".

### Declaración ambiental inicial

> VTracker (nombre temporal) se compromete a minimizar su huella mediante código eficiente, caché y eventos, y a medir su impacto antes de optimizar. No genera residuos físicos; su impacto ambiental es el consumo energético del host y de la red a APIs.

Formalmente ISO 14001:2015 pide política ambiental, aspectos/impactos, objetivos y control operacional — para este tamaño basta la declaración + métricas.

---

## 3. ISO 27001 — Seguridad de la información (SGSI)

**Objetivo:** proteger confidencialidad, integridad y disponibilidad (CIA) de la información — especialmente **claves API** y **datos de jugadores** que requieren consentimiento RSO.

El SGSI se basa en **Annex A (93 controles, edición 2022)**, agrupados en Organizativos, Personas, Físicos y Tecnológicos. Para software intervienen directamente los controles **8.25 a 8.33**.

### Controles clave mapeados a VTracker

| Control ISO 27001:2022 | Qué exige | Implementación en VTracker |
|---|---|---|
| **A.8.25 Ciclo de vida de desarrollo seguro** | Puertas de seguridad en cada fase SDLC | `TASKS.md:2B` valida diseño antes de codificar provider; `doctor` falla seguro sin exponer secretos. |
| **A.8.26 Requisitos de seguridad de aplicaciones** | Definir requisitos antes de codificar | `docs/ISO.md` + `Arquitectura-inicial.md:10` (límites: sin scouting sin opt-in). |
| **A.8.27 Principios de arquitectura segura** | Threat modeling, diseño seguro | `Provider Layer` desacoplado, `Request Manager` centralizado, secretos solo en `env`. |
| **A.8.28 Codificación segura** | Estándares + análisis estático | `cargo clippy`, `cargo audit`, `serde` tipado, ningún `unwrap` en I/O. |
| **A.8.29 Pruebas de seguridad en desarrollo** | SAST/DAST antes de release | `cargo test` + futuros `cargo deny` y escáner de dependencias. |
| **A.8.32 Gestión de cambios** | Trazabilidad de cambios a producción | Commits firmados, PRs, `git tag` para releases. |
| **A.8.8 Gestión de vulnerabilidades** | SLA de parcheo (ej. 15d crítica) | `cargo audit` en CI, `SBOM` (`cargo cyclonedx`) futuro. |
| **A.5.9 Inventario de activos** | Inventario de información/activos | `Cargo.lock` + futuro `SBOM` como inventario de software. |

### Medidas ya tomadas

* **`.gitignore:7`** ignora `.env`/`.env.*.local`/`*.log`; `.env.example` es la única plantilla versionada.
* **Carga de secretos solo en runtime** vía `dotenvy` (no hardcodeados). `diagnostics::doctor` muestra `***` o `no configurada`, nunca el valor (`src/diagnostics/mod.rs`).
* **`config.toml` fuera del repo** (`%APPDATA%\vtracker\config.toml`) — solo opciones no sensibles.
* **RSO/opt-in obligatorio** para datos personales (Riot Policy) — documentado en `TASKS.md:2A`.

### Hoja de ruta SGSI proporcional

1. **Alcance:** binario `vtracker` + `.env` + `config.toml` + API Riot (cuando se habilite).
2. **SoA (Statement of Applicability):** justificar qué controles de los 93 aplican (los 8 de arriba + básicos organizativos).
3. **Evaluación de riesgos:** `docs/RISK.md` ligero (amenazas: fuga de `RIOT_API_KEY`, exponer datos sin consentimiento, dependencia comprometida).
4. **Auditoría interna:** `doctor` extendido + `cargo audit` + revisión de logs con `tracing`.
5. **Mejora continua:** revisiones trimestrales breves.

> **Nota práctica:** la certificación ISO 27001 formal requiere auditor externo (coste estimado 2026: $5k-10k auditoría + $8k-20k primer año DIY) y 6-12 meses. Para este tamaño se recomienda **adoptar el SGSI ahora y certificar cuando haya tracción comercial**, reutilizando la misma evidencia para SOC 2 si se vende en EE. UU.

---

## Cómo se integran las tres normas en el flujo de trabajo

* **PHVA único:** Planificar en `TASKS.md`/`Arquitectura-inicial.md` → Hacer en `src/` con `cargo test/fmt/clippy` → Verificar con `doctor` + benchmarks + `cargo audit` → Actuar con fixes y nuevos tests.
* **Documentación viva:** `README.md` (usuario), `Arquitectura-inicial.md` (decisiones), `docs/ISO.md` (este), `TASKS.md` (plan) — todos versionados.
* **Medición antes de optimizar:** calidad (tests), ambiental (CPU/mem/latencia), seguridad (audits/SBOM).

## Referencias

* ISO 9001:2015, ISO 90003:2014, ISO/IEC 29110 (VSEs), ISO 14001:2015, ISO/IEC 27001:2022 (93 controles), ISO/IEC 27001:2022 Guía para pymes (PUB100484).
* Riot Developer Portal — VALORANT API & RSO, política de opt-in.
* Green coding / carbon-conscious coding (ANSI, 2025).

---
*Última actualización: 2026-08-24 — integrado al repo `vtracker` (nombre temporal).*
