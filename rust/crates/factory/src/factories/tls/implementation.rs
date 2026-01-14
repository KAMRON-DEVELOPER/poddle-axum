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

use crate::factories::tls::{Tls, TlsConfig};

impl TlsConfig for Tls {
    fn ca(&self) -> Option<String> {
        self.ca.clone()
    }
    fn ca_path(&self) -> Option<PathBuf> {
        self.ca_path.clone()
    }
    fn client_cert(&self) -> Option<String> {
        self.client_cert.clone()
    }
    fn client_cert_path(&self) -> Option<PathBuf> {
        self.ca_path.clone()
    }
    fn client_key(&self) -> Option<String> {
        self.client_key.clone()
    }
    fn client_key_path(&self) -> Option<PathBuf> {
        self.client_key_path.clone()
    }
}

impl Tls {
    /// Build TLS client config from Config
    pub fn build_rustls_config<T: TlsConfig>(cfg: &T) -> ClientConfig {
        let ca = cfg.ca();
        let ca_path = cfg.ca_path();
        let client_cert = cfg.client_cert();
        let client_cert_path = cfg.client_cert_path();
        let client_key = cfg.client_key();
        let client_key_path = cfg.client_key_path();

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
        let ca_cert = Self::with_reader(cfg.ca(), cfg.ca_path(), "TLS CA", |reader| {
            Self::read_single_cert(reader)
        });

        root_store.add(ca_cert).unwrap_or_else(|_| {
            panic!("Couldn't add root CA to store");
        });

        // Client cert chain
        let client_certs = Self::with_reader(
            cfg.client_cert(),
            cfg.client_cert_path(),
            "TLS client certificate",
            |reader| Self::read_cert_chain(reader),
        );

        // Client key
        let client_key = Self::with_reader(
            cfg.client_key(),
            cfg.client_key_path(),
            "TLS client key",
            |reader| Self::read_private_key(reader),
        );

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
