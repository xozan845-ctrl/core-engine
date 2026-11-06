# Core Engine · scheduling-service
FROM node:20-alpine AS build
WORKDIR /app
COPY package.json package-lock.json tsconfig.base.json ./
COPY packages packages
RUN npm ci --prefer-offline --no-audit --no-fund
RUN mkdir -p packages/scheduling-service/node_modules packages/shared/node_modules
RUN npm run build -w @core/shared && npm run build -w @core/scheduling-service
RUN npm prune --omit=dev

FROM node:20-alpine AS runtime
WORKDIR /app
ENV NODE_ENV=production
COPY --from=build /app/node_modules ./node_modules
COPY --from=build /app/package.json ./package.json
COPY --from=build /app/packages/shared/dist ./packages/shared/dist
COPY --from=build /app/packages/shared/package.json ./packages/shared/package.json
COPY --from=build /app/packages/shared/node_modules ./packages/shared/node_modules
COPY --from=build /app/packages/scheduling-service/dist ./packages/scheduling-service/dist
COPY --from=build /app/packages/scheduling-service/package.json ./packages/scheduling-service/package.json
COPY --from=build /app/packages/scheduling-service/node_modules ./packages/scheduling-service/node_modules
USER node
EXPOSE 3011
CMD ["node", "packages/scheduling-service/dist/main.js"]
