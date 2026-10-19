use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use async_trait::async_trait;
use base64::Engine;
use libsignal_protocol::{
    DeviceId, Direction, GenericSignedPreKey, IdentityChange, IdentityKey, IdentityKeyPair,
    IdentityKeyStore, KyberPreKeyId, KyberPreKeyRecord, KyberPreKeyStore, PreKeyBundle, PreKeyId,
    PreKeyRecord, PreKeyStore, ProtocolAddress, PublicKey, SenderKeyRecord, SenderKeyStore,
    SessionRecord, SessionStore, SignedPreKeyId, SignedPreKeyRecord, SignedPreKeyStore,
};
use rand::Rng;
use rcgen::{CertificateParams, CustomExtension, DistinguishedName, DnType, KeyPair, SanType};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::ids::{app_device_id, cert_id_display};

const OID_IDENTITY: &[u64] = &[1, 3, 6, 1, 4, 1, 55529, 1];
const OID_SIGNATURE: &[u64] = &[1, 3, 6, 1, 4, 1, 55529, 2];

pub struct IdentityMaterial {
    pub device_id: String,
    pub identity: IdentityKeyPair,
    pub registration_id: u32,
    pub cert_der: Vec<u8>,
    pub cert_key_pkcs8: Vec<u8>,
    pub cert_id: String,
    pub distribution_id: Uuid,
}

pub const TLS_DNS_NAME: &str = "tunepeerly.local";

pub fn load_or_create(db: &Db) -> AppResult<IdentityMaterial> {
    if let Some(bytes) = db.signal_blob("identity")? {
        let identity = IdentityKeyPair::try_from(bytes.as_slice())
            .map_err(|err| AppError::Message(err.to_string()))?;
        let registration_id = registration_from_db(db)?;
        let mut cert_der = db
            .signal_blob("cert_der")?
            .ok_or_else(|| AppError::Message("Missing certificate".into()))?;
        let mut cert_key_pkcs8 = db
            .signal_blob("cert_key")?
            .ok_or_else(|| AppError::Message("Missing certificate key".into()))?;
        if !certificate_names_host(&cert_der, TLS_DNS_NAME) {
            let mut rng = rand::rng();
            let (next_der, next_key) = issue_certificate(&identity, &mut rng)?;
            db.set_signal_blob("cert_der", &next_der)?;
            db.set_signal_blob("cert_key", &next_key)?;
            cert_der = next_der;
            cert_key_pkcs8 = next_key;
        }
        let distribution_id = distribution_from_db(db)?;
        return Ok(IdentityMaterial {
            device_id: app_device_id(&identity.identity_key().serialize()),
            identity,
            registration_id,
            cert_der: cert_der.clone(),
            cert_key_pkcs8,
            cert_id: cert_id_display(&cert_der),
            distribution_id,
        });
    }
    let mut rng = rand::rng();
    let identity = IdentityKeyPair::generate(&mut rng);
    let registration_id: u32 = rng.random::<u32>() & 0x3fff;
    let (cert_der, cert_key_pkcs8) = issue_certificate(&identity, &mut rng)?;
    let distribution_id = Uuid::new_v4();
    db.set_signal_blob("identity", &identity.serialize())?;
    db.set_signal_blob("registration_id", &registration_id.to_be_bytes())?;
    db.set_signal_blob("cert_der", &cert_der)?;
    db.set_signal_blob("cert_key", &cert_key_pkcs8)?;
    db.set_signal_blob("distribution_id", distribution_id.as_bytes())?;
    Ok(IdentityMaterial {
        device_id: app_device_id(&identity.identity_key().serialize()),
        identity,
        registration_id,
        cert_id: cert_id_display(&cert_der),
        cert_der,
        cert_key_pkcs8,
        distribution_id,
    })
}

fn registration_from_db(db: &Db) -> AppResult<u32> {
    let bytes = db
        .signal_blob("registration_id")?
        .ok_or_else(|| AppError::Message("Missing registration id".into()))?;
    let buf: [u8; 4] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| AppError::Message("Missing registration id".into()))?;
    Ok(u32::from_be_bytes(buf))
}

fn distribution_from_db(db: &Db) -> AppResult<Uuid> {
    let bytes = db
        .signal_blob("distribution_id")?
        .ok_or_else(|| AppError::Message("Missing sender key id".into()))?;
    Uuid::from_slice(&bytes).map_err(|err| AppError::Message(err.to_string()))
}

