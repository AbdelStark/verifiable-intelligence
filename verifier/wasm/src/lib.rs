use base64::Engine;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use wasm_bindgen::prelude::*;

const COMMITLLM_PIN: &str = "25541e83347655e44ad6e84eb901e1e7ae392a66";

#[wasm_bindgen]
pub fn commitllm_pin() -> String {
    COMMITLLM_PIN.to_string()
}

#[wasm_bindgen]
pub fn verify_v4_audit(key_bytes: &[u8], audit_bytes: &[u8]) -> String {
    verify_v4_audit_value(key_bytes, audit_bytes).to_string()
}

#[wasm_bindgen]
pub fn verify_viex_bundle(bundle_json: &str, key_bytes: &[u8]) -> String {
    verify_viex_bundle_value(bundle_json, key_bytes).to_string()
}

fn verify_viex_bundle_value(bundle_json: &str, key_bytes: &[u8]) -> Value {
    let bundle: Value = match serde_json::from_str(bundle_json) {
        Ok(bundle) => bundle,
        Err(error) => return fail(format!("proof bundle JSON parse failed: {error}")),
    };
    if bundle.get("magic").and_then(Value::as_str) != Some("VIEX") {
        return fail("proof bundle magic must be VIEX");
    }
    if bundle
        .pointer("/verifier/commitllm_pin")
        .and_then(Value::as_str)
        != Some(COMMITLLM_PIN)
    {
        return fail("proof bundle CommitLLM pin does not match WASM verifier pin");
    }
    if bundle
        .pointer("/verifier/verification_mode")
        .and_then(Value::as_str)
        != Some("browser-wasm")
    {
        return fail("proof bundle verifier mode must be browser-wasm");
    }
    if bundle.pointer("/receipt/encoding").and_then(Value::as_str) != Some("base64") {
        return fail("proof bundle receipt must be embedded as base64");
    }

    let bytes_b64 = match bundle.pointer("/receipt/bytes_b64").and_then(Value::as_str) {
        Some(bytes_b64) => bytes_b64,
        None => return fail("proof bundle receipt.bytes_b64 is required"),
    };
    let audit_bytes = match base64::engine::general_purpose::STANDARD.decode(bytes_b64) {
        Ok(bytes) => bytes,
        Err(error) => return fail(format!("receipt base64 decode failed: {error}")),
    };

    let expected_hash = match bundle.pointer("/receipt/sha256").and_then(Value::as_str) {
        Some(hash) => hash,
        None => return fail("proof bundle receipt.sha256 is required"),
    };
    let actual_hash = sha256_hex(&audit_bytes);
    if actual_hash != expected_hash {
        return json!({
            "overall": "fail",
            "error": "receipt sha256 does not match receipt bytes",
            "field": "receipt.sha256",
            "expected": expected_hash,
            "actual": actual_hash,
            "commitllm_pin": COMMITLLM_PIN,
            "verification_mode": "browser-wasm"
        });
    }

    let commitllm = verify_v4_audit_value(key_bytes, &audit_bytes);
    let overall = if commitllm.get("overall").and_then(Value::as_str) == Some("pass") {
        "pass"
    } else {
        "fail"
    };
    json!({
        "overall": overall,
        "commitllm": commitllm,
        "receipt_hash_matches": true,
        "commitllm_pin": COMMITLLM_PIN,
        "verification_mode": "browser-wasm"
    })
}

fn verify_v4_audit_value(key_bytes: &[u8], audit_bytes: &[u8]) -> Value {
    let key = match verilm_core::serialize::deserialize_key(key_bytes) {
        Ok(key) => key,
        Err(error) => return fail(error),
    };
    match verilm_verify::canonical::verify_binary(&key, audit_bytes, None, None, None) {
        Ok(report) => json!({
            "overall": match report.verdict {
                verilm_verify::Verdict::Pass => "pass",
                verilm_verify::Verdict::Fail => "fail",
            },
            "token_index": report.token_index,
            "checks_run": report.checks_run,
            "checks_passed": report.checks_passed,
            "coverage": report.coverage,
            "failures": report.failures,
            "skipped": report.skipped,
            "duration_ms": report.duration.as_secs_f64() * 1000.0,
            "commitllm_pin": COMMITLLM_PIN,
            "verification_mode": "browser-wasm"
        }),
        Err(error) => fail(error),
    }
}

fn fail(error: impl Into<String>) -> Value {
    json!({
        "overall": "fail",
        "error": error.into(),
        "commitllm_pin": COMMITLLM_PIN,
        "verification_mode": "browser-wasm"
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
