# Contexto de producto para futuras contribuciones

- Requisito principal confirmado por el usuario el 2026-08-28: mostrar el roster de aliados y enemigos de la partida (diez jugadores en modos 5v5), con rangos y estadísticas disponibles y permitidos. Perfil propio, historial y postpartida son funciones complementarias.
- La implementación actual solo muestra contexto propio. Es una limitación pendiente de resolver, no una decisión de excluir al resto de jugadores del producto.
- Consultar `docs/DECISIONS.md`, ADR-011, y `TASKS.md` antes de cambiar el alcance. No sustituir el requisito principal por un producto de estadísticas exclusivamente propias sin confirmación explícita del usuario.
- Disponibilidad técnica no equivale a autorización. Validar fuentes, términos y consentimiento aplicable antes de implementar consultas de terceros; respetar identidades ocultas y restricciones de privacidad. Mostrar datos no disponibles como tales, sin inventarlos ni eludir controles.
- Distinguir siempre requisito, implementación y validación real. No marcar el roster como terminado porque exista el modelo, un mock o contexto propio de partida.