fn issue_certificate(
    identity: &IdentityKeyPair,
    rng: &mut (impl Rng + rand::CryptoRng),
) -> AppResult<(Vec<u8>, Vec<u8>)> {
    let key = KeyPair::generate()?;
    let tls_public = key.public_key_der();
    let identity_public = identity.identity_key().serialize();
    let mut message = tls_public.to_vec();
    message.extend_from_slice(&identity_public);
    let signature = identity
        .private_key()
        .calculate_signature(&message, rng)
        .map_err(|err| AppError::Message(err.to_string()))?;
    let mut params = CertificateParams::new(vec![TLS_DNS_NAME.into()])?;
    params.distinguished_name = DistinguishedName::new();
    params
        .distinguished_name
        .push(DnType::CommonName, "TunePeerly");
    params.subject_alt_names = vec![SanType::DnsName(
        TLS_DNS_NAME.try_into().expect("static dns name"),
    )];
    params.custom_extensions.push(CustomExtension::from_oid_content(
        OID_IDENTITY,
        identity_public.to_vec(),
    ));
    params.custom_extensions.push(CustomExtension::from_oid_content(
        OID_SIGNATURE,
        signature.to_vec(),
    ));
    let cert = params.self_signed(&key)?;
    Ok((cert.der().to_vec(), key.serialize_der()))
}

pub fn pairing_text(identity_public: &[u8], cert_der: &[u8], display_name: &str) -> String {
    let b64 = base64::engine::general_purpose::STANDARD;
    format!(
        "tunepeerly-pair\nname={display_name}\nidentity={}\ncert={}",
        b64.encode(identity_public),
        b64.encode(cert_der)
    )
}

#[derive(Debug, Clone)]
pub struct ParsedPairing {
    pub display_name: String,
    pub identity_public: Vec<u8>,
    pub cert_der: Vec<u8>,
}

pub fn parse_pairing_text(text: &str) -> AppResult<ParsedPairing> {
    let mut name = String::new();
    let mut identity = None;
    let mut cert = None;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("name=") {
            name = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("identity=") {
            identity = Some(
                base64::engine::general_purpose::STANDARD
                    .decode(value.trim())
                    .map_err(|err| AppError::Message(err.to_string()))?,
            );
        } else if let Some(value) = line.strip_prefix("cert=") {
            cert = Some(
                base64::engine::general_purpose::STANDARD
                    .decode(value.trim())
                    .map_err(|err| AppError::Message(err.to_string()))?,
            );
        }
    }
    let identity_public = identity.ok_or_else(|| AppError::Message("Pairing text has no identity key.".into()))?;
    let cert_der = cert.ok_or_else(|| AppError::Message("Pairing text has no certificate.".into()))?;
    if !certificate_binds(&cert_der, &identity_public)? {
        return Err(AppError::Message("Pairing certificate does not match its identity key.".into()));
    }
    Ok(ParsedPairing {
        display_name: if name.is_empty() { "Device".into() } else { name },
        identity_public,
        cert_der,
    })
}

fn certificate_names_host(cert_der: &[u8], host: &str) -> bool {
    let Ok((_, cert)) = x509_parser::parse_x509_certificate(cert_der) else {
        return false;
    };
    cert.subject_alternative_name()
        .ok()
        .flatten()
        .is_some_and(|san| {
            san.value.general_names.iter().any(|name| {
                matches!(name, x509_parser::extensions::GeneralName::DNSName(dns) if dns.eq_ignore_ascii_case(host))
            })
        })
}

pub fn certificate_binds(cert_der: &[u8], expected_identity: &[u8]) -> AppResult<bool> {
    let (_, cert) = x509_parser::parse_x509_certificate(cert_der)
        .map_err(|err| AppError::Message(err.to_string()))?;
    cert.verify_signature(None)
        .map_err(|err| AppError::Message(format!("Certificate signature: {err}")))?;
    let mut identity = None;
    let mut signature = None;
    for extension in cert.extensions() {
        let oid = extension.oid.to_id_string();
        if oid == "1.3.6.1.4.1.55529.1" {
            identity = Some(extension.value.to_vec());
        } else if oid == "1.3.6.1.4.1.55529.2" {
            signature = Some(extension.value.to_vec());
        }
    }
    let identity = identity.ok_or_else(|| AppError::Message("Certificate has no identity key.".into()))?;
    let signature = signature.ok_or_else(|| AppError::Message("Certificate has no identity signature.".into()))?;
    if identity.as_slice() != expected_identity {
        return Ok(false);
    }
    let public = IdentityKey::decode(&identity).map_err(|err| AppError::Message(err.to_string()))?;
    let spki = cert.public_key().raw;
    let mut message = spki.to_vec();
    message.extend_from_slice(&identity);
    Ok(public.public_key().verify_signature(&message, &signature))
}

