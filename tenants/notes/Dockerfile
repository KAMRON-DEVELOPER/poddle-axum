FROM node:24-alpine AS builder

WORKDIR /app

# Install dependencies
COPY package*.json ./
RUN npm install

# Copy source and config
COPY tsconfig.json ./
COPY src ./src

# Build TypeScript
RUN npm run build

# Stage 2: Run
FROM node:24-alpine

ENV OTEL_EXPORTER_OTLP_ENDPOINT='https://alloy-gateway.poddle.uz:4317'
ENV OTEL_EXPORTER_OTLP_PROTOCOL='grpc'
ENV OTEL_SERVICE_NAME='notes-service'
ENV PORT=3000
ENV NODE_ENV=development

WORKDIR /app

# Copy built artifacts and package files
COPY --from=builder /app/dist ./dist
COPY --from=builder /app/package*.json ./

# Install only production dependencies
RUN npm ci --only=production

EXPOSE ${PORT}

CMD ["node", "dist/app.js"]