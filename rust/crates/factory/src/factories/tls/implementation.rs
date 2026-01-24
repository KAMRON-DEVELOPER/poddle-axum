use std::{
    fs::File,
    io::{BufRead, BufReader, Cursor},
    path::PathBuf,
};

use rustls::{
    ClientConfig, RootCertStore,
    pki_types::{CertificateDer, PrivateKeyDer},
};
use rustls_pemfile::{Item, read_one};

use crate::factories::tls::TlsConfig;

impl TlsConfig {
    /// Build TLS client config from Config
    pub fn build_rustls_config(&self) -> ClientConfig {
        let ca = self.ca.clone();
        let ca_path = self.ca_path.clone();
        let client_cert = self.client_cert.clone();
        let client_cert_path = self.client_cert_path.clone();
        let client_key = self.client_key.clone();
        let client_key_path = self.client_key_path.clone();

        if ca.is_none() && ca_path.is_none() {
            panic!("Missing Tls Ca");
        }
        if client_cert.is_none() && client_cert_path.is_none() {
            panic!("Missing Tls Cert");
        }
        if client_key.is_none() && client_key_path.is_none() {
            panic!("Missing Tls Key");
        }

        let mut root_store: RootCertStore = RootCertStore::empty();

        // Root CA
        let ca_cert = Self::with_reader(ca, ca_path, "TLS CA", |reader| {
            Self::read_single_cert(reader)
        });

        root_store.add(ca_cert).unwrap_or_else(|_| {
            panic!("Couldn't add root CA to store");
        });

        // Client cert chain
        let client_certs = Self::with_reader(
            client_cert,
            client_cert_path,
            "TLS client certificate",
            |reader| Self::read_cert_chain(reader),
        );

        // Client key
        let client_key =
            Self::with_reader(client_key, client_key_path, "TLS client key", |reader| {
                Self::read_private_key(reader)
            });

        // Build ClientConfig
        let client_config = ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_client_auth_cert(client_certs, client_key)
            .unwrap_or_else(|e| {
                panic!(
                    "Couldn't set client authentication certificate chain, {}",
                    e
                )
            });

        client_config
    }

    fn with_reader<T>(
        data: Option<String>,
        path: Option<PathBuf>,
        context: &str,
        f: impl FnOnce(&mut dyn BufRead) -> T,
    ) -> T {
        if let Some(data) = data {
            let mut reader = Cursor::new(data.as_bytes());
            f(&mut reader)
        } else if let Some(path) = path {
            let display = path.clone();
            let file = File::open(&path)
                .unwrap_or_else(|e| panic!("Couldn't open {} {:?}: {}", context, display, e));
            let mut reader = BufReader::new(file);
            f(&mut reader)
        } else {
            panic!("Missing {}", context);
        }
    }

    fn read_single_cert(reader: &mut dyn BufRead) -> tonic::transport::CertificateDer<'static> {
        match read_one(reader) {
            Ok(Some(Item::X509Certificate(cert))) => cert,
            Ok(Some(_)) => panic!("Unsupported PEM type for certificate"),
            Ok(None) => panic!("No certificate found in PEM"),
            Err(e) => panic!("Failed to read certificate PEM: {}", e),
        }
    }

    fn read_cert_chain(reader: &mut dyn BufRead) -> Vec<CertificateDer<'static>> {
        let mut certs = Vec::new();

        loop {
            match read_one(reader) {
                Ok(Some(Item::X509Certificate(cert))) => certs.push(cert),
                Ok(Some(_)) => panic!("Unsupported PEM type in certificate chain"),
                Ok(None) => break,
                Err(e) => panic!("Failed to read certificate PEM: {}", e),
            }
        }

        if certs.is_empty() {
            panic!("No certificates found in PEM");
        }

        certs
    }

    fn read_private_key(reader: &mut dyn BufRead) -> PrivateKeyDer<'static> {
        match read_one(reader) {
            Ok(Some(Item::Pkcs8Key(key))) => PrivateKeyDer::Pkcs8(key),
            Ok(Some(Item::Pkcs1Key(key))) => PrivateKeyDer::Pkcs1(key),
            Ok(Some(Item::Sec1Key(key))) => PrivateKeyDer::Sec1(key),
            Ok(Some(_)) => panic!("Unsupported PEM type for private key"),
            Ok(None) => panic!("No private key found in PEM"),
            Err(e) => panic!("Failed to read private key PEM: {}", e),
        }
    }
}
