# 1. Initialize vault and save keys

kubectl exec -n vault vault-0 -- vault operator init \
  -key-shares=5 -key-threshold=3 -format=json > ~/certs/vault-keys.json

## 2. Add to your config

add-vault-config ~/certs/vault-keys.json

## Enter: staging

## Enter: <https://vault-staging.poddle.uz>

## 3. Done! Now you can use

vault-staging
vault-unseal-staging

## 4. Unseal your vault

kubectl exec -n vault vault-0 -- vault operator unseal $UNSEAL_KEY1
kubectl exec -n vault vault-0 -- vault operator unseal $UNSEAL_KEY2
kubectl exec -n vault vault-0 -- vault operator unseal $UNSEAL_KEY3
