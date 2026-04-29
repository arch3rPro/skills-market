use anyhow::{bail, Result};
use reqwest::{
    header::{CONTENT_TYPE, ETAG},
    Method, RequestBuilder, StatusCode,
};
use std::time::Duration;

pub type WebDavAuth = Option<(String, Option<String>)>;
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const TRANSFER_TIMEOUT_SECS: u64 = 300;

pub fn parse_base_url(raw: &str) -> Result<reqwest::Url> {
    let url = reqwest::Url::parse(raw.trim())?;
    match url.scheme() {
        "http" | "https" if url.username().is_empty() && url.password().is_none() => Ok(url),
        "http" | "https" => {
            bail!(
                "WebDAV base URL must not include embedded credentials; configure them separately"
            )
        }
        _ => bail!("WebDAV base URL must use http or https"),
    }
}

pub fn path_segments(raw: &str) -> impl Iterator<Item = &str> {
    raw.trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
}

pub fn build_remote_url(base_url: &str, segments: &[String]) -> Result<String> {
    let mut url = parse_base_url(base_url)?;
    {
        let mut path = url
            .path_segments_mut()
            .map_err(|_| anyhow::anyhow!("WebDAV base URL cannot be used as a base"))?;
        path.pop_if_empty();
        for segment in segments {
            path.push(segment);
        }
    }
    Ok(url.to_string())
}

pub fn auth_from_credentials(username: &str, password: Option<&str>) -> WebDavAuth {
    let username = username.trim();
    if username.is_empty() {
        None
    } else {
        Some((username.to_string(), password.map(ToString::to_string)))
    }
}

pub async fn test_connection(base_url: &str, auth: WebDavAuth) -> Result<()> {
    let client = reqwest::Client::new();
    let method = Method::from_bytes(b"PROPFIND")?;
    let response = apply_auth(
        client
            .request(method, parse_base_url(base_url)?)
            .header("Depth", "0")
            .timeout(default_timeout()),
        &auth,
    )
    .send()
    .await?;

    let status = response.status();
    if status.is_success() || status.as_u16() == 207 {
        Ok(())
    } else {
        bail!("WebDAV connection test failed with status {status}");
    }
}

pub async fn ensure_remote_directories(
    base_url: &str,
    segments: &[String],
    auth: WebDavAuth,
) -> Result<()> {
    let client = reqwest::Client::new();
    let method = Method::from_bytes(b"MKCOL")?;
    let mut current_segments = Vec::with_capacity(segments.len());

    for segment in segments {
        current_segments.push(segment.clone());
        let url = build_remote_url(base_url, &current_segments)?;
        let response = apply_auth(
            client
                .request(method.clone(), url)
                .timeout(default_timeout()),
            &auth,
        )
        .send()
        .await?;
        let status = response.status();

        if mkcol_status_is_ok(status) {
            continue;
        }

        bail!("WebDAV directory creation failed with status {status}");
    }

    Ok(())
}

fn mkcol_status_is_ok(status: StatusCode) -> bool {
    status.is_success() || status == StatusCode::METHOD_NOT_ALLOWED
}

pub async fn put_bytes(
    url: &str,
    auth: WebDavAuth,
    bytes: Vec<u8>,
    content_type: &str,
) -> Result<()> {
    let client = reqwest::Client::new();
    let response = apply_auth(
        client
            .put(parse_base_url(url)?)
            .header(CONTENT_TYPE, content_type)
            .timeout(transfer_timeout())
            .body(bytes),
        &auth,
    )
    .send()
    .await?;

    let status = response.status();
    if status.is_success() {
        Ok(())
    } else {
        bail!("WebDAV upload failed with status {status}");
    }
}

pub async fn get_bytes(
    url: &str,
    auth: WebDavAuth,
    max_bytes: usize,
) -> Result<Option<(Vec<u8>, Option<String>)>> {
    let client = reqwest::Client::new();
    let mut response = apply_auth(
        client.get(parse_base_url(url)?).timeout(transfer_timeout()),
        &auth,
    )
    .send()
    .await?;
    let status = response.status();

    if status == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !status.is_success() {
        bail!("WebDAV download failed with status {status}");
    }
    if response
        .content_length()
        .is_some_and(|len| len > max_bytes as u64)
    {
        bail!("WebDAV download exceeds maximum size");
    }

    let etag = response
        .headers()
        .get(ETAG)
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string);
    let mut bytes = Vec::new();

    while let Some(chunk) = response.chunk().await? {
        if bytes.len() + chunk.len() > max_bytes {
            bail!("WebDAV download exceeds maximum size");
        }
        bytes.extend_from_slice(&chunk);
    }

    Ok(Some((bytes, etag)))
}

pub async fn head_etag(url: &str, auth: WebDavAuth) -> Result<Option<String>> {
    let client = reqwest::Client::new();
    let response = apply_auth(
        client.head(parse_base_url(url)?).timeout(default_timeout()),
        &auth,
    )
    .send()
    .await?;

    if !response.status().is_success() {
        return Ok(None);
    }

    Ok(response
        .headers()
        .get(ETAG)
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string))
}

fn apply_auth(builder: RequestBuilder, auth: &WebDavAuth) -> RequestBuilder {
    match auth {
        Some((username, password)) => builder.basic_auth(username, password.as_deref()),
        None => builder,
    }
}

fn default_timeout() -> Duration {
    Duration::from_secs(DEFAULT_TIMEOUT_SECS)
}

fn transfer_timeout() -> Duration {
    Duration::from_secs(TRANSFER_TIMEOUT_SECS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_remote_url_encodes_segments() {
        let segments = vec![
            "skills manager".to_string(),
            "v1".to_string(),
            "manifest.json".to_string(),
        ];

        let url = build_remote_url("https://dav.example.com/root/", &segments).unwrap();

        assert_eq!(
            url,
            "https://dav.example.com/root/skills%20manager/v1/manifest.json"
        );
    }

    #[test]
    fn parse_base_url_rejects_non_http() {
        let error = parse_base_url("file:///tmp/data").unwrap_err();

        assert!(error.to_string().contains("http"));
    }

    #[test]
    fn parse_base_url_rejects_embedded_credentials() {
        let error = parse_base_url("https://user:pass@example.com/path").unwrap_err();

        assert!(error.to_string().contains("credentials"));
    }

    #[test]
    fn mkcol_conflict_is_not_accepted_as_existing_directory() {
        assert!(mkcol_status_is_ok(StatusCode::CREATED));
        assert!(mkcol_status_is_ok(StatusCode::METHOD_NOT_ALLOWED));
        assert!(!mkcol_status_is_ok(StatusCode::CONFLICT));
    }

    #[test]
    fn path_segments_ignores_empty_parts() {
        let segments: Vec<_> = path_segments("/root//profile/").collect();

        assert_eq!(segments, vec!["root", "profile"]);
    }
}
