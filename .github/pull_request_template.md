## Objetivo

Describe el problema y el resultado del cambio.

## Validación

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo test`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] Probé la TUI con `dashboard --demo` si corresponde.
- [ ] Diferencié claramente demo, implementación y validación real.

## Privacidad y datos

- [ ] No agregué credenciales, identificadores internos ni datos reales de jugadores.
- [ ] Los datos ausentes u ocultos degradan de forma segura.
- [ ] Revisé `docs/ROSTER-POLICY.md` si el cambio afecta datos de terceros.

## Documentación

- [ ] Actualicé README, TASKS o ADR cuando el comportamiento o el alcance cambió.
