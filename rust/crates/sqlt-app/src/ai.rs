// SPDX-License-Identifier: GPL-3.0-only
//! AI assistant: a provider-agnostic, OpenAI-compatible chat client.
//!
//! The endpoint and model live in `<config>/ai.json`; the API key lives in the OS keyring and is never
//! written to a file. There is no telemetry and nothing is sent until the user runs a function. The requests
//! are plain HTTP so a local mock server can be used in tests (no quota, no credentials).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::store::config_dir;

const KEYRING_SERVICE: &str = "com.sqltools.rust";
const KEYRING_ACCOUNT: &str = "ai-key";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConfig {
    /// Base URL including `/v1`; any OpenAI-compatible endpoint works (OpenAI, Azure proxy, Ollama, vLLM).
    pub base_url: String,
    pub model: String,
    pub timeout_secs: u64,
}

impl Default for AiConfig {
    fn default() -> Self {
        AiConfig {
            base_url: "https://api.openai.com/v1".to_string(),
            model: "gpt-4o-mini".to_string(),
            timeout_secs: 60,
        }
    }
}

pub fn config_file() -> PathBuf {
    config_dir().join("ai.json")
}

pub fn load_config(path: &Path) -> AiConfig {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<AiConfig>(&text).ok())
        .unwrap_or_default()
}

pub fn save_config(path: &Path, config: &AiConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let json = serde_json::to_string_pretty(config).map_err(|error| error.to_string())?;
    std::fs::write(path, json).map_err(|error| error.to_string())
}

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT).map_err(|error| error.to_string())
}

/// `scheme://host` with the host as the HTTP client will resolve it: userinfo is stripped (a URL like
/// `http://localhost@evil.com` connects to evil.com) and the port is dropped.
pub fn origin(base_url: &str) -> Option<String> {
    let (scheme, rest) = base_url.trim().split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host_port = authority.rsplit('@').next()?;
    let host = if let Some(inner) = host_port.strip_prefix('[') {
        format!("[{}]", inner.split(']').next()?)
    } else {
        host_port.split(':').next()?.to_string()
    };
    if host.is_empty() {
        return None;
    }
    Some(format!(
        "{}://{}",
        scheme.to_ascii_lowercase(),
        host.to_ascii_lowercase()
    ))
}

/// The stored key, if any. It never leaves the keyring except to sign one request.
pub fn stored_key() -> Result<Option<String>, String> {
    match entry()?.get_password() {
        Ok(key) if !key.is_empty() => Ok(Some(key)),
        Ok(_) => Ok(None),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

/// The key for this configuration. A key is stored with the origin it was typed for, so changing the
/// provider endpoint never sends the previous provider's credential.
pub fn stored_key_for(config: &AiConfig) -> Result<Option<String>, String> {
    let Some(value) = stored_key()? else {
        return Ok(None);
    };
    match value.split_once('\n') {
        Some((stored, key)) => {
            let same = origin(&config.base_url).as_deref() == Some(stored);
            Ok(if same && !key.is_empty() {
                Some(key.to_string())
            } else {
                None
            })
        }
        // Keys stored before the origin was recorded keep working: the user typed them for this config.
        None => Ok(Some(value)),
    }
}

pub fn set_key(key: Option<&str>, base_url: &str) -> Result<(), String> {
    match key.map(str::trim).filter(|key| !key.is_empty()) {
        Some(key) => {
            let origin = origin(base_url).ok_or_else(|| "Invalid base URL".to_string())?;
            entry()?
                .set_password(&format!("{origin}\n{key}"))
                .map_err(|error| error.to_string())
        }
        None => match entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.to_string()),
        },
    }
}

/// The functions the panel offers: `(id, system prompt, user instruction)`.
pub fn instructions(function: &str, code: &str) -> Result<(String, String), String> {
    let system =
        "You are a senior Oracle PL/SQL assistant. Reply with the code or the explanation only: \
                  no markdown fences, no preamble. Keep the original indentation style."
            .to_string();
    let task = match function {
        "create" => "Write the SQL or PL/SQL that does this:",
        "complete" => "Complete the following code, keeping what is there:",
        "fix" => "Find and fix the errors in the following code. Explain the fix in one line, then the code:",
        "explain" => "Explain what the following code does, briefly:",
        "comment" => "Add short comments to the following code and reply with the commented code:",
        "review" => "Review the following code: correctness, performance and Oracle pitfalls, in a short list:",
        "optimize" => "Rewrite the following code to be faster without changing its result:",
        other => return Err(format!("Unknown AI function: {other}")),
    };
    Ok((system, format!("{task}\n\n{code}")))
}

