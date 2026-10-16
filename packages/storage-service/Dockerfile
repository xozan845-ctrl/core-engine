# Core Engine · storage-service
# Dockerfile del servicio `storage-service` (workspace NestJS, npm workspaces).
# Contexto de build: la RAÍZ del workspace.
#   docker build -f packages/storage-service/Dockerfile -t core-engine-storage-service .
#
# Imagen por servicio: se publica y promueve por su propia etiqueta (R-CD-1..8).

FROM node:20-alpine AS build
WORKDIR /app

# Declaraciones de todos los workspaces + escaneo ligero del código.
# Node 20 = misma versión que CI (R-EN-1); engines del repo pide >=20.
COPY package.json package-lock.json tsconfig.base.json ./
COPY packages packages

# Instalación reproducible desde el lock (R-EN-1): npm ci, no npm install.
RUN npm ci --prefer-offline --no-audit --no-fund
# npm workspaces anida algunas dependencias por workspace; crear los directorios
# asegura que el COPY del runtime no falle.
RUN mkdir -p packages/storage-service/node_modules packages/shared/node_modules

# Compilación: shared primero (dependencia), luego el servicio objetivo.
RUN npm run build -w @core/shared && npm run build -w @core/storage-service

# El runtime NO lleva dependencias de desarrollo (R-CD-14): tras compilar
# se poda el árbol para no enviar tooling de test (vitest, swc, eslint…).
RUN npm prune --omit=dev

FROM node:20-alpine AS runtime
WORKDIR /app
ENV NODE_ENV=production

COPY --from=build /app/node_modules ./node_modules
COPY --from=build /app/package.json ./package.json
COPY --from=build /app/packages/shared/dist ./packages/shared/dist
COPY --from=build /app/packages/shared/package.json ./packages/shared/package.json
# dependencias que npm anida bajo `shared` (no hoisted en la raiz): sin esto el
# servicio no arranca (MODULE_NOT_FOUND en rabbit.service; p. ej. amqplib 2).
COPY --from=build /app/packages/shared/node_modules ./packages/shared/node_modules
COPY --from=build /app/packages/storage-service/dist ./packages/storage-service/dist
COPY --from=build /app/packages/storage-service/package.json ./packages/storage-service/package.json
# dependencias que npm anida bajo el propio workspace (no hoisted en la raíz):
# `minio` y sus transitivas quedan aquí en algunas resoluciones del workspace.
COPY --from=build /app/packages/storage-service/node_modules ./packages/storage-service/node_modules

USER node
EXPOSE 3010
CMD ["node", "packages/storage-service/dist/main.js"]
