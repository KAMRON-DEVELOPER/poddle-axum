path "kvv2/data/deployments/*" {
  capabilities = ["read", "create", "update"]
}
path "kvv2/metadata/deployments/*" {
  capabilities = ["list", "read"]
}
path "kvv2/delete/deployments/*" {
  capabilities = ["update"]
}