pub fn build_request(model: &str, system: &str, user: &str) -> Value {
    json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user },
        ],
        "temperature": 0.2,
    })
}

/// `choices[0].message.content`, or why there is none.
pub fn parse_response(json: &str) -> Result<String, String> {
    let value: Value = serde_json::from_str(json).map_err(|error| error.to_string())?;
    if let Some(message) = value.get("error").and_then(|error| error.get("message")) {
        return Err(format!("Provider error: {message}"));
    }
    value
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .map(|content| content.trim().to_string())
        .filter(|content| !content.is_empty())
        .ok_or_else(|| "The provider returned no content".to_string())
}

/// True for a loopback host, where plain HTTP is acceptable (the local mock and local runtimes).
pub fn is_loopback(base_url: &str) -> bool {
    let host = origin(base_url)
        .and_then(|value| value.split_once("://").map(|(_, host)| host.to_string()));
    matches!(host.as_deref(), Some("127.0.0.1" | "localhost" | "[::1]"))
}

/// Removes the credential from anything that will be shown or stored.
pub fn redact(message: &str, api_key: &str) -> String {
    if api_key.is_empty() {
        return message.to_string();
    }
    message.replace(api_key, "***")
}

/// One chat completion. Blocks up to `timeout_secs`; nothing is retried behind the user's back.
pub fn run(config: &AiConfig, api_key: &str, function: &str, code: &str) -> Result<String, String> {
    let (system, user) = instructions(function, code)?;
    if config
        .base_url
        .trim()
        .to_ascii_lowercase()
        .starts_with("http://")
        && !is_loopback(&config.base_url)
    {
        return Err(format!(
            "Refusing to send the API key over plain HTTP to {}. Use https:// (http is only allowed for localhost).",
            config.base_url
        ));
    }
    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));
    let agent = ureq::AgentBuilder::new()
        // A redirect would carry the Authorization header to whatever host it points at.
        .redirects(0)
        .timeout(std::time::Duration::from_secs(config.timeout_secs.max(5)))
        .build();
    let response = agent
        .post(&url)
        .set("Authorization", &format!("Bearer {api_key}"))
        .set("Content-Type", "application/json")
        .send_json(build_request(&config.model, &system, &user));
    match response {
        Ok(response) => {
            let body = response
                .into_string()
                .map_err(|error| redact(&error.to_string(), api_key))?;
            parse_response(&body)
                .map(|answer| redact(&answer, api_key))
                .map_err(|error| redact(&error, api_key))
        }
        Err(ureq::Error::Status(status, response)) => {
            let body = response.into_string().unwrap_or_default();
            let message = parse_response(&body).err().unwrap_or(body);
            Err(redact(&format!("HTTP {status}: {message}"), api_key))
        }
        Err(error) => Err(redact(&error.to_string(), api_key)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn every_function_has_its_instruction() {
        for function in [
            "create", "complete", "fix", "explain", "comment", "review", "optimize",
        ] {
            let (system, user) = instructions(function, "select 1 from dual").unwrap();
            assert!(system.contains("PL/SQL"));
            assert!(user.ends_with("select 1 from dual"));
        }
        assert!(instructions("nope", "x").is_err());
    }

    #[test]
    fn the_request_looks_like_a_chat_completion() {
        let request = build_request("m", "sys", "usr");
        assert_eq!(request["model"], "m");
        assert_eq!(request["messages"][0]["role"], "system");
        assert_eq!(request["messages"][1]["content"], "usr");
    }

    #[test]
    fn only_loopback_may_use_plain_http() {
        assert!(is_loopback("http://127.0.0.1:8080/v1"));
        assert!(is_loopback("http://localhost/v1"));
        assert!(!is_loopback("http://api.example.com/v1"));
        assert!(!is_loopback("https://api.example.com/v1"));
    }

    #[test]
    fn the_key_is_never_returned_in_an_error() {
        let error = "invalid header: Authorization: Bearer sk-secret-123";
        assert_eq!(
            redact(error, "sk-secret-123"),
            "invalid header: Authorization: Bearer ***"
        );
        assert_eq!(
            redact("nothing to hide", "sk-secret-123"),
            "nothing to hide"
        );
    }

    #[test]
    fn a_remote_plain_http_url_is_refused_before_sending() {
        let config = AiConfig {
            base_url: "http://api.example.com/v1".to_string(),
            model: "m".to_string(),
            timeout_secs: 5,
        };
        let error = run(&config, "sk-secret", "explain", "select 1").unwrap_err();
        assert!(error.contains("https"), "{error}");
        assert!(!error.contains("sk-secret"), "{error}");
    }

    #[test]
    fn the_answer_is_read_and_errors_are_reported() {
        let ok =
            r#"{"choices":[{"message":{"role":"assistant","content":"  select 2 from dual  "}}]}"#;
        assert_eq!(parse_response(ok).unwrap(), "select 2 from dual");
        let provider_error = r#"{"error":{"message":"bad key"}}"#;
        assert!(parse_response(provider_error)
            .unwrap_err()
            .contains("bad key"));
        assert!(parse_response(r#"{"choices":[]}"#).is_err());
        assert!(parse_response("not json").is_err());
    }

    #[test]
    fn config_round_trip_and_defaults() {
        let path = std::env::temp_dir().join(format!("sqlt-ai-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        assert_eq!(load_config(&path), AiConfig::default());
        let config = AiConfig {
            base_url: "http://127.0.0.1:9/v1".to_string(),
            model: "local".to_string(),
            timeout_secs: 5,
        };
        save_config(&path, &config).unwrap();
        assert_eq!(load_config(&path), config);
        let _ = std::fs::remove_file(path);
    }

    /// The whole client against a local mock: no quota, no key, and the request body is checked.
    #[test]
    fn a_local_mock_answers_a_completion() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // Read headers, then exactly Content-Length bytes: one read can stop mid-body.
            let mut request = String::new();
            let mut buffer = [0u8; 4096];
            let (header_end, length) = loop {
                let read = stream.read(&mut buffer).unwrap();
                if read == 0 {
                    break (request.len(), 0);
                }
                request.push_str(&String::from_utf8_lossy(&buffer[..read]));
                if let Some(end) = request.find("\r\n\r\n") {
                    let headers = &request[..end];
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.strip_prefix("content-length: ")
                                .or_else(|| line.strip_prefix("Content-Length: "))
                        })
                        .and_then(|value| value.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    break (end + 4, length);
                }
            };
            while request.len() < header_end + length {
                let read = stream.read(&mut buffer).unwrap();
                if read == 0 {
                    break;
                }
                request.push_str(&String::from_utf8_lossy(&buffer[..read]));
            }
            let body = r#"{"choices":[{"message":{"content":"select 42 from dual"}}]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).unwrap();
            request
        });

        let config = AiConfig {
            base_url: format!("http://{address}/v1"),
            model: "mock".to_string(),
            timeout_secs: 10,
        };
        let answer = run(&config, "test-key", "complete", "select 1").expect("answer");
        assert_eq!(answer, "select 42 from dual");
        let request = handle.join().unwrap();
        assert!(request.starts_with("POST /v1/chat/completions"));
        assert!(request.contains("Authorization: Bearer test-key"));
        assert!(request.contains("\"model\":\"mock\""));
        assert!(request.contains("select 1"));
    }

    #[test]
    fn userinfo_is_not_mistaken_for_the_host() {
        assert!(!is_loopback("http://localhost@evil.example.com/v1"));
        assert_eq!(
            origin("http://localhost@evil.example.com/v1").unwrap(),
            "http://evil.example.com"
        );
        assert!(is_loopback("http://[::1]:8080/v1"));
        assert_eq!(
            origin("HTTPS://API.Example.com/v1").unwrap(),
            "https://api.example.com"
        );
    }
}
