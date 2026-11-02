//! End-to-end smoke client for a running MTProto edge.
//!
//! Connects over MTProto, completes the authorization-key exchange, initializes
//! the session (`invokeWithLayer`/`initConnection`/`help.getConfig`) and drives
//! the login flow (`auth.sendCode` -> `auth.signIn`) against the edge.
//!
//! Run it in WSL (the machine's Application Control policy blocks freshly built
//! binaries on Windows):
//!
//! ```console
//! RTD_EDGE_ADDR=127.0.0.1:2398 RTD_EDGE_PHONE=+10000000000 \
//!   cargo run -p rtd-client --example edge_e2e
//! ```
//!
//! By default it trusts the published gramsrv v2 test RSA server identity; set
//! `RTD_EDGE_PUBLIC_KEY` to a PEM file to trust a different server.

use std::process::ExitCode;
use std::sync::Arc;

use rtd_client::{Client, InitParams, connect_tcp};
use rtd_exchange::ClientConfig;
use rtd_proto::CodecKind;
use rtd_tg::{AuthAuthorization, AuthSentCode, CodeSettings};
use rtd_util::{Clock, SystemClock};

/// The published gramsrv v2 test MTProto RSA public key (PKCS#1 PEM).
const TEST_SERVER_PUBLIC_KEY: &str = "-----BEGIN RSA PUBLIC KEY-----
MIIBCgKCAQEAxF//0M0+/5PzgdNagTX+J+dJgr75ZCTuiG8i4x7YwmJF+jiOGCjm
7X7BLCaMc1+hOZYDL3+Gvle/AKykW1qouaCJMVx/H+2l8LFXLelZ2PLawTb8A7Bl
TqWzL3db5BugMNWziL9TuhR8In1bwKY07QVpR9in5zjAsAGLBk+mGt0DnVyMf1Xo
p2lLCFNmm0F4ykcAeaLCCIPbGWddliLY8xEEhI4GO2l1U3kZMwIOdOnAGJFtgUAo
Te+FHR6F1s9adCVZB1teL/hf9R+WmekJwygVz0MYEH7y6U49T45+/W7OF6X6g0W0
j1uSSrsY4qN7twxbTad9zdGZ7ys+9v+PuQIDAQAB
-----END RSA PUBLIC KEY-----
";

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_owned())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("edge_e2e: FAILED: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let address = env_or("RTD_EDGE_ADDR", "127.0.0.1:2398");
    let phone = env_or("RTD_EDGE_PHONE", "+10000000000");
    let code = env_or("RTD_EDGE_CODE", "12345");
    let api_id: i32 = env_or("RTD_EDGE_API_ID", "2040").parse()?;
    let api_hash = env_or("RTD_EDGE_API_HASH", "b18441a1ff607e10a989891a5462e627");
    let dc: i32 = env_or("RTD_EDGE_DC", "2").parse()?;
    // A gramsrv v2 core speaks layer 229; rtd's own canonical is 232.
    let layer: i32 = env_or("RTD_EDGE_LAYER", "229").parse()?;

    let public_pem = match std::env::var("RTD_EDGE_PUBLIC_KEY") {
        Ok(path) => std::fs::read_to_string(path)?,
        Err(_) => TEST_SERVER_PUBLIC_KEY.to_owned(),
    };
    let public_key = rtd_crypto::parse_rsa_public_key(&public_pem)?;

    let codec = match env_or("RTD_EDGE_CODEC", "intermediate").as_str() {
        "abridged" => CodecKind::Abridged,
        "padded" => CodecKind::PaddedIntermediate,
        "full" => CodecKind::Full,
        _ => CodecKind::Intermediate,
    };
    let obfuscated = std::env::var("RTD_EDGE_OBFUSCATED").is_ok();
    let obfuscation = if obfuscated { Some(&[][..]) } else { None };
    println!("connecting to {address} (dc {dc}, codec {codec:?})");
    let transport = connect_tcp(&address, dc as u16, codec, obfuscation)?;
    let clock: Arc<dyn Clock + Send + Sync> = Arc::new(SystemClock);
    let config = ClientConfig {
        dc,
        temporary: false,
        expires_in: 0,
        public_keys: vec![public_key],
    };
    let mut client = Client::connect(transport, &config, clock)?;
    client.set_layer(layer);
    println!("auth-key exchange complete (salt={})", client.salt());

    let config_object = client.initialize(InitParams {
        api_id,
        device_model: "rtd-edge-e2e".to_owned(),
        system_version: "linux".to_owned(),
        app_version: "0.0.0".to_owned(),
        system_lang_code: "en".to_owned(),
        lang_pack: String::new(),
        lang_code: "en".to_owned(),
    })?;
    println!("initialize ok: {config_object:?}");

    let sent = client.send_code(&phone, api_id, &api_hash, code_settings())?;
    let phone_code_hash = match &sent {
        AuthSentCode::SentCode {
            phone_code_hash, ..
        } => phone_code_hash.clone(),
        other => return Err(format!("unexpected auth.sendCode result: {other:?}").into()),
    };
    println!("auth.sendCode ok: phone_code_hash={phone_code_hash}");

    let authorization = client.sign_in(&phone, &phone_code_hash, Some(&code))?;
    match &authorization {
        AuthAuthorization::Authorization { .. } => {
            println!("auth.signIn ok: authorized");
        }
        AuthAuthorization::AuthorizationSignUpRequired { .. } => {
            println!("auth.signIn ok: sign-up required (no account yet)");
        }
    }
    println!("E2E OK");
    Ok(())
}

fn code_settings() -> CodeSettings {
    CodeSettings {
        flags: 0,
        allow_flashcall: false,
        current_number: false,
        allow_app_hash: false,
        allow_missed_call: false,
        allow_firebase: false,
        unknown_number: false,
        logout_tokens: None,
        token: None,
        app_sandbox: None,
    }
}
