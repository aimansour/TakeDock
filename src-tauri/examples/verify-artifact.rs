use base64::Engine;
use minisign_verify::{PublicKey, Signature};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 3 {
        return Err("usage: verify-artifact ARTIFACT SIGNATURE EXPECTED_VERSION".into());
    }
    let config: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json"))?;
    let decode = |data: &str| -> Result<String, Box<dyn std::error::Error>> {
        Ok(String::from_utf8(
            base64::engine::general_purpose::STANDARD.decode(data.trim())?,
        )?)
    };
    let key = PublicKey::decode(&decode(
        config["plugins"]["updater"]["pubkey"]
            .as_str()
            .ok_or("missing key")?,
    )?)?;
    let signature = Signature::decode(&decode(&std::fs::read_to_string(&args[1])?)?)?;
    key.verify(&std::fs::read(&args[0])?, &signature, false)?;
    let signed_version = signature
        .trusted_comment()
        .split_whitespace()
        .find_map(|word| word.strip_prefix("version:"));
    if signed_version != Some(args[2].as_str()) {
        return Err("signed version mismatch".into());
    }
    println!("PASS: artifact signature and signed version verified.");
    Ok(())
}
