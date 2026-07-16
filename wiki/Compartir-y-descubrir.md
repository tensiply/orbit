# Compartir y descubrir

orbit puede compartir su instancia en la red local para que otros miembros del equipo la usen, y puede descubrir otras instancias orbit en la red.

---

## `orbit serve`

Inicia un servidor TCP con descubrimiento mDNS para compartir orbit en la red local.

```bash
orbit serve                     # inicia en background (non-blocking por defecto)
orbit serve --foreground        # inicia en primer plano (bloqueante)
orbit serve stop                # detiene el servidor
orbit serve status              # estado: running/stopped, puerto, peers conectados
```

### ¿Qué expone?

El servidor expone una API local que permite:
- Lanzar sesiones orbit desde otro equipo
- Consultar el estado de sesiones activas
- Acceder al contexto del workspace

---

## `orbit discover`

Encuentra instancias orbit corriendo en la red local vía mDNS:

```bash
orbit discover                  # lista instancias orbit en la LAN
```

Muestra: hostname, dirección IP, puerto, versión de orbit, workspaces disponibles.

---

## Seguridad

### RBAC

`orbit serve` incluye un sistema de control de acceso (RBAC) basado en roles. Los permisos se configuran por usuario o token.

### JWT local

La autenticación entre cliente y servidor usa JWT generados localmente. No hay dependencia de servicios externos.

### Descubrimiento cero-config

El descubrimiento en LAN usa mDNS — no requiere configuración manual de IPs. Los peers se anuncian automáticamente en la red local.

---

## Tab Peers en el TUI

Al correr `orbit serve`, el tab **Peers** (tecla `9`) en el TUI muestra las instancias orbit conectadas en la red local con su estado y workspaces disponibles.

---

## Casos de uso

- **Equipo compartiendo contexto:** un miembro del equipo corre `orbit serve` con el workspace configurado; otros hacen `orbit discover` y conectan sin duplicar el governance repo
- **Pair programming:** dos desarrolladores comparten sesiones orbit en la misma red
- **CI/CD remoto:** un runner hace `orbit discover` para conectarse a una instancia orbit con el contexto del proyecto
