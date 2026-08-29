# Roster — disponibilidad técnica y autorización

> Verificado el 2026-08-28 contra la documentación oficial de Riot. Esta nota
> gobierna la integración real del roster y ADR-013. Una API accesible desde el
> cliente no equivale a permiso para usar o mostrar sus datos.

## Decisión actual

VTracker conserva el roster de la partida actual como requisito principal. Por
decisión explícita del usuario (ADR-014), el desarrollo local consulta equipos,
agentes, rango disponible y nombres no ocultos desde los servicios ya usados por
el cliente. Las identidades `Incognito` se muestran como `Jugador N` y no se
envían a Name Service.

El modelo que recibe la interfaz no contiene PUUID ni match ID. La advertencia
oficial de esta nota continúa siendo relevante para una futura distribución,
pero no bloquea el prototipo local solicitado. La implementación no se presenta
como producto aprobado por Riot.

## Matriz de decisión

| Dato o función | Estado en VTracker | Motivo |
|---|---|---|
| Mapa, modo y agente propio | Implementado | Contexto propio efímero; no se expone roster. |
| Roster ficticio de la demo | Implementado | Prototipo local claramente identificado; no consulta jugadores. |
| Nombre visible en partida | Implementado | Name Service se consulta una vez; jugadores `Incognito` quedan excluidos. |
| Agente y rango del roster | Implementado cuando el campo está presente | Sale de `Current Game Match`; ausencia explícita, sin inventar. |
| HS, KAST, K/D, WR e historial | Implementado en el prototipo local | Hasta cinco partidas por jugador desde PD; detalles deduplicados, concurrencia máxima seis y degradación por fila. Su distribución requiere la revisión indicada abajo. |
| Identidad oculta | Implementado | Se muestra `Jugador N` con su agente; el PUUID efímero permite agregar estadísticas Ranked, pero no se consulta ni muestra el Riot ID. |
| Modelo normalizado aliado/enemigo/participante | Implementado con fuente real | Representa campos disponibles u ocultos y no retiene IDs. |

## Fuentes oficiales

- [VALORANT — Getting Started and Policies](https://developer.riotgames.com/docs/valorant): exige registrar los productos, RSO/opt-in para estadísticas personales, respeto de identidades ocultas y enumera el *scouting* como uso no aprobado.
- [Riot Games General Policies](https://developer.riotgames.com/policies/general): exige registrar y auditar productos y funciones nuevas, usar servicios admitidos y no desanonimizar jugadores.
- [OAuth Client Documentation](https://support-developer.riotgames.com/hc/en-us/articles/22897607341075-OAuth-Client-Documentation): documenta el flujo de opt-in y que se necesita una aplicación aprobada antes de operar con OAuth/RSO.
- [Game-specific policies](https://developer.riotgames.com/policies/game-specific): índice oficial de políticas específicas, actualizado el 2026-08-21 al momento de esta revisión.

## Condiciones antes de distribución

1. Registrar VTracker y presentar la maqueta/prototipo para revisión del caso de uso.
2. Obtener una production key y acceso RSO; implementar consentimiento verificable.
3. Definir con Riot qué campos del roster en partida se pueden presentar y en qué fase.
4. Eliminar del diseño las estadísticas previas de rivales; no basta con ocultarlas por configuración.
5. Añadir pruebas de opt-in, identidad oculta, ausencia de campos, redacción de IDs y revocación de sesión.
6. Someter la función nueva a auditoría antes de habilitarla en producción.

El prototipo local puede avanzar bajo ADR-014, pero esas condiciones deben
resolverse antes de presentarlo como producto distribuible o aprobado.
