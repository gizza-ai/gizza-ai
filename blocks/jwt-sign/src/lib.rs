//! gizza-ai/jwt-sign — build and sign a JSON Web Token (JWT/JWS compact form)
//! from a header + payload using an HMAC secret (HS*) or an RSA/EC private key
//! (RS*/ES*). Thin wrapper; chat schema single-sourced from descriptor(); handler
//! delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_jwt_sign_core::{sign, Alg};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    payload: String,
    secret: String,
    #[serde(default = "default_alg")]
    algorithm: String,
    #[serde(default)]
    header: String,
}

fn default_alg() -> String {
    "HS256".to_string()
}

#[derive(Serialize)]
struct Resp {
    jwt: String,
    algorithm: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("payload")
                .required()
                .describe("The JWT claims set as a JSON object, e.g. {\"sub\":\"123\",\"name\":\"Ada\",\"exp\":1893456000}."),
        )
        .param(Param::string("secret").required().describe(
            "The signing key: for HS* the shared HMAC secret string; for RS*/ES* a PEM-encoded private key (PKCS#8 '-----BEGIN PRIVATE KEY-----', or PKCS#1 for RSA).",
        ))
        .param(
            Param::enumv(
                "algorithm",
                ["HS256", "HS384", "HS512", "RS256", "RS384", "RS512", "ES256", "ES384"],
            )
            .default("HS256")
            .describe("Signing algorithm: HS256/384/512 (HMAC), RS256/384/512 (RSA PKCS#1 v1.5), or ES256/384 (ECDSA). Default HS256."),
        )
        .param(Param::string("header").describe(
            "Optional extra JOSE header fields as a JSON object, e.g. {\"kid\":\"key-1\"}. 'alg' is always set from the chosen algorithm and 'typ' defaults to 'JWT'.",
        ))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JwtSign;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/jwt-sign",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build and sign a JSON Web Token (JWT)",
    skill(
        description = "Build and sign a JSON Web Token (JWT, JWS compact serialization) from a header and payload. algorithm=HS256 (default), HS384, HS512 (HMAC with a shared secret), RS256/384/512 (RSASSA-PKCS1-v1_5 with a PEM RSA private key), or ES256/384 (ECDSA with a PEM P-256/P-384 private key). 'payload' is the JSON claims object; 'secret' is the HMAC secret (HS*) or PEM private key (RS*/ES*); optional 'header' adds JOSE header fields (alg is set automatically, typ defaults to JWT). Returns the compact JWT string. Runs locally — the secret/key and claims never leave the device.",
        parameters = schema_json()
    ),
)]
impl JwtSign {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "jwt-sign", |a: Args| {
            let alg = Alg::parse(&a.algorithm).map_err(SkillError::InvalidArgs)?;
            let jwt = sign(&a.header, &a.payload, &a.secret, alg)
                .map_err(SkillError::InvalidArgs)?;
            Ok(Resp {
                jwt,
                algorithm: alg.name().to_string(),
            })
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "payload": { "type": "string", "description": "The JWT claims set as a JSON object, e.g. {\"sub\":\"123\",\"name\":\"Ada\",\"exp\":1893456000}." },
                    "secret": { "type": "string", "description": "The signing key: for HS* the shared HMAC secret string; for RS*/ES* a PEM-encoded private key (PKCS#8 '-----BEGIN PRIVATE KEY-----', or PKCS#1 for RSA)." },
                    "algorithm": { "type": "string", "enum": ["HS256", "HS384", "HS512", "RS256", "RS384", "RS512", "ES256", "ES384"], "default": "HS256", "description": "Signing algorithm: HS256/384/512 (HMAC), RS256/384/512 (RSA PKCS#1 v1.5), or ES256/384 (ECDSA). Default HS256." },
                    "header": { "type": "string", "description": "Optional extra JOSE header fields as a JSON object, e.g. {\"kid\":\"key-1\"}. 'alg' is always set from the chosen algorithm and 'typ' defaults to 'JWT'." }
                },
                "required": ["payload", "secret"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
