---
description: Procesa recibos de nómina PDF, extrae datos con markitdown y registra en hledger
agent: implementation
---

Procesa recibos de nómina PDF de la carpeta de entrada, extrae los datos relevantes
con markitdown, registra cada recibo como transacción en hledger y mueve los archivos
a la carpeta de procesados.

## Flujo general

1. Lista los PDF en la carpeta inbox definida por el tenant activo.
2. Si no hay archivos, reporta "No hay recibos pendientes" y termina.
3. Para cada PDF:
   - Extrae contenido con `~/.orbit/cache/venv/bin/markitdown <archivo>`
   - Parsea: fecha de pago, empresa/patrón, neto a pagar, período (si disponible)
   - Si un campo crítico (fecha o neto) no se puede extraer, pregunta al usuario antes de continuar
   - Muestra preview de la transacción propuesta y pide confirmación
   - Si confirma: registra en hledger y mueve a procesados
   - Si salta: deja el archivo en inbox sin registrar
4. Muestra resumen: archivos procesados vs saltados.

## Configuración por tenant

Las rutas de inbox, procesados y el journal se definen en el override del tenant.
Ver override en `tenants/FINANCE/source-of-truth/orbit/commands/nomina-import.md`.
