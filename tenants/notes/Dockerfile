FROM node:24-alpine AS builder

WORKDIR /app

# Install dependencies
COPY package*.json ./
RUN npm install

# Copy source and config
COPY tsconfig.json ./
COPY src ./src

# Build TypeScript
RUN npm install -g typescript
RUN npm run build

# Stage 2: Run
FROM node:24-alpine

ENV OTEL_EXPORTER_OTLP_ENDPOINT='https://alloy-gateway.poddle.uz:4317'
ENV OTEL_SERVICE_NAME='notes-service'
ENV PORT=3000
ENV NODE_ENV=development

WORKDIR /app

# Remove dev dependencies
RUN rm -rf src tsconfig.json

EXPOSE 3000

CMD ["node", "dist/app.js"]