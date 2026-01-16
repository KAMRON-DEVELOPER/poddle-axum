import { NodeSDK } from '@opentelemetry/sdk-node';
import { OTLPTraceExporter } from '@opentelemetry/exporter-trace-otlp-grpc';
import { resourceFromAttributes } from '@opentelemetry/resources';
import { ATTR_SERVICE_NAME, ATTR_SERVICE_VERSION } from '@opentelemetry/semantic-conventions';
import { HttpInstrumentation } from '@opentelemetry/instrumentation-http';
import { ExpressInstrumentation } from '@opentelemetry/instrumentation-express';

// -----------------------------
// Configuration
// -----------------------------
export const SERVICE_NAME = process.env.OTEL_SERVICE_NAME || 'notes-service';
const OTEL_ENDPOINT = process.env.OTEL_EXPORTER_OTLP_ENDPOINT || 'https://alloy-gateway.poddle.uz:4317';

console.log(`[INFO] Initializing tracing for ${SERVICE_NAME} to ${OTEL_ENDPOINT}`);

// -----------------------------
// OpenTelemetry Setup
// -----------------------------
const traceExporter = new OTLPTraceExporter({
  url: OTEL_ENDPOINT,
});

const resource = resourceFromAttributes({
  [ATTR_SERVICE_NAME]: SERVICE_NAME,
  [ATTR_SERVICE_VERSION]: '1.0.0',
  'deployment.environment': 'production',
});

export const sdk = new NodeSDK({
  resource,
  traceExporter,
  instrumentations: [new HttpInstrumentation(), new ExpressInstrumentation()],
});