pub fn safety_number(local: &IdentityKey, remote: &IdentityKey) -> AppResult<String> {
    let local_id = local.serialize();
    let remote_id = remote.serialize();
    let fingerprint = libsignal_protocol::Fingerprint::new(2, 5200, &local_id, local, &remote_id, remote)
        .map_err(|err| AppError::Message(err.to_string()))?;
    fingerprint
        .display_string()
        .map_err(|err| AppError::Message(err.to_string()))
}

#[derive(Default, Serialize, Deserialize)]
struct PersistedMaps {
    sessions: HashMap<String, String>,
    prekeys: HashMap<String, String>,
    signed: HashMap<String, String>,
    kyber: HashMap<String, String>,
    sender_keys: HashMap<String, String>,
}

#[derive(Clone)]
pub struct SignalStore {
    db: Arc<Mutex<Db>>,
    identity: IdentityKeyPair,
    registration_id: u32,
    maps: Rc<RefCell<PersistedMaps>>,
    trusted: Rc<RefCell<HashMap<String, Vec<u8>>>>,
}

impl SignalStore {
    pub fn open(db: Arc<Mutex<Db>>, identity: IdentityKeyPair, registration_id: u32) -> AppResult<Self> {
        let maps = {
            let guard = db.lock().expect("db");
            match guard.signal_blob("signal_maps")? {
                Some(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
                None => PersistedMaps::default(),
            }
        };
        Ok(Self {
            db,
            identity,
            registration_id,
            maps: Rc::new(RefCell::new(maps)),
            trusted: Rc::new(RefCell::new(HashMap::new())),
        })
    }

    fn persist(&self) -> AppResult<()> {
        let bytes = serde_json::to_vec(&*self.maps.borrow())?;
        self.db.lock().expect("db").set_signal_blob("signal_maps", &bytes)
    }

    pub fn trust(&self, name: &str, key: &[u8]) {
        self.trusted.borrow_mut().insert(name.to_string(), key.to_vec());
    }

    /// Dropping our own record makes the next distribution message start a fresh chain.
    pub fn forget_sender_key(&self, sender: &str, distribution_id: Uuid) {
        self.maps
            .borrow_mut()
            .sender_keys
            .remove(&format!("{sender}:{distribution_id}"));
        let _ = self.persist();
    }

    pub fn forget_peer(&self, device_id: &str) {
        {
            let mut maps = self.maps.borrow_mut();
            let prefix = format!("{device_id}:");
            maps.sender_keys.retain(|key, _| !key.starts_with(&prefix));
            maps.sessions.retain(|key, _| !key.starts_with(device_id));
        }
        self.trusted.borrow_mut().remove(device_id);
        let _ = self.persist();
    }

    pub fn address_for(device_id: &str) -> ProtocolAddress {
        ProtocolAddress::new(
            device_id.to_string(),
            DeviceId::new(1).expect("device id 1 is valid"),
        )
    }

    pub async fn ensure_prekeys(&mut self) -> AppResult<PreKeyBundle> {
        let signed_id = SignedPreKeyId::from(1u32);
        if self.maps.borrow().signed.is_empty() {
            let mut rng = rand::rng();
            let pair = libsignal_protocol::KeyPair::generate(&mut rng);
            let signature = self
                .identity
                .private_key()
                .calculate_signature(&pair.public_key.serialize(), &mut rng)
                .map_err(|err| AppError::Message(err.to_string()))?;
            let record = SignedPreKeyRecord::new(
                signed_id,
                libsignal_protocol::Timestamp::from_epoch_millis(
                    SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                ),
                &pair,
                &signature,
            );
            let bytes = record
                .serialize()
                .map_err(|err| AppError::Message(err.to_string()))?;
            self.maps.borrow_mut().signed.insert("1".into(), b64(&bytes));
        }
        let kyber_id = KyberPreKeyId::from(1u32);
        if self.maps.borrow().kyber.is_empty() {
            let record = KyberPreKeyRecord::generate(
                libsignal_protocol::kem::KeyType::Kyber1024,
                kyber_id,
                self.identity.private_key(),
            )
            .map_err(|err| AppError::Message(err.to_string()))?;
            let bytes = record
                .serialize()
                .map_err(|err| AppError::Message(err.to_string()))?;
            self.maps.borrow_mut().kyber.insert("1".into(), b64(&bytes));
        }
        self.persist()?;
        let signed = self.get_signed_pre_key(signed_id).await.map_err(|err| AppError::Message(err.to_string()))?;
        let kyber = self.get_kyber_pre_key(kyber_id).await.map_err(|err| AppError::Message(err.to_string()))?;
        PreKeyBundle::new(
            self.registration_id,
            DeviceId::new(1).expect("device id"),
            None,
            signed_id,
            signed.public_key().map_err(|err| AppError::Message(err.to_string()))?,
            signed.signature().map_err(|err| AppError::Message(err.to_string()))?.to_vec(),
            kyber_id,
            kyber.public_key().map_err(|err| AppError::Message(err.to_string()))?,
            kyber.signature().map_err(|err| AppError::Message(err.to_string()))?.to_vec(),
            *self.identity.identity_key(),
        )
        .map_err(|err| AppError::Message(err.to_string()))
    }
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}
fn unb64(text: &str) -> Result<Vec<u8>, AppError> {
    base64::engine::general_purpose::STANDARD
        .decode(text)
        .map_err(|err| AppError::Message(err.to_string()))
}

#[async_trait(?Send)]
impl IdentityKeyStore for SignalStore {
    async fn get_identity_key_pair(&self) -> libsignal_protocol::error::Result<IdentityKeyPair> {
        Ok(self.identity)
    }
    async fn get_local_registration_id(&self) -> libsignal_protocol::error::Result<u32> {
        Ok(self.registration_id)
    }
    async fn save_identity(
        &mut self,
        address: &ProtocolAddress,
        identity: &IdentityKey,
    ) -> libsignal_protocol::error::Result<IdentityChange> {
        let previous = self.trusted.borrow_mut().insert(address.name().to_string(), identity.serialize().to_vec());
        let changed = previous.as_ref().is_some_and(|old| old.as_slice() != identity.serialize().as_ref());
        Ok(IdentityChange::from_changed(changed))
    }
    async fn is_trusted_identity(
        &self,
        address: &ProtocolAddress,
        identity: &IdentityKey,
        _direction: Direction,
    ) -> libsignal_protocol::error::Result<bool> {
        Ok(match self.trusted.borrow().get(address.name()) {
            Some(saved) => saved.as_slice() == identity.serialize().as_ref(),
            None => true,
        })
    }
    async fn get_identity(
        &self,
        address: &ProtocolAddress,
    ) -> libsignal_protocol::error::Result<Option<IdentityKey>> {
        match self.trusted.borrow().get(address.name()) {
            Some(bytes) => Ok(Some(IdentityKey::decode(bytes)?)),
            None => Ok(None),
        }
    }
}

#[async_trait(?Send)]
impl PreKeyStore for SignalStore {
    async fn get_pre_key(&self, prekey_id: PreKeyId) -> libsignal_protocol::error::Result<PreKeyRecord> {
        let key = u32::from(prekey_id).to_string();
        let Some(text) = self.maps.borrow().prekeys.get(&key).cloned() else {
            return Err(libsignal_protocol::SignalProtocolError::InvalidPreKeyId);
        };
        let bytes = unb64(&text).map_err(|_| libsignal_protocol::SignalProtocolError::InvalidPreKeyId)?;
        PreKeyRecord::deserialize(&bytes)
    }
    async fn save_pre_key(
        &mut self,
        prekey_id: PreKeyId,
        record: &PreKeyRecord,
    ) -> libsignal_protocol::error::Result<()> {
        let bytes = record.serialize()?;
        self.maps.borrow_mut().prekeys.insert(u32::from(prekey_id).to_string(), b64(&bytes));
        let _ = self.persist();
        Ok(())
    }
    async fn remove_pre_key(&mut self, prekey_id: PreKeyId) -> libsignal_protocol::error::Result<()> {
        self.maps.borrow_mut().prekeys.remove(&u32::from(prekey_id).to_string());
        let _ = self.persist();
        Ok(())
    }
}

#[async_trait(?Send)]
impl SignedPreKeyStore for SignalStore {
    async fn get_signed_pre_key(
        &self,
        signed_prekey_id: SignedPreKeyId,
    ) -> libsignal_protocol::error::Result<SignedPreKeyRecord> {
        let key = u32::from(signed_prekey_id).to_string();
        let Some(text) = self.maps.borrow().signed.get(&key).cloned() else {
            return Err(libsignal_protocol::SignalProtocolError::InvalidSignedPreKeyId);
        };
        let bytes = unb64(&text).map_err(|_| libsignal_protocol::SignalProtocolError::InvalidSignedPreKeyId)?;
        SignedPreKeyRecord::deserialize(&bytes)
    }
    async fn save_signed_pre_key(
        &mut self,
        signed_prekey_id: SignedPreKeyId,
        record: &SignedPreKeyRecord,
    ) -> libsignal_protocol::error::Result<()> {
        let bytes = record.serialize()?;
        self.maps.borrow_mut().signed.insert(u32::from(signed_prekey_id).to_string(), b64(&bytes));
        let _ = self.persist();
        Ok(())
    }
}

#[async_trait(?Send)]
impl KyberPreKeyStore for SignalStore {
    async fn get_kyber_pre_key(
        &self,
        kyber_prekey_id: KyberPreKeyId,
    ) -> libsignal_protocol::error::Result<KyberPreKeyRecord> {
        let key = u32::from(kyber_prekey_id).to_string();
        let Some(text) = self.maps.borrow().kyber.get(&key).cloned() else {
            return Err(libsignal_protocol::SignalProtocolError::InvalidKyberPreKeyId);
        };
        let bytes = unb64(&text).map_err(|_| libsignal_protocol::SignalProtocolError::InvalidKyberPreKeyId)?;
        KyberPreKeyRecord::deserialize(&bytes)
    }
    async fn save_kyber_pre_key(
        &mut self,
        kyber_prekey_id: KyberPreKeyId,
        record: &KyberPreKeyRecord,
    ) -> libsignal_protocol::error::Result<()> {
        let bytes = record.serialize()?;
        self.maps.borrow_mut().kyber.insert(u32::from(kyber_prekey_id).to_string(), b64(&bytes));
        let _ = self.persist();
        Ok(())
    }
    async fn mark_kyber_pre_key_used(
        &mut self,
        _kyber_prekey_id: KyberPreKeyId,
        _ec_prekey_id: SignedPreKeyId,
        _base_key: &PublicKey,
    ) -> libsignal_protocol::error::Result<()> {
        Ok(())
    }
}

#[async_trait(?Send)]
impl SessionStore for SignalStore {
    async fn load_session(
        &self,
        address: &ProtocolAddress,
    ) -> libsignal_protocol::error::Result<Option<SessionRecord>> {
        let Some(text) = self.maps.borrow().sessions.get(address.name()).cloned() else {
            return Ok(None);
        };
        let bytes = unb64(&text).map_err(|_| libsignal_protocol::SignalProtocolError::InvalidSessionStructure("session"))?;
        Ok(Some(SessionRecord::deserialize(&bytes)?))
    }
    async fn store_session(
        &mut self,
        address: &ProtocolAddress,
        record: &SessionRecord,
    ) -> libsignal_protocol::error::Result<()> {
        let bytes = record.serialize()?;
        self.maps.borrow_mut().sessions.insert(address.name().to_string(), b64(&bytes));
        let _ = self.persist();
        Ok(())
    }
}

#[async_trait(?Send)]
impl SenderKeyStore for SignalStore {
    async fn store_sender_key(
        &mut self,
        sender: &ProtocolAddress,
        distribution_id: Uuid,
        record: &SenderKeyRecord,
    ) -> libsignal_protocol::error::Result<()> {
        let bytes = record.serialize()?;
        self.maps.borrow_mut().sender_keys.insert(
            format!("{}:{distribution_id}", sender.name()),
            b64(&bytes),
        );
        let _ = self.persist();
        Ok(())
    }
    async fn load_sender_key(
        &mut self,
        sender: &ProtocolAddress,
        distribution_id: Uuid,
    ) -> libsignal_protocol::error::Result<Option<SenderKeyRecord>> {
        let key = format!("{}:{distribution_id}", sender.name());
        let Some(text) = self.maps.borrow().sender_keys.get(&key).cloned() else {
            return Ok(None);
        };
        let bytes = unb64(&text).map_err(|_| libsignal_protocol::SignalProtocolError::InvalidSessionStructure("sender"))?;
        Ok(Some(SenderKeyRecord::deserialize(&bytes)?))
    }
}
