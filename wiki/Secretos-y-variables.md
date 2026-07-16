# Secretos y variables de entorno

orbit provee dos mecanismos para manejar valores sensibles y variables de configuración: **`orbit secret`** para secretos en el keychain del sistema operativo, y **`orbit env`** para variables de entorno en `orbit.json` por scope.

---

## `orbit secret` — Keychain del sistema operativo

Almacena secretos en el keychain nativo (Keychain en macOS, Secret Service / libsecret en Linux).

```bash
orbit secret set MY_API_KEY       # lee el valor de stdin de forma segura
orbit secret get MY_API_KEY       # imprime el valor
orbit secret delete MY_API_KEY    # elimina el secreto
```

Los secretos nunca se guardan en archivos de config — solo en el keychain. Para referenciarlos en `orbit.json` usa el prefijo `secret://keychain/<key>`.

---

## `orbit env` — Variables de entorno por scope

Gestiona variables de entorno almacenadas en `orbit.json` del scope activo. Se inyectan en el entorno del engine al lanzar.

```bash
orbit env list                      # lista las variables del scope activo
orbit env get MY_VAR                # muestra el valor de una variable
orbit env set MY_VAR valor          # define o actualiza una variable
orbit env delete MY_VAR             # elimina una variable
```

El scope se auto-detecta desde el directorio actual. Las variables se guardan en el bloque `"env"` del `orbit.json` del scope.

---

## Resolvers

Al definir variables en `orbit.json`, puedes usar prefijos especiales para evitar guardar valores en claro:

| Prefijo | Descripción | Ejemplo |
|---|---|---|
| `secret://keychain/<key>` | Lee del keychain del SO en tiempo de lanzamiento | `"secret://keychain/MY_API_KEY"` |
| `env://<VAR>` | Referencia otra variable de entorno del sistema | `"env://HOME"` |
| `file://<path>` | Lee el valor del archivo en el path indicado | `"file://~/.token"` |

### Ejemplo en `orbit.json`

```jsonc
{
  "env": {
    "ANTHROPIC_API_KEY": "secret://keychain/ANTHROPIC_KEY",
    "MY_WORKSPACE": "env://HOME",
    "DEPLOY_TOKEN": "file://~/.deploy-token"
  }
}
```

Los valores con prefijo se resuelven justo antes de lanzar el engine, nunca se almacenan en claro en disco.

---

## Variables exportadas por orbit

Además de las variables definidas en `orbit.json`, orbit siempre exporta estas variables al lanzar:

| Variable | Valor |
|---|---|
| `AI_ENGINE` | Engine activo |
| `AI_TENANT` | Tenant activo |
| `AI_PROJECT` | Proyecto activo |
| `AI_REPOSITORY` | Repositorio activo |
| `AI_WORKSPACE_ROOT` | Ruta al workspace de código |
| `AI_CONTEXT_ROOT` | AI root del workspace |
| `AI_GLOBAL_ROOT` | AI root global |
| `AI_GLOBAL_MODE` | `true` en modo global |

---

## Buenas prácticas

- Usa `orbit secret set` para API keys, tokens y cualquier valor sensible
- Nunca escribas secretos directamente en `orbit.json` — usa `secret://keychain/<key>`
- Las variables de entorno definidas en scopes más específicos (repo) sobrescriben a las de scopes más generales (global)
- Para desarrollo local, un `.env` cargado manualmente antes de `orbit launch` también funciona — pero los resolvers de keychain son más seguros y portables
