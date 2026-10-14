# Core Engine · logistics-service
# Dockerfile del servicio `logistics-service` (workspace NestJS, npm workspaces).
# Contexto de build: la RAÍZ del workspace.
#   docker build -f packages/logistics-service/Dockerfile -t core-engine-logistics-service .
#
# Imagen por servicio: sustituye al Dockerfile raíz parametrizado por
# `--build-arg SERVICIO` y habilita el release a GHCR imagen-por-servicio
# (R-CD-1..8): cada servicio se publica y se promueve con su propia etiqueta.

FROM node:20-alpine AS build
WORKDIR /app

# Declaraciones de todos los workspaces + escaneo ligero del código.
# Node 20 = misma versión que CI (R-EN-1); engines del repo pide >=20.
COPY package.json package-lock.json tsconfig.base.json ./
COPY packages packages

# Instalación reproducible desde el lock (R-EN-1): npm ci, no npm install.
RUN npm ci --prefer-offline --no-audit --no-fund
# npm workspaces anida algunas dependencias por workspace (p. ej. node-cron en
# commissions/finance); crear el directorio asegura que el COPY del runtime no falle.
RUN mkdir -p packages/logistics-service/node_modules

# Compilación: shared primero (dependencia), luego el servicio objetivo.
RUN npm run build -w @core/shared && npm run build -w @core/logistics-service

# El runtime NO lleva dependencias de desarrollo (R-CD-7/R-CD-22): tras compilar
# se poda el árbol para no enviar tooling de test (jest, ts-jest, babel…).
RUN npm prune --omit=dev

FROM node:20-alpine AS runtime
WORKDIR /app
ENV NODE_ENV=production

COPY --from=build /app/node_modules ./node_modules
COPY --from=build /app/package.json ./package.json
COPY --from=build /app/packages/shared/dist ./packages/shared/dist
COPY --from=build /app/packages/shared/package.json ./packages/shared/package.json
COPY --from=build /app/packages/logistics-service/dist ./packages/logistics-service/dist
COPY --from=build /app/packages/logistics-service/package.json ./packages/logistics-service/package.json
# dependencias que npm anida bajo el propio workspace (no hoisted en la raíz).
COPY --from=build /app/packages/logistics-service/node_modules ./packages/logistics-service/node_modules

USER node
EXPOSE 3005
CMD ["node", "packages/logistics-service/dist/main.js"]
