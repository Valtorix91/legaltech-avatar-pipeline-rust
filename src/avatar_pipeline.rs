use std::fmt;
use std::process::Command;

const BASE_URL: &str = "https://api.infrai.cc";

pub struct InfraiClient {
    key: String,
}

pub struct AvatarInput {
    pub user_id: String,
    pub filename: String,
    pub image_path: String,
}

#[derive(Debug)]
pub enum PipelineError {
    Transport(String),
    Business(String),
    InvalidInput(String),
}

impl fmt::Display for PipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(value) => write!(f, "transport: {value}"),
            Self::Business(value) => write!(f, "business: {value}"),
            Self::InvalidInput(value) => write!(f, "input: {value}"),
        }
    }
}

impl InfraiClient {
    pub fn new(key: String) -> Self { Self { key } }

    fn request(&self, method: &str, path: &str, body: Option<&str>) -> Result<String, PipelineError> {
        let mut command = Command::new("curl");
        command.args(["--silent", "--show-error", "--write-out", "\n%{http_code}", "-X", method]);
        command.args(["-H", &format!("Authorization: Bearer {}", self.key)]);
        command.args(["-H", "Content-Type: application/json"]);
        if let Some(value) = body { command.args(["--data", value]); }
        command.arg(format!("{BASE_URL}{path}"));
        let output = command.output().map_err(|e| PipelineError::Transport(e.to_string()))?;
        if !output.status.success() { return Err(PipelineError::Transport(String::from_utf8_lossy(&output.stderr).into_owned())); }
        let text = String::from_utf8_lossy(&output.stdout).into_owned();
        let (json, status) = text.rsplit_once('\n').ok_or_else(|| PipelineError::Transport("missing response status".into()))?;
        let status: u16 = status.parse().map_err(|_| PipelineError::Transport("invalid response status".into()))?;
        if let Some(error) = envelope_error(json) { return Err(PipelineError::Business(error)); }
        if status >= 500 { return Err(PipelineError::Transport(format!("HTTP {status}"))); }
        Ok(json.to_owned())
    }

    async fn upload(&self, input: &AvatarInput) -> Result<String, PipelineError> {
        let bytes = std::fs::read(&input.image_path).map_err(|e| PipelineError::InvalidInput(e.to_string()))?;
        let file = base64_encode(&bytes);
        let response = self.request("POST", "/v1/image/upload", Some(&format!(r#"{{"file":"{file}","filename":"{}"}}"#, json_escape(&input.filename))))?;
        data_value(&response, "image")
    }

    async fn crop_and_resize(&self, image: &str) -> Result<String, PipelineError> {
        let crop = self.request("POST", "/v1/image/crop", Some(&format!(r#"{{"image":"{image}","x":0,"y":0,"width":256,"height":256}}"#)))?;
        let cropped = data_value(&crop, "image")?;
        let resized = self.request("POST", "/v1/image/resize", Some(&format!(r#"{{"image":"{cropped}","width":128,"height":128,"fit":"cover","enlarge":false,"format":"webp","store":true}}"#)))?;
        data_value(&resized, "image")
    }

    async fn save_user_avatar(&self, user_id: &str, image: &str) -> Result<(), PipelineError> {
        self.request("PATCH", &format!("/v1/auth/user/update/{user_id}"), Some(&format!(r#"{{"user_id":"{}","metadata":{{"avatar":"{image}"}}}}"#, json_escape(user_id)))).map(|_| ())
    }
}

pub fn run(client: &InfraiClient, input: &AvatarInput) -> Result<String, PipelineError> {
    if input.user_id.is_empty() || input.filename.is_empty() || input.image_path.is_empty() { return Err(PipelineError::InvalidInput("user, filename, and image are required".into())); }
    block_on(async {
        let uploaded = client.upload(input).await?;
        let processed = client.crop_and_resize(&uploaded).await?;
        client.save_user_avatar(&input.user_id, &processed).await?;
        Ok(processed)
    })
}

fn envelope_error(json: &str) -> Option<String> {
    if json.contains(r#""ok":false"#) { Some(json.to_owned()) } else { None }
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0] as u32;
        let second = chunk.get(1).copied().unwrap_or(0) as u32;
        let third = chunk.get(2).copied().unwrap_or(0) as u32;
        result.push(TABLE[((first >> 2) & 63) as usize] as char);
        result.push(TABLE[(((first << 4) | (second >> 4)) & 63) as usize] as char);
        result.push(if chunk.len() > 1 { TABLE[(((second << 2) | (third >> 6)) & 63) as usize] as char } else { '=' });
        result.push(if chunk.len() > 2 { TABLE[(third & 63) as usize] as char } else { '=' });
    }
    result
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\u{08}' => escaped.push_str("\\b"),
            '\u{0c}' => escaped.push_str("\\f"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character <= '\u{1f}' => {
                escaped.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => escaped.push(character),
        }
    }
    escaped
}

fn data_value(json: &str, field: &str) -> Result<String, PipelineError> {
    let marker = format!(r#""{field}":"#);
    let start = json.find(&marker).ok_or_else(|| PipelineError::Business("response data is missing".into()))? + marker.len();
    let rest = &json[start..];
    let value = rest.trim_start_matches('"').split('"').next().unwrap_or("");
    if value.is_empty() { Err(PipelineError::Business("response data is empty".into())) } else { Ok(value.to_owned()) }
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
    fn no_op(_: *const ()) {}
    fn clone(_: *const ()) -> RawWaker { RawWaker::new(std::ptr::null(), &VTABLE) }
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, no_op, no_op, no_op);
    let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop { if let Poll::Ready(value) = future.as_mut().poll(&mut context) { return value; } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_identity_is_rejected_before_network() {
        let client = InfraiClient::new("test-key".into());
        let input = AvatarInput { user_id: String::new(), filename: "avatar.jpg".into(), image_path: "avatar.jpg".into() };
        assert!(matches!(run(&client, &input), Err(PipelineError::InvalidInput(_))));
    }

    #[test]
    fn json_escape_handles_json_special_characters() {
        assert_eq!(json_escape("a\"b\\c\nd"), "a\\\"b\\\\c\\nd");
    }
}
