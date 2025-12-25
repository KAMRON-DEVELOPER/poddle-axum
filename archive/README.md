K3S Cloud Service Setup Guide
Prerequisites

2 KVM VMs with K3S installed
Domain configured (e.g., pinespot.uz)
Wildcard DNS: *.app.pinespot.uz → Your K3S LoadBalancer IP

Step 1: Install Cert-Manager
bash# Install cert-manager
kubectl apply -f <https://github.com/cert-manager/cert-manager/releases/download/v1.13.0/cert-manager.yaml>

# Wait for cert-manager to be ready

kubectl wait --for=condition=ready pod -l app.kubernetes.io/instance=cert-manager -n cert-manager --timeout=300s

kubectl apply -f cert-manager-issuer.yaml
kubectl apply -f traefik-config.yaml
kubectl rollout restart deployment/traefik -n kube-system
kubectl apply -f postgres.yaml
kubectl apply -f redis.yaml
kubectl apply -f rabbitmq.yaml

Step 7: Deploy Your Compute Service

Step 8: Create ServiceAccount and RBAC for Compute Service

# Save as compute-rbac.yaml

Step 9: Generate Encryption Key
bash# Generate encryption key for secrets
openssl rand -base64 32

# Add this to your compute-service-secrets in Step 7

Step 10: Deploy Everything
bash# Apply all configurations
kubectl apply -f postgres.yaml
kubectl apply -f redis.yaml
kubectl apply -f rabbitmq.yaml
kubectl apply -f compute-rbac.yaml
kubectl apply -f compute-service.yaml

# Check deployments

kubectl get pods
kubectl get svc
kubectl get ingress

Configure your domain's DNS:

# A Records

api.pinespot.uz      → K3S_LOADBALANCER_IP

# Wildcard for user deployments

*.app.pinespot.uz    → K3S_LOADBALANCER_IP
Monitoring
bash# Watch deployment status
kubectl get deployments -w

# Check logs

kubectl logs -f deployment/compute-service

# Check Traefik

kubectl logs -f deployment/traefik -n kube-system

Troubleshooting
bash# Check cert-manager logs
kubectl logs -n cert-manager deployment/cert-manager

# Check certificate status

kubectl get certificate
kubectl describe certificate compute-service-tls

# Check Traefik logs

kubectl logs -n kube-system deployment/traefik

# Check pod logs

kubectl logs deployment/compute-service -f
