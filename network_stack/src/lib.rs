use aes_gcm_siv::aead::{Aead, KeyInit};
use aes_gcm_siv::{Aes256GcmSiv, Nonce};
use anyhow::{anyhow, Result};
use base64::Engine;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use std::io;
use std::net::{SocketAddr, UdpSocket};
use x25519_dalek::{PublicKey as DhPublicKey, StaticSecret};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum FrameKind {
    Open = 0x01,
    Data = 0x02,
    Close = 0x04,
    Rpc = 0x08,
    Ack = 0x10,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameHeader {
    pub version: u8,
    pub flags: u8,
    pub stream_id: u32,
    pub method: u16,
    pub length: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    pub header: FrameHeader,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn new(kind: FrameKind, stream_id: u32, method: u16, payload: Vec<u8>) -> Self {
        Self {
            header: FrameHeader {
                version: 1,
                flags: kind as u8,
                stream_id,
                method,
                length: payload.len() as u32,
            },
            payload,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(12 + self.payload.len());
        bytes.push(self.header.version);
        bytes.push(self.header.flags);
        bytes.extend_from_slice(&self.header.stream_id.to_be_bytes());
        bytes.extend_from_slice(&self.header.method.to_be_bytes());
        bytes.extend_from_slice(&self.header.length.to_be_bytes());
        bytes.extend_from_slice(&self.payload);
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 12 {
            return Err(anyhow!("frame too short"));
        }
        let version = bytes[0];
        let flags = bytes[1];
        let stream_id = u32::from_be_bytes(bytes[2..6].try_into()?);
        let method = u16::from_be_bytes(bytes[6..8].try_into()?);
        let length = u32::from_be_bytes(bytes[8..12].try_into()?);
        let payload = bytes[12..].to_vec();
        if payload.len() != length as usize {
            return Err(anyhow!("frame payload length mismatch"));
        }
        Ok(Self {
            header: FrameHeader {
                version,
                flags,
                stream_id,
                method,
                length,
            },
            payload,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Identity {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
}

impl Identity {
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        Self {
            signing_key,
            verifying_key,
        }
    }

    pub fn public_id(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(self.verifying_key.to_bytes())
    }

    pub fn sign(&self, payload: &[u8]) -> [u8; 64] {
        self.signing_key.sign(payload).to_bytes()
    }

    pub fn verifying_key(&self) -> VerifyingKey {
        self.verifying_key
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hello {
    pub identity: [u8; 32],
    pub ephemeral_key: [u8; 32],
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Accept {
    pub identity: [u8; 32],
    pub ephemeral_key: [u8; 32],
    pub signature: Vec<u8>,
}

impl Hello {
    pub fn verify(&self) -> Result<()> {
        let verifying = VerifyingKey::from_bytes(&self.identity)?;
        let signature = Signature::from_bytes(
            &self
                .signature
                .clone()
                .try_into()
                .map_err(|_| anyhow!("invalid hello signature length"))?,
        );
        let mut payload = Vec::new();
        payload.extend_from_slice(&self.identity);
        payload.extend_from_slice(&self.ephemeral_key);
        verifying.verify(&payload, &signature)?;
        Ok(())
    }
}

impl Accept {
    pub fn verify(&self) -> Result<()> {
        let verifying = VerifyingKey::from_bytes(&self.identity)?;
        let signature = Signature::from_bytes(
            &self
                .signature
                .clone()
                .try_into()
                .map_err(|_| anyhow!("invalid accept signature length"))?,
        );
        let mut payload = Vec::new();
        payload.extend_from_slice(&self.identity);
        payload.extend_from_slice(&self.ephemeral_key);
        verifying.verify(&payload, &signature)?;
        Ok(())
    }
}

pub fn hello(identity: &Identity, ephemeral: &StaticSecret) -> Hello {
    let ephemeral_key = DhPublicKey::from(ephemeral).to_bytes();
    let identity_bytes = identity.verifying_key().to_bytes();
    let mut payload = Vec::new();
    payload.extend_from_slice(&identity_bytes);
    payload.extend_from_slice(&ephemeral_key);
    Hello {
        identity: identity_bytes,
        ephemeral_key,
        signature: identity.sign(&payload).to_vec(),
    }
}

pub fn accept(identity: &Identity, ephemeral: &StaticSecret) -> Accept {
    let ephemeral_key = DhPublicKey::from(ephemeral).to_bytes();
    let identity_bytes = identity.verifying_key().to_bytes();
    let mut payload = Vec::new();
    payload.extend_from_slice(&identity_bytes);
    payload.extend_from_slice(&ephemeral_key);
    Accept {
        identity: identity_bytes,
        ephemeral_key,
        signature: identity.sign(&payload).to_vec(),
    }
}

#[derive(Debug, Clone)]
pub struct SecureChannel {
    key: [u8; 32],
}

impl SecureChannel {
    pub fn derive(local_secret: &StaticSecret, remote_public: [u8; 32]) -> Self {
        let remote_public = DhPublicKey::from(remote_public);
        let shared = local_secret.diffie_hellman(&remote_public);
        let mut hasher = Sha3_256::new();
        hasher.update(shared.as_bytes());
        let key: [u8; 32] = hasher.finalize().into();
        Self { key }
    }

    pub fn encrypt(&self, sequence: u64, plaintext: &[u8]) -> Result<Vec<u8>> {
        let cipher = Aes256GcmSiv::new_from_slice(&self.key)?;
        let nonce = nonce_from_sequence(sequence);
        cipher
            .encrypt(&nonce, plaintext)
            .map_err(|_| anyhow!("snsp encryption failed"))
    }

    pub fn decrypt(&self, sequence: u64, ciphertext: &[u8]) -> Result<Vec<u8>> {
        let cipher = Aes256GcmSiv::new_from_slice(&self.key)?;
        let nonce = nonce_from_sequence(sequence);
        cipher
            .decrypt(&nonce, ciphertext)
            .map_err(|_| anyhow!("snsp decryption failed"))
    }
}

fn nonce_from_sequence(sequence: u64) -> Nonce {
    let mut bytes = [0u8; 12];
    bytes[4..].copy_from_slice(&sequence.to_be_bytes());
    Nonce::clone_from_slice(&bytes)
}

pub trait Transport {
    fn send(&mut self, bytes: &[u8]) -> io::Result<usize>;
    fn recv(&mut self, buffer: &mut [u8]) -> io::Result<usize>;
}

pub struct UdpTransport {
    socket: UdpSocket,
    peer: SocketAddr,
}

impl UdpTransport {
    pub fn bind(bind: SocketAddr, peer: SocketAddr) -> io::Result<Self> {
        let socket = UdpSocket::bind(bind)?;
        socket.connect(peer)?;
        Ok(Self { socket, peer })
    }

    pub fn peer(&self) -> SocketAddr {
        self.peer
    }
}

impl Transport for UdpTransport {
    fn send(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.socket.send(bytes)
    }

    fn recv(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.socket.recv(buffer)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcEnvelope {
    pub method: String,
    pub payload: Vec<u8>,
}

impl RpcEnvelope {
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        let method = self.method.as_bytes();
        bytes.extend_from_slice(&(method.len() as u16).to_be_bytes());
        bytes.extend_from_slice(method);
        bytes.extend_from_slice(&self.payload);
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 2 {
            return Err(anyhow!("rpc payload too short"));
        }
        let method_len = u16::from_be_bytes(bytes[..2].try_into()?) as usize;
        if bytes.len() < 2 + method_len {
            return Err(anyhow!("rpc method length out of range"));
        }
        let method = String::from_utf8(bytes[2..2 + method_len].to_vec())?;
        let payload = bytes[2 + method_len..].to_vec();
        Ok(Self { method, payload })
    }
}
