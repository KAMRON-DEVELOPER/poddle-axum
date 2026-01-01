path "kvv2/data/*" {
  capabilities = ["read", "create", "update"]
}
path "kvv2/metadata/*" {
  capabilities = ["list", "read"]
}
path "kvv2/delete/*" {
  capabilities = ["update"]
}