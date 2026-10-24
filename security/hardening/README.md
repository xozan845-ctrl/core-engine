# Hardening del host (SSH y TLS)

Estos cambios **no** se aplican desde el repositorio: son configuración del VPS
o del proxy de borde. Aplícalos con una sesión SSH abierta y valida antes de
recargar para no quedarte fuera.

## SSH · deshabilitar HMAC SHA-1

Hallazgo B-02: `ssh-sha1-hmac-algo` en `:22` (OpenSSH 10.2p1 Ubuntu).

```sh
sudo install -m 0644 security/hardening/sshd_config.d/99-core-engine.conf \
  /etc/ssh/sshd_config.d/99-core-engine.conf
sudo sshd -t
# En otra sesión, comprobar que entras antes de cerrar la actual:
sudo systemctl reload ssh
```

Verificación:

```sh
sudo sshd -T | grep -i '^macs'   # no debe listar sha1
nmap -p22 --script ssh2-enum-algos api.kbcoleccion.com
```

No se toca `PasswordAuthentication` ni `PermitRootLogin` en este fichero: son
decisiones de acceso. Si usas solo claves, añade:

```conf
PermitRootLogin prohibit-password
PasswordAuthentication no
```

## TLS · quitar CBC y fijar curvas

Hallazgo B-03: TLS 1.2 con preferencia de cipher del cliente y CBC en la lista.
El borde de Dockploy es Traefik; aplica `traefik-tls.yml` como configuración
dinámica (file provider) y recarga Traefik.

```sh
# Ejemplo: copiar al directorio dinámico de Traefik y validar
sudo cp security/hardening/traefik-tls.yml /etc/traefik/dynamic/tls.yml
docker restart traefik   # o el mecanismo de reload de Dockploy
```

Verificación:

```sh
nmap -p443 --script ssl-enum-ciphers api.kbcoleccion.com
```

Esperado: TLS 1.2 solo con AES-GCM/CHACHA20 (sin CBC) y TLS 1.3. Si Traefik
mantiene `cipher preference: client`, es informativo: el servidor elige de la
lista permitida y ya no hay suites CBC. Si quieres eliminar también esa
observación, valora `minVersion: VersionTLS13` (solo si tus clientes lo soportan).

## Firewall · 8080

Además del bind a loopback en el compose (H-01), cierra el puerto en el host:

```sh
sudo ufw deny 8080/tcp
sudo ufw reload
```

Verifica desde fuera: `curl -m 5 http://api.kbcoleccion.com:8080/` debe agotar
tiempo o ser rechazado, nunca devolver el JSON del gateway.
