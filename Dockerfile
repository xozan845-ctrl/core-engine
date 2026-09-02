# Core Engine · Core Engine
# Dockerfile unico para todos los microservicios del workspace (NestJS).
# Contexto de build: raiz del workspace.
# Uso: docker build --build-arg SERVICIO=<nombre-del-paquete> .

FROM node:24-alpine AS build
WORKDIR /app

ARG SERVICIO
ENV SERVICIO=$SERVICIO

# declaraciones de todos los workspaces (para npm ci) + escaneo ligero de codigo
COPY package.json package-lock.json tsconfig.base.json ./
COPY packages packages

RUN npm install --no-audit --no-fund

# compilacion: shared primero (dependencia), luego el servicio objetivo
RUN npm run build -w @core/shared && npm run build -w @core/$SERVICIO

FROM node:24-alpine AS runtime
WORKDIR /app
ENV NODE_ENV=production

ARG SERVICIO
ENV SERVICIO=$SERVICIO

COPY --from=build /app/node_modules ./node_modules
COPY --from=build /app/package.json ./package.json
COPY --from=build /app/packages/shared/dist ./packages/shared/dist
COPY --from=build /app/packages/shared/package.json ./packages/shared/package.json
COPY --from=build /app/packages/$SERVICIO/dist ./packages/$SERVICIO/dist
COPY --from=build /app/packages/$SERVICIO/package.json ./packages/$SERVICIO/package.json

USER node
EXPOSE 3000
CMD ["sh", "-c", "node packages/$SERVICIO/dist/main.js"]