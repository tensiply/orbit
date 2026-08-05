# Shell Integration

Integración de orbit con la shell. Necesaria para que `orbit launch` cambie de directorio automáticamente al trabajo dir del scope.

---

## Por qué es necesaria

`orbit launch` abre una sesión en tmux, pero si quieres que tu terminal actual también se mueva al directorio de trabajo del repo, orbit necesita una función de shell que intercepte el comando. Sin la integración, `orbit launch` funciona igual, pero no hace `cd` en la shell actual.

---

## Instalación

### Opción 1 — Automática

```bash
orbit completions install
```

Agrega `eval "$(orbit shell-init)"` al archivo rc del shell detectado (`~/.zshrc`, `~/.bashrc` o `~/.config/fish/config.fish`).

### Opción 2 — Manual

Agrega una línea a tu archivo de configuración de shell:

**zsh / bash** (`~/.zshrc` o `~/.bashrc`):
```bash
eval "$(orbit shell-init)"
```

**fish** (`~/.config/fish/config.fish`):
```fish
orbit shell-init fish | source
```

Luego recarga la shell:
```bash
source ~/.zshrc   # o ~/.bashrc
```

---

## Qué hace

`orbit shell-init` imprime una función `orbit` que envuelve el binario original. Para `orbit launch`, la función llama al binario con `--print-work-dir` para obtener el directorio de trabajo del scope y luego ejecuta `cd` antes de continuar. Para cualquier otro subcomando, pasa los argumentos directamente al binario.

```bash
# Comportamiento de orbit launch con integración activa:
orbit launch AI AIDEV AI-ECOSYSTEM orbit
# → cambia el directorio actual al work_dir del scope
# → abre tmux con la sesión del engine
```

Sin la integración, `orbit launch` funciona normalmente pero no ejecuta el `cd` en tu terminal.

---

## Verificar que está instalada

```bash
orbit doctor
```

La sección **Shell integration** indica si `orbit shell-init` está presente en el rc file del shell activo.

---

## Soporte de shells

| Shell | Mecanismo            |
|-------|----------------------|
| zsh   | función `orbit()`    |
| bash  | función `orbit()`    |
| fish  | función `orbit`      |

El shell se detecta automáticamente desde `$SHELL`. Para forzar un shell específico:

```bash
orbit shell-init zsh
orbit shell-init bash
orbit shell-init fish
```
