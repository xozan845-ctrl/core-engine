# ADR-0036 — Endurecimiento de autenticación

- **Fecha:** 2026-11-19
- **Estado:** Aceptado (implementado)
- **Relacionado:** [ADR-0034](./0034-ciclo-vida-tenant.md)

## Contexto

El login solo validaba bcrypt y el rate limit por IP del gateway. No había
lockout por cuenta, verificación de correo, política fuerte de contraseñas,
MFA ni gestión de sesiones/dispositivos.

## Decisión

1. **Política de contraseñas.** 12–128 caracteres, mayúscula, minúscula,
   número y símbolo; no puede contener el correo, el nombre ni una lista de
   contraseñas comunes. Se aplica en registro, alta administrativa, cambio y
   restablecimiento.
2. **Lockout por cuenta.** 5 intentos fallidos bloquean 15 minutos con
   `423 CUENTA_BLOQUEADA`; un login exitoso resetea el contador. El rate limit
   por IP del gateway sigue activo como primera barrera.
3. **Verificación de correo.** Token de un solo uso, hash SHA-256 en BD,
   vigencia 24 h y enlace al frontend. `AUTH_EXIGIR_CORREO_VERIFICADO` exige
   correo verificado en login/refresh; por defecto se activa en producción.
   Los usuarios existentes se backfillean como verificados. Aceptar una
   invitación de personal marca el correo como verificado.
4. **MFA opcional TOTP.** Secreto de 20 bytes en base32, SHA-1, 6 dígitos,
   30 s. `iniciar` devuelve el `otpauth://`; `confirmar` activa y entrega 8
   códigos de recuperación de un solo uso (hasheados). El login acepta TOTP o
   código de recuperación. `desactivar` exige la contraseña actual.
5. **Sesiones/dispositivos.** Cada sesión guarda `user_agent`, `ultimo_uso_en`
   y permite listar/revocar desde `/auth/sesiones`.
6. **Metadatos.** El gateway reenvía `User-Agent`; el login lo persiste.

## Consecuencias

- **Positivas:** cuentas resistentes a fuerza bruta, correo verificado, MFA y
  revocación de dispositivos; la contraseña por defecto deja de ser suficiente.
- **Contrato:** nuevos códigos `CUENTA_BLOQUEADA`, `CORREO_NO_VERIFICADO`,
  `MFA_REQUERIDO`, `MFA_INVALIDO`; nuevas rutas de verificación, MFA y
  sesiones.
- **Datos:** migración `99_auth_hardening.sql`; tokens de verificación y
  códigos MFA en tablas propias.
- **Operación:** el secreto TOTP se guarda en BD; debe protegerse con cifrado
  de disco/backups y acceso restringido. Rotar/deshabilitar MFA requiere la
  contraseña actual.
- **Pendiente:** MFA obligatorio por política de tenant; códigos de
  recuperación descargables; notificación al usuario cuando se activa MFA o se
  revoca una sesión.

## Criterios de aceptación

- Una contraseña corta o común responde `CONTRASENA_DEBIL`.
- Cinco fallos bloquean la cuenta y ni la contraseña correcta entra.
- Un token de verificación es de un solo uso.
- Con la exigencia activa, un correo sin verificar no puede iniciar sesión.
- Un usuario con MFA no entra sin código; TOTP y recuperación funcionan.
- Listar y revocar una sesión invalida su refresh.
