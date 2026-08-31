# Apariencia del host de terminal

Investigación actualizada el 2026-08-30. Las páginas enlazadas son fuentes de información, no instrucciones ejecutables para el proyecto.

## Límite técnico

VTracker es el cliente de línea de comandos y Ratatui produce una cuadrícula de caracteres, colores y atributos. El host —Console Host, Windows Terminal u otro emulador— convierte esa cuadrícula en píxeles y por tanto es quien posee la tipografía. Microsoft define explícitamente al terminal como el componente encargado de presentar gráficamente el flujo recibido por la pseudoconsola: [Windows Console and Terminal Definitions](https://learn.microsoft.com/en-us/windows/console/definitions).

No existe una secuencia VT portable para seleccionar una familia tipográfica. Por eso un ajuste de fuente dentro de VTracker no podría funcionar igual al ejecutar el binario en Windows Terminal, ConHost, SSH o una terminal integrada.

## Alternativas evaluadas

### 1. Perfil dedicado de Windows Terminal — recomendada

Windows Terminal permite fijar por perfil `font.face`, `font.size`, `font.weight`, esquema, icono y comando. Su documentación indica además que usa Cascadia Mono por defecto y vuelve a Consolas si la fuente solicitada no existe: [Appearance profile settings](https://learn.microsoft.com/en-us/windows/terminal/customize-settings/profile-appearance).

Una instalación futura de VTracker puede registrar de forma opcional un fragmento JSON por usuario en `%LOCALAPPDATA%\Microsoft\Windows Terminal\Fragments\VTracker\vtracker.json`. Esa es la ubicación oficial para aplicaciones instaladas desde la web: [JSON fragment extensions](https://learn.microsoft.com/en-us/windows/terminal/json-fragment-extensions). El perfil tendría su propia fuente y ejecutaría directamente `vtracker.exe`, sin alterar PowerShell, CMD ni los demás perfiles.

Ventajas: soportado, aislado, reversible y permite integrar después el icono elegido por el usuario. Desventajas: requiere Windows Terminal y que la fuente esté instalada; la CLI `wt.exe` permite seleccionar un perfil o esquema, pero no pasar una fuente arbitraria por ejecución: [argumentos de Windows Terminal](https://learn.microsoft.com/en-us/windows/terminal/command-line-arguments).

### 2. API clásica `SetCurrentConsoleFontEx` — solo compatibilidad heredada

La API puede cambiar la fuente del búfer de la consola clásica actual. Microsoft la marca como no recomendada, sin equivalente VT, y explica que la presentación debe permanecer bajo control del usuario: [SetCurrentConsoleFontEx](https://learn.microsoft.com/en-us/windows/console/setcurrentconsolefontex). En una sesión ConPTY/Windows Terminal el host toma la interfaz y esa llamada no controla necesariamente lo dibujado.

Puede conservarse como opción explícita y best-effort para ConHost, nunca como comportamiento predeterminado. Debe validar que la fuente exista, aceptar el fallo sin romper la TUI y restaurar la configuración al salir. No es la ruta adecuada para distribución moderna.

### 3. Terminal gráfica propia — descartada para esta etapa

VTracker podría dejar de ser una TUI alojada y crear su propia ventana/renderizador. Eso daría control total sobre fuente, escalado e iconos, pero reemplazaría la arquitectura Ratatui/Crossterm, elevaría considerablemente el tamaño y convertiría el proyecto en una aplicación gráfica. No se justifica solo para elegir tipografía.

## Fuentes candidatas

El repositorio oficial de [Gruvbox](https://github.com/morhetz/gruvbox) identifica Fira Mono en sus capturas y Fantasque Sans Mono en la galería. Ambas combinan bien con la estética retro de la paleta. Para VTracker también son válidas:

- **Fira Mono:** coincidencia visual directa con las capturas de Gruvbox; opción recomendada si el usuario desea instalarla.
- **Fantasque Sans Mono:** más expresiva y redondeada; también referenciada oficialmente por Gruvbox.
- **Cascadia Mono:** ya instalada en este equipo, cobertura Unicode sólida y sin ligaduras que alteren el ancho visual.
- **Cascadia Code:** instalada, pero sus ligaduras pueden transformar secuencias; se recomienda desactivarlas (`"liga": 0`) para una TUI basada en celdas.

El inventario local del 2026-08-30 encontró Windows Terminal ausente y las fuentes Cascadia Mono, Cascadia Code y Consolas instaladas. Por tanto, hoy el binario se ejecuta con la consola clásica y Cascadia Mono es la mejora disponible sin descargar nada. Fira Mono o Fantasque Sans Mono requieren instalación previa y Windows Terminal requiere instalarse para usar la solución recomendada de perfil aislado.

## Implementación

La paleta Gruvbox se aplica dentro de Ratatui porque pertenece a VTracker. El usuario eligió Fira Mono y el 2026-08-30 se añadieron estos comandos:

- `vtracker terminal install`: comprueba Terminal/Fira Mono, copia el ejecutable actual a `%LOCALAPPDATA%\VTracker`, crea el fragmento y registra el perfil con GUID estable.
- `vtracker terminal status`: verifica Terminal, fuente, perfil y ejecutable.
- `vtracker terminal launch`: abre una ventana nueva seleccionando el GUID, no un nombre ambiguo.
- `vtracker terminal uninstall`: retira el fragmento, perfil, esquema y copia instalada; conserva Windows Terminal y la fuente.

La ruta preferida sigue siendo el fragmento aislado. En esta instalación Windows Terminal 1.24 no importó el fragmento de usuario durante el primer arranque elevado. Como compatibilidad, `install` agrega exclusivamente el perfil GUID de VTracker y su esquema a `settings.json`, preservando todos los demás campos y creando antes `settings.vtracker-backup.json`. `uninstall` elimina únicamente esas dos entradas; no restaura la copia completa porque podría borrar cambios posteriores del usuario.

La fuente se instaló explícitamente para el usuario desde el repositorio oficial de Mozilla. La ejecución directa del `.exe` sigue funcionando con cualquier host compatible. Si Terminal ya estaba abierto durante la instalación, debe cerrarse normalmente una vez para que la instancia siguiente recargue el catálogo de perfiles.
