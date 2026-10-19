use std::sync::Arc;

use rustls::client::danger::ServerCertVerifier;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{
    ClientConfig, DigitallySignedStruct, DistinguishedName, Error, ServerConfig, SignatureScheme,
};

use crate::identity::certificate_binds;

/// Checks the X.509 signature and the Signal identity binding.
/// Handshake signatures still go through the rustls crypto provider.
#[derive(Debug)]
pub struct IdentityVerifier {
    provider: Arc<rustls::crypto::CryptoProvider>,
    expected_identity: Option<Vec<u8>>,
    expected_cert_id: Option<[u8; 32]>,
}

impl IdentityVerifier {
    pub fn peer(provider: Arc<rustls::crypto::CryptoProvider>, expected_identity: Option<Vec<u8>>) -> Self {
        Self {
            provider,
            expected_identity,
            expected_cert_id: None,
        }
    }

    pub fn relay(provider: Arc<rustls::crypto::CryptoProvider>, expected_cert_id: [u8; 32]) -> Self {
        Self {
            provider,
            expected_identity: None,
            expected_cert_id: Some(expected_cert_id),
        }
    }

    fn check_end_entity(&self, end_entity: &CertificateDer<'_>) -> Result<(), Error> {
        if let Some(expected) = &self.expected_cert_id {
            let actual = crate::ids::cert_id_bytes(end_entity.as_ref());
            if &actual != expected {
                return Err(Error::General("relay certificate id does not match".into()));
            }
            let (_, cert) = x509_parser::parse_x509_certificate(end_entity.as_ref())
                .map_err(|err| Error::General(err.to_string()))?;
            cert.verify_signature(None)
                .map_err(|err| Error::General(err.to_string()))?;
            return Ok(());
        }
        let identity = identity_extension(end_entity.as_ref()).map_err(Error::General)?;
        if let Some(expected) = &self.expected_identity {
            if &identity != expected {
                return Err(Error::General("peer identity does not match the pinned key".into()));
            }
        }
        if !certificate_binds(end_entity.as_ref(), &identity).map_err(|err| Error::General(err.to_string()))? {
            return Err(Error::General("certificate is not signed for its identity key".into()));
        }
        Ok(())
    }
}

fn identity_extension(cert_der: &[u8]) -> Result<Vec<u8>, String> {
    let (_, cert) = x509_parser::parse_x509_certificate(cert_der).map_err(|err| err.to_string())?;
    for extension in cert.extensions() {
        if extension.oid.to_id_string() == "1.3.6.1.4.1.55529.1" {
            return Ok(extension.value.to_vec());
        }
    }
    Err("certificate has no identity key".into())
}

impl ServerCertVerifier for IdentityVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, Error> {
        self.check_end_entity(end_entity)?;
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

impl ClientCertVerifier for IdentityVerifier {
    fn offer_client_auth(&self) -> bool {
        true
    }
    fn client_auth_mandatory(&self) -> bool {
        self.expected_cert_id.is_none()
    }
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }
    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, Error> {
        self.check_end_entity(end_entity)?;
        Ok(ClientCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

pub fn provider() -> Arc<rustls::crypto::CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

pub fn client_config(
    provider: Arc<rustls::crypto::CryptoProvider>,
    cert_der: &[u8],
    key_pkcs8: &[u8],
    verifier: Arc<IdentityVerifier>,
    alpn: Option<&[u8]>,
) -> Result<ClientConfig, Error> {
    let cert = CertificateDer::from(cert_der.to_vec());
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_pkcs8.to_vec()));
    let mut config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|err| Error::General(err.to_string()))?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_client_auth_cert(vec![cert], key)?;
    if let Some(protocol) = alpn {
        config.alpn_protocols = vec![protocol.to_vec()];
    } else {
        config.alpn_protocols = vec![b"tunepeerly".to_vec()];
    }
    Ok(config)
}

pub fn server_config(
    provider: Arc<rustls::crypto::CryptoProvider>,
    cert_der: &[u8],
    key_pkcs8: &[u8],
    verifier: Arc<IdentityVerifier>,
) -> Result<ServerConfig, Error> {
    let cert = CertificateDer::from(cert_der.to_vec());
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_pkcs8.to_vec()));
    let mut config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|err| Error::General(err.to_string()))?
        .with_client_cert_verifier(verifier)
        .with_single_cert(vec![cert], key)?;
    config.alpn_protocols = vec![b"tunepeerly".to_vec()];
    Ok(config)
}
