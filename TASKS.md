# VTracker — Lista de tareas

Este documento ordena el trabajo restante. Las integraciones con Riot u otros proveedores solo se implementarán después de confirmar permisos, términos de uso y límites aplicables.

## Hecho: MVP local

- [x] Crear proyecto Rust y documentación de arquitectura.
- [x] Implementar `vtracker watch`.
- [x] Detectar procesos locales de Riot/VALORANT sin acceso a memoria.
- [x] Mostrar estados honestos: cliente cerrado, cliente disponible y juego abierto con modo no confirmado.
- [x] Registrar transiciones recientes en el panel.
- [x] Añadir configuración de intervalo y `--once`.
- [x] Añadir `vtracker doctor` para diagnosticar procesos y configuración.
- [x] Añadir pruebas del motor de transiciones.

## Prioridad 1 — Base fiable

- [ ] Probar `watch` con el cliente cerrado, abierto y el juego abierto en un equipo real.
- [ ] Registrar una tabla de los procesos observados en cada situación para evitar falsos positivos.
- [x] Añadir un log opcional a archivo con fecha, estado y transición.
- [x] Mejorar el parser de configuración y avisar claramente de valores inválidos.
- [ ] Añadir pruebas para configuración, argumentos CLI y salida de `doctor`.
- [x] Revisar la política y APIs oficiales de Riot para el primer proveedor.
- [ ] Definir una fuente autorizada que distinga lobby, selección, partida y postpartida; la API oficial publicada no expone ese estado en tiempo real.

## Prioridad 2 — Fuente de estado y datos

- [ ] Validar la documentación, autenticación, consentimiento, límites y políticas del primer proveedor.
- [ ] Crear una interfaz `GameStateSource` para que el resto de la app no dependa de un proveedor concreto.
- [ ] Implementar el primer adaptador autorizado para estado de cliente/partida.
- [ ] Modelar estados `Lobby`, `PreGame`, `AgentSelect`, `InMatch` y `PostMatch` solo si la fuente los entrega de forma fiable.
- [ ] Mostrar errores recuperables y último estado conocido cuando una fuente falle.
- [ ] Crear `vtracker doctor` para validar la disponibilidad del proveedor, sin exponer secretos.

## Prioridad 3 — Datos propios e historial

- [ ] Definir modelos normalizados de jugador, partida, ronda y resultado.
- [ ] Implementar una capa de providers por capacidades: perfil, historial y detalle de partida.
- [ ] Implementar caché L1 en memoria con TTL.
- [ ] Implementar caché L2 en disco con versión de esquema y expiración.
- [ ] Centralizar solicitudes con timeout, deduplicación y reintentos seguros.
- [ ] Añadir el comando `vtracker history` para consultar el historial propio autorizado.

## Prioridad 4 — Estadísticas

- [ ] Crear el módulo `analytics` separado de providers y UI.
- [ ] Calcular K/D, KDA y win rate a partir de datos normalizados.
- [ ] Calcular HS%, ADR y ACS solo cuando la fuente entregue los campos necesarios.
- [ ] Documentar tratamiento de empates, abandonos, overtime y modos no competitivos.
- [ ] Añadir desgloses por agente, mapa, modo y últimas N partidas.
- [ ] Cubrir las fórmulas con fixtures y pruebas reproducibles.

## Prioridad 5 — TUI completa

- [ ] Añadir Ratatui y Crossterm.
- [ ] Crear Dashboard con estado, salud de fuentes y resumen de sesión.
- [ ] Crear vistas Match, Player, History y Settings.
- [ ] Añadir navegación por teclado, ayuda y estados de carga/error.
- [ ] Adaptar layouts a terminales pequeñas y grandes.
- [ ] Mantener I/O y cálculos fuera del renderizado.

## Prioridad 6 — Robustez y distribución

- [ ] Añadir logs estructurados y niveles configurables.
- [ ] Medir arranque, CPU en reposo, memoria y eficacia de caché.
- [ ] Añadir pruebas de integración para CLI, caché y providers simulados.
- [ ] Añadir `vtracker cache` y `vtracker config`.
- [ ] Preparar binarios de release e instrucciones de instalación.
- [ ] Revisar seguridad de credenciales y privacidad antes de distribuir.

## Siguiente tarea recomendada

Investigar y validar una fuente autorizada que distinga con fiabilidad lobby, selección de agente y partida real. No se debe inferir ese estado a partir de procesos locales.
