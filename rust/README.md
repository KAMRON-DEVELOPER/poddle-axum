# NOTES

It would be worth mentioning that locally I'm using `native` trusted certificates from system which includes Vault PKI root ca, in production we would use `webpki` which uses a compiled-in list of public CAs (like DigiCert, Let's Encrypt). Or keep using vault pki in prod, I don't know.

## BUILD

While docker container building `builder` stage builds actual source code and `sqlx` need to perform compile time check.
As a solution we can use sqlx offline mode by running `cargo sqlx prepare --workspace` in workspace root and setting `SQLX_OFFLINE=true`. It generates a `.sqlx` directory containing multiple JSON files. If you rename `.sqlx` folder you need to set `SQLX_OFFLINE_DIR`.
