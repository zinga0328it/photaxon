use hmac::{Hmac, Mac};
use rand::{Rng, RngCore, rngs::OsRng};
use sha2::Sha256;
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

pub struct OtpMaterial {
    code: String,
    salt: String,
    hash: String,
}

impl OtpMaterial {
    pub fn generate() -> Self {
        let code = format!("{:06}", OsRng.gen_range(0..1_000_000_u32));
        let mut salt_bytes = [0_u8; 16];
        OsRng.fill_bytes(&mut salt_bytes);
        let salt = encode_hex(&salt_bytes);
        let hash = hash_otp(&salt, &code);
        Self { code, salt, hash }
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn salt(&self) -> &str {
        &self.salt
    }

    pub fn hash(&self) -> &str {
        &self.hash
    }
}

pub fn hash_otp(salt: &str, code: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(salt.as_bytes()).expect("HMAC accepts keys of arbitrary length");
    mac.update(code.as_bytes());
    encode_hex(&mac.finalize().into_bytes())
}

pub fn verify_otp(salt: &str, code: &str, expected_hash: &str) -> bool {
    if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    let actual = hash_otp(salt, code);
    bool::from(actual.as_bytes().ct_eq(expected_hash.as_bytes()))
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::{OtpMaterial, hash_otp, verify_otp};

    #[test]
    fn salted_hmac_matches_existing_activation_contract() {
        let salt = "00112233445566778899aabbccddeeff";
        let digest = hash_otp(salt, "123456");
        assert!(verify_otp(salt, "123456", &digest));
        assert!(!verify_otp(salt, "123457", &digest));
        assert!(!verify_otp(salt, "12345", &digest));
    }

    #[test]
    fn generated_otp_has_six_digits_and_unique_salted_hash() {
        let first = OtpMaterial::generate();
        let second = OtpMaterial::generate();
        assert_eq!(first.code().len(), 6);
        assert!(first.code().bytes().all(|byte| byte.is_ascii_digit()));
        assert_eq!(first.salt().len(), 32);
        assert_eq!(first.hash().len(), 64);
        assert_ne!(first.salt(), second.salt());
    }
}
