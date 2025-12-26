#!/bin/bash

echo "Starting port forwarding for all services..."
echo "Press Ctrl+C to stop all"

# Function to cleanup on exit
cleanup() {
    echo "Stopping all port forwards..."
    pkill -f "kubectl port-forward"
    exit 0
}

trap cleanup EXIT INT TERM

# PostgreSQL
echo "✅ PostgreSQL → localhost:5432"
kubectl port-forward -n postgres-ns svc/postgres-service 5432:5432 &

# Redis
echo "✅ Redis → localhost:6379"
kubectl port-forward -n redis-ns svc/redis-service 6379:6379 &

# RabbitMQ
echo "✅ RabbitMQ → localhost:5672 (AMQP), localhost:15672 (Management)"
kubectl port-forward -n rabbitmq-ns svc/rabbitmq-service 5672:5672 &
kubectl port-forward -n rabbitmq-ns svc/rabbitmq-service 15672:15672 &

# Kafka
echo "✅ Kafka → localhost:9092"
kubectl port-forward -n kafka-ns kafka-ss-0 9092:9092 &

echo ""
echo "All services port forwarded! Press Ctrl+C to stop."
wait