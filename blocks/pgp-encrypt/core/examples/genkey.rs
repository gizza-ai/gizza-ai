//! Dev helper: generate a throwaway OpenPGP key pair and print the armored
//! PUBLIC key to stdout (for manually exercising the CLI). Not shipped.
use pgp::composed::{KeyType, SecretKeyParamsBuilder, SubkeyParamsBuilder};
use pgp::crypto::sym::SymmetricKeyAlgorithm;
use pgp::types::SecretKeyTrait;
use rand::rngs::OsRng;
use smallvec::smallvec;

fn main() {
    let mut sub = SubkeyParamsBuilder::default();
    sub.key_type(KeyType::Rsa(2048)).can_encrypt(true).passphrase(None);
    let sub = sub.build().unwrap();
    let mut params = SecretKeyParamsBuilder::default();
    params
        .key_type(KeyType::Rsa(2048))
        .can_sign(true)
        .can_certify(true)
        .primary_user_id("Test <test@example.com>".into())
        .preferred_symmetric_algorithms(smallvec![SymmetricKeyAlgorithm::AES256])
        .subkey(sub);
    let sk = params.build().unwrap().generate(OsRng).unwrap();
    let sk = sk.sign(OsRng, || String::new()).unwrap();
    let pk = sk.public_key();
    let signed = pk.sign(OsRng, &sk.primary_key, || String::new()).unwrap();
    print!("{}", signed.to_armored_string(Default::default()).unwrap());
}
