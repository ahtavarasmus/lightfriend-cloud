use p256::ecdsa::SigningKey;
use p256::pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding};
use std::error::Error;
use std::fs;
use std::path::Path;
use tracing::info;

const PRIVATE_KEY_PATH: &str = "./tesla_private_key.pem";
const PUBLIC_KEY_PATH: &str = "./tesla_public_key.pem";

// Generate or load EC key pair for Tesla vehicle command signing
pub fn generate_or_load_keys() -> Result<(String, String), Box<dyn Error>> {
    // Check if keys already exist
    if Path::new(PRIVATE_KEY_PATH).exists() && Path::new(PUBLIC_KEY_PATH).exists() {
        info!("Loading existing Tesla EC key pair");
        let private_key = fs::read_to_string(PRIVATE_KEY_PATH)?;
        let public_key = fs::read_to_string(PUBLIC_KEY_PATH)?;
        return Ok((private_key, public_key));
    }

    info!("Generating new Tesla EC key pair (secp256r1)");

    // Generate new key pair
    let signing_key = SigningKey::random(&mut rand::thread_rng());
    let verifying_key = signing_key.verifying_key();

    // Encode private key to PEM
    let private_pem = signing_key
        .to_pkcs8_pem(LineEnding::LF)
        .map_err(|e| format!("Failed to encode private key: {}", e))?;

    // Encode public key to PEM
    let public_pem = verifying_key
        .to_public_key_pem(LineEnding::LF)
        .map_err(|e| format!("Failed to encode public key: {}", e))?;

    // Save keys to files
    fs::write(PRIVATE_KEY_PATH, &private_pem)?;
    fs::write(PUBLIC_KEY_PATH, &public_pem)?;

    info!("Tesla EC key pair generated and saved");
    info!("Private key: {}", PRIVATE_KEY_PATH);
    info!("Public key: {}", PUBLIC_KEY_PATH);

    Ok((private_pem.to_string(), public_pem.to_string()))
}

// Get the public key (used for serving via API endpoint)
pub fn get_public_key() -> Result<String, Box<dyn Error>> {
    if Path::new(PUBLIC_KEY_PATH).exists() {
        Ok(fs::read_to_string(PUBLIC_KEY_PATH)?)
    } else {
        // Generate keys if they don't exist
        let (_, public_key) = generate_or_load_keys()?;
        Ok(public_key)
    }
}
