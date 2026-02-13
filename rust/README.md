# NOTES

It would be worth mentioning that locally I'm using `native` trusted certificates from system which includes Vault PKI root ca, in production we would use `webpki` which uses a compiled-in list of public CAs (like DigiCert, Let's Encrypt). Or keep using vault pki in prod, I don't know.
