# Primeros pasos

## Requisitos

- **tmux** — para gestión de sesiones
- **Al menos un engine de IA:** [opencode](https://opencode.ai), [Gemini CLI](https://github.com/google-gemini/gemini-cli), o [Claude Code](https://claude.ai/code)
- Linux o macOS

---

## Instalación

### Binario precompilado (recomendado)

```bash
# Linux x86_64
curl -fsSL https://github.com/befraeloircorona/orbit/releases/latest/download/orbit-linux-x86_64 \
  -o ~/.local/bin/orbit && chmod +x ~/.local/bin/orbit

# Linux aarch64
curl -fsSL https://github.com/befraeloircorona/orbit/releases/latest/download/orbit-linux-aarch64 \
  -o ~/.local/bin/orbit && chmod +x ~/.local/bin/orbit

# macOS Apple Silicon
curl -fsSL https://github.com/befraeloircorona/orbit/releases/latest/download/orbit-macos-aarch64 \
  -o /usr/local/bin/orbit && chmod +x /usr/local/bin/orbit

# macOS Intel
curl -fsSL https://github.com/befraeloircorona/orbit/releases/latest/download/orbit-macos-x86_64 \
  -o /usr/local/bin/orbit && chmod +x /usr/local/bin/orbit
```

### Desde fuente

```bash
git clone https://github.com/befraeloircorona/orbit.git
cd orbit
cargo build --release
cp target/release/orbit ~/.local/bin/
```

Requiere Rust 1.75+.

---

## WSL (Windows Subsystem for Linux)

Instala las dependencias antes de correr orbit:

```bash
# tmux
sudo apt-get install -y tmux

# Node.js (para engines npm)
curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash -
sudo apt-get install -y nodejs
```

---

## Configuración inicial

Ejecuta el wizard de setup para configurar engines, plugins y directorio de instalación:

```bash
orbit setup
```

El wizard:
1. Detecta qué engines están instalados y su estado de autenticación
2. Ofrece instalar los que falten
3. Detecta plugins disponibles y ofrece instalarlos
4. Configura el directorio de instalación de orbit

---

## Primera sesión

```bash
# Verificar entorno
orbit doctor

# Lanzar desde el directorio actual (auto-detecta scope)
orbit launch .

# O abrir el TUI interactivo
orbit
```

Si orbit detecta automáticamente el scope desde tu directorio actual, lanza directamente con el contexto correcto: tenant, proyecto, repositorio e instrucciones acumuladas de todas las capas.

---

## Verificar la instalación

```bash
orbit status          # snapshot rápido: engine, workspace, daemon, versión
orbit doctor          # diagnóstico completo de entorno
```

`orbit doctor` verifica: tmux, engines de IA, AI root, socket del daemon, directorio de instalación, plugins activos.

---

## Siguientes pasos

- Entender el [Modelo de workspace](Modelo-de-workspace) para organizar tu contexto
- Ver todos los [Comandos](Comandos) disponibles
- Configurar [Plugins](Plugins) para ampliar funcionalidades
