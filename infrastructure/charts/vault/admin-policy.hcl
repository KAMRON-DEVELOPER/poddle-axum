path "kvv2/data/*" {
  capabilities = ["create", "read", "update", "delete"]
}
path "kvv2/metadata/*" {
  capabilities = ["list", "read", "delete"]
}
path "kvv2/delete/*" {
  capabilities = ["update"]
}