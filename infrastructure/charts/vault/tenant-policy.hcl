/*
~ ❯ vault auth list
Path           Type          Accessor                    Description                Version
----           ----          --------                    -----------                -------
kubernetes/    kubernetes    auth_kubernetes_0e73be2f    n/a                        n/a
token/         token         auth_token_3bb5335c         token based credentials    n/a
~ ❯
*/
path "kvv2/data/{{identity.entity.aliases.auth_kubernetes_0e73be2f.metadata.service_account_namespace}}/*" {
  capabilities = ["read"]
}
path "kvv2/metadata/{{identity.entity.aliases.auth_kubernetes_0e73be2f.metadata.service_account_namespace}}/*" {
  capabilities = ["list", "read"]
}