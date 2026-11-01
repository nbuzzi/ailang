# Agent setup and handoff prompt

Copy the prompt below to the agent that will use Ailang. Replace project paths
with the actual workspace; MCP configuration file locations depend on the host.

```text
Quiero que configures y uses Ailang para reducir los scripts descartables de
Python y PowerShell en este proyecto, especialmente para archivos, logs,
configuraciones y tablas de datos de mi MMORPG.

Repositorio: https://github.com/nbuzzi/ailang
En esta PC existe en: C:/Users/Usuario/Desktop/lang/ailang
Binario compilado: C:/Users/Usuario/Desktop/lang/ailang/target/release/ailang.exe

1. Instalación y verificación
- Primero identifica el proyecto en el que trabajarás y lee sus AGENTS.md.
- Revisa el clon existente y sus cambios antes de actualizarlo; no sobrescribas
  trabajo local. Si no existe, clona el repositorio en una carpeta de herramientas.
- Lee docs/ai-prompt-sheet.md y la sección de políticas del README de Ailang.
- Si hace falta compilar, usa Rust stable y, desde el repositorio:
    cargo test --workspace
    cargo build --release -p ailang-cli
  En Windows el resultado es target/release/ailang.exe; en Linux/macOS,
  target/release/ailang. Usa la ruta absoluta. El binario no necesita Python.
- No descargues un binario ajeno ni configures claves de API: no hacen falta.

2. Integración con tu cliente
- Prefiere el servidor MCP si tu host admite servidores locales stdio.
- Usa como command la ruta absoluta al binario y como args:
    ["serve", "--mcp", "--allow-dir", "RUTA_ABSOLUTA_DEL_PROYECTO", "--no-net"]
- Adapta esta configuración al formato documentado de TU cliente. No asumas que
  todos usan el mismo archivo de configuración o las mismas opciones.
- Si el cliente admite cwd, ponlo en la raíz del proyecto. Si no, usa rutas
  absolutas dentro de las expresiones. --allow-dir no cambia el directorio actual.
- La herramienta MCP se llama ailang y recibe {"expr":"expresión"}.
- Si MCP no está disponible, ejecuta el binario con la expresión como UN argumento
  mediante tu herramienta de procesos. Usa argumentos estructurados si existen.
  Si necesitas un shell, cuida sus reglas de comillas; no inventes sintaxis universal.
- Verifica el registro de la herramienta o explica si tu cliente necesita reinicio.
  No afirmes que MCP quedó conectado solo por escribir el archivo de configuración.

3. Uso habitual
- Prefiere Ailang para búsquedas, lectura, transformaciones JSON/CSV/YAML, hashes
  de texto y ediciones puntuales que sus primitivas ya soporten.
- Ejemplos de expresiones, con rutas relativas al cwd:
    search src TODO | head 20
    head 40 data/items.json
    < data/items.json | json | filter(.level > 20) | sort(.level) | head 20
    < config.json | json | set(.version, "2") > config.json
    glob logs/**/*.log | grep ERROR | head 20
    replace src/example.txt "texto anterior" "texto nuevo"
- Antes de una edición delicada, usa una ejecución CLI con --dry-run para revisar
  el diff, y luego aplica la expresión bajo las autorizaciones normales del proyecto.
- No crees un script Python para una búsqueda, un reemplazo o una conversión simple
  que ya resuelva Ailang. Conserva las herramientas especializadas del proyecto
  cuando sean necesarias: compiladores, tests, Git, parsers, simulaciones y librerías.
- No inventes funciones: Ailang todavía no tiene validate, refs, SQL, XML ni
  operaciones específicas de NPCs, quests o drop tables. Si falta una capacidad,
  usa una alternativa apropiada y explica brevemente por qué.
- Devuelve al contexto solo lo necesario: selecciona campos, usa head y guarda
  resultados extensos en archivos. head limita el resultado, no la memoria usada
  por las etapas anteriores: los pipelines todavía son buffered.

4. Reglas de ejecución que debes conocer
- --read-only bloquea mutaciones locales, subprocessos y HTTP POST.
- --no-net bloquea HTTP y subprocessos. --allow-dir también bloquea subprocessos
  y cd. No desactives estas políticas para sortear un error: usa las herramientas
  autorizadas del host para builds/tests/Git cuando corresponda.
- Un glob se limita por su raíz: con --allow-dir /proyecto, usa rutas dentro de
  /proyecto, no un glob que empiece fuera. Los enlaces fuera de la raíz se rechazan.
- --dry-run devuelve {result, dry_run}; los previews son independientes sobre el
  filesystem actual. No simula una transacción ni permite leer un archivo que una
  sentencia anterior solo habría creado. No hay rollback de ejecuciones reales.
- Las restricciones son de aplicación, no una sandbox de sistema operativo.
- exec("git", ["status"]) ejecuta programa y argumentos directamente.
  exec("git status") ya no es válido; no hay shell implícito.
- Los sinks escriben un string como texto crudo, un valor no string como JSON y
  múltiples valores como un array. No asumas que un único resultado será un array.
- Los errores de herramienta MCP llevan isError=true. Revisa el resultado antes
  de seguir y verifica los cambios mediante los tests normales del proyecto.
- bench solo mide tamaño del fuente y latencia de parseo; no ejecuta expresiones
  ni mide tokens de un modelo. No afirmes porcentajes de ahorro sin medición real.

Configúralo, haz una prueba de lectura y otra de dry-run en una carpeta temporal,
y dime la ruta del binario, la raíz permitida, si MCP quedó realmente disponible
y qué operaciones utilizarás para evitar scripts repetidos en este proyecto.
```

An MCP configuration object for hosts using the `mcpServers` convention:

```json
{
  "mcpServers": {
    "ailang": {
      "command": "C:/Users/Usuario/Desktop/lang/ailang/target/release/ailang.exe",
      "args": [
        "serve",
        "--mcp",
        "--allow-dir",
        "RUTA_ABSOLUTA_DEL_PROYECTO",
        "--no-net"
      ]
    }
  }
}
```

Replace the placeholder before starting the server. On another machine, also
replace the executable path. This JSON convention is not universal across hosts.
