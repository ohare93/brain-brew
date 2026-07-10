//! Typed, injectable resource policy for remote package fetches and archives.
//!
//! Production network requests are HTTPS-only. Tests may opt into the private
//! local-HTTP adapter; lock source YAML can never select that adapter.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tempfile::NamedTempFile;
use url::Url;

const USER_AGENT: &str = concat!("brainbrew/", env!("CARGO_PKG_VERSION"));

/// One conservative policy shared by GitHub API and archive source adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FetchPolicy {
    pub(crate) connect_timeout: Duration,
    pub(crate) read_timeout: Duration,
    pub(crate) total_timeout: Duration,
    pub(crate) max_redirects: u32,
    pub(crate) max_download_bytes: u64,
    pub(crate) max_json_bytes: u64,
    pub(crate) max_decompressed_tar_bytes: u64,
    pub(crate) max_regular_file_bytes: u64,
    pub(crate) max_archive_entries: u64,
    pub(crate) max_expanded_regular_bytes: u64,
    pub(crate) max_archive_path_bytes: usize,
    pub(crate) max_archive_path_depth: usize,
    pub(crate) max_archive_metadata_bytes: u64,
    pub(crate) max_expansion_ratio: u64,
}

impl Default for FetchPolicy {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            read_timeout: Duration::from_secs(30),
            total_timeout: Duration::from_secs(120),
            max_redirects: 5,
            max_download_bytes: 64 * 1024 * 1024,
            max_json_bytes: 1024 * 1024,
            max_decompressed_tar_bytes: 512 * 1024 * 1024,
            max_regular_file_bytes: 64 * 1024 * 1024,
            max_archive_entries: 20_000,
            max_expanded_regular_bytes: 256 * 1024 * 1024,
            max_archive_path_bytes: 1024,
            max_archive_path_depth: 32,
            max_archive_metadata_bytes: 64 * 1024,
            max_expansion_ratio: 200,
        }
    }
}

#[derive(Debug)]
pub(crate) struct DownloadedFile {
    file: NamedTempFile,
    pub(crate) bytes: u64,
    pub(crate) started: Instant,
}

impl DownloadedFile {
    pub(crate) fn path(&self) -> &Path {
        self.file.path()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TransportMode {
    HttpsOnly,
    #[cfg(test)]
    LocalHttp,
}

pub(crate) fn fetch_to_temp(
    source: &str,
    policy: &FetchPolicy,
    max_bytes: u64,
    accept: Option<&str>,
) -> Result<DownloadedFile, String> {
    fetch_to_temp_with_mode(source, policy, max_bytes, accept, TransportMode::HttpsOnly)
}

fn fetch_to_temp_with_mode(
    source: &str,
    policy: &FetchPolicy,
    max_bytes: u64,
    accept: Option<&str>,
    mode: TransportMode,
) -> Result<DownloadedFile, String> {
    let started = Instant::now();
    if let Some(path) = local_source_path(source)? {
        return copy_local_to_temp(source, &path, policy, started, max_bytes);
    }

    let mut url = Url::parse(source)
        .map_err(|error| format!("package source {source:?}: invalid URL: {error}"))?;
    validate_network_url(source, &url, mode)?;
    let mut redirects = 0_u32;

    loop {
        let elapsed = started.elapsed();
        let remaining = policy.total_timeout.checked_sub(elapsed).ok_or_else(|| {
            budget_error(
                source,
                "total_timeout",
                format_duration(elapsed),
                format_duration(policy.total_timeout),
            )
        })?;
        let agent = ureq::builder()
            .redirects(0)
            .redirect_auth_headers(ureq::RedirectAuthHeaders::Never)
            .timeout_connect(policy.connect_timeout.min(remaining))
            .timeout_read(policy.read_timeout.min(remaining))
            .timeout_write(remaining)
            .timeout(remaining)
            .user_agent(USER_AGENT)
            .build();
        let mut request = agent.get(url.as_str()).set("Accept-Encoding", "identity");
        if let Some(accept) = accept {
            request = request.set("Accept", accept);
        }
        let response = match request.call() {
            Ok(response) => response,
            Err(ureq::Error::Status(_, response)) => response,
            Err(error) => {
                let elapsed = started.elapsed();
                if error.to_string().to_ascii_lowercase().contains("timed out") {
                    let (budget, limit) = if elapsed >= policy.total_timeout {
                        ("total_timeout", policy.total_timeout)
                    } else if elapsed < policy.read_timeout {
                        ("connect_timeout", policy.connect_timeout)
                    } else {
                        ("read_timeout", policy.read_timeout)
                    };
                    return Err(format!(
                        "{}; transport error: {error}",
                        budget_error(
                            source,
                            budget,
                            format_duration(elapsed),
                            format_duration(limit)
                        )
                    ));
                }
                return Err(format!(
                    "package source {source:?}: transport error: {error}"
                ));
            }
        };

        if matches!(response.status(), 301 | 302 | 303 | 307 | 308) {
            redirects += 1;
            if redirects > policy.max_redirects {
                return Err(budget_error(
                    source,
                    "redirect_count",
                    redirects,
                    policy.max_redirects,
                ));
            }
            let location = response.header("Location").ok_or_else(|| {
                format!(
                    "package source {source:?}: redirect {} from {} has no Location header",
                    response.status(),
                    url
                )
            })?;
            let next = url.join(location).map_err(|error| {
                format!(
                    "package source {source:?}: invalid redirect Location {location:?}: {error}"
                )
            })?;
            validate_network_url(source, &next, mode)?;
            // No credentials are accepted in URLs, and RedirectAuthHeaders::Never
            // strips all credential headers even for same-host redirects. HTTPS
            // cross-host redirects are intentionally permitted for CDNs.
            url = next;
            continue;
        }

        if !(200..300).contains(&response.status()) {
            return Err(format!(
                "package source {source:?}: fetch of {url} returned HTTP {} {}",
                response.status(),
                response.status_text()
            ));
        }
        validate_network_url(
            source,
            &Url::parse(response.get_url()).map_err(|error| {
                format!("package source {source:?}: invalid final response URL: {error}")
            })?,
            mode,
        )?;
        check_content_length(source, response.header("Content-Length"), max_bytes)?;
        return stream_response_to_temp(source, response.into_reader(), policy, started, max_bytes);
    }
}

fn validate_network_url(source: &str, url: &Url, mode: TransportMode) -> Result<(), String> {
    let allow_http = match mode {
        TransportMode::HttpsOnly => false,
        #[cfg(test)]
        TransportMode::LocalHttp => true,
    };
    let allowed = url.scheme() == "https" || (url.scheme() == "http" && allow_http);
    if !allowed {
        return Err(format!(
            "package source {source:?}: transport_scheme budget rejected scheme {:?}; current={}, limit=https",
            url.scheme(),
            url.scheme()
        ));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(format!(
            "package source {source:?}: URL credentials are forbidden and are never forwarded across redirects"
        ));
    }
    Ok(())
}

fn check_content_length(source: &str, value: Option<&str>, max_bytes: u64) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    let length = value.parse::<u64>().map_err(|_| {
        format!("package source {source:?}: invalid Content-Length header {value:?}")
    })?;
    if length > max_bytes {
        return Err(budget_error(
            source,
            "compressed_download_bytes",
            length,
            max_bytes,
        ));
    }
    Ok(())
}

fn stream_response_to_temp(
    source: &str,
    mut reader: impl Read,
    policy: &FetchPolicy,
    started: Instant,
    max_bytes: u64,
) -> Result<DownloadedFile, String> {
    let mut file =
        NamedTempFile::new().map_err(|error| format!("package source {source:?}: {error}"))?;
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        check_total_deadline(source, policy, started)?;
        let read_started = Instant::now();
        let count = reader.read(&mut buffer).map_err(|error| {
            let total_elapsed = started.elapsed();
            let (budget, current, limit) = if total_elapsed >= policy.total_timeout {
                (
                    "total_timeout",
                    format_duration(total_elapsed),
                    format_duration(policy.total_timeout),
                )
            } else {
                (
                    "read_timeout",
                    format_duration(read_started.elapsed()),
                    format_duration(policy.read_timeout),
                )
            };
            budget_error(source, budget, current, limit) + &format!("; read error: {error}")
        })?;
        let read_elapsed = read_started.elapsed();
        if read_elapsed > policy.read_timeout {
            return Err(budget_error(
                source,
                "read_timeout",
                format_duration(read_elapsed),
                format_duration(policy.read_timeout),
            ));
        }
        if count == 0 {
            break;
        }
        total = total.saturating_add(count as u64);
        if total > max_bytes {
            return Err(budget_error(
                source,
                "compressed_download_bytes",
                total,
                max_bytes,
            ));
        }
        file.write_all(&buffer[..count]).map_err(|error| {
            format!("package source {source:?}: temporary download write failed: {error}")
        })?;
        check_total_deadline(source, policy, started)?;
    }
    file.as_file_mut().sync_all().map_err(|error| {
        format!("package source {source:?}: temporary download sync failed: {error}")
    })?;
    Ok(DownloadedFile {
        file,
        bytes: total,
        started,
    })
}

fn copy_local_to_temp(
    source: &str,
    path: &Path,
    policy: &FetchPolicy,
    started: Instant,
    max_bytes: u64,
) -> Result<DownloadedFile, String> {
    let metadata =
        fs::metadata(path).map_err(|error| format!("package source {source:?}: {error}"))?;
    if metadata.len() > max_bytes {
        return Err(budget_error(
            source,
            "compressed_download_bytes",
            metadata.len(),
            max_bytes,
        ));
    }
    let input =
        fs::File::open(path).map_err(|error| format!("package source {source:?}: {error}"))?;
    stream_response_to_temp(source, input, policy, started, max_bytes)
}

fn local_source_path(source: &str) -> Result<Option<PathBuf>, String> {
    if source.starts_with("file://") {
        let url = Url::parse(source)
            .map_err(|error| format!("package source {source:?}: invalid file URL: {error}"))?;
        return url
            .to_file_path()
            .map(Some)
            .map_err(|()| format!("package source {source:?}: file URL is not a local path"));
    }
    let path = Path::new(source);
    Ok(path.exists().then(|| path.to_path_buf()))
}

pub(crate) fn check_total_deadline(
    source: &str,
    policy: &FetchPolicy,
    started: Instant,
) -> Result<(), String> {
    let elapsed = started.elapsed();
    if elapsed > policy.total_timeout {
        Err(budget_error(
            source,
            "total_timeout",
            format_duration(elapsed),
            format_duration(policy.total_timeout),
        ))
    } else {
        Ok(())
    }
}

pub(crate) fn budget_error(
    source: &str,
    budget: &str,
    current: impl std::fmt::Display,
    limit: impl std::fmt::Display,
) -> String {
    format!(
        "package source {source:?}: {budget} budget exhausted: current={current}, limit={limit}"
    )
}

fn format_duration(duration: Duration) -> String {
    format!("{}ms", duration.as_millis())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn documented_defaults_cover_connect_read_total_and_resource_budgets() {
        let policy = FetchPolicy::default();
        assert_eq!(policy.connect_timeout, Duration::from_secs(10));
        assert_eq!(policy.read_timeout, Duration::from_secs(30));
        assert_eq!(policy.total_timeout, Duration::from_secs(120));
        assert_eq!(policy.max_redirects, 5);
        assert_eq!(policy.max_download_bytes, 64 * 1024 * 1024);
        assert_eq!(policy.max_archive_entries, 20_000);
        assert_eq!(policy.max_expansion_ratio, 200);
    }

    #[test]
    fn production_transport_rejects_http_and_url_credentials() {
        let policy = FetchPolicy::default();
        let error = fetch_to_temp("http://example.test/archive.tar", &policy, 100, None)
            .expect_err("HTTP is rejected before connecting");
        assert!(error.contains("transport_scheme"), "{error}");
        let error = fetch_to_temp(
            "https://user:secret@example.test/archive.tar",
            &policy,
            100,
            None,
        )
        .expect_err("credentials are rejected before connecting");
        assert!(error.contains("credentials are forbidden"), "{error}");
    }

    #[test]
    fn local_http_adapter_bounds_redirects_and_rejects_downgrade() {
        let (base, server_handle) = server(vec![
            "HTTP/1.1 302 Found\r\nLocation: /again\r\nContent-Length: 0\r\n\r\n".into(),
            "HTTP/1.1 302 Found\r\nLocation: /again\r\nContent-Length: 0\r\n\r\n".into(),
        ]);
        let policy = FetchPolicy {
            max_redirects: 1,
            ..FetchPolicy::default()
        };
        let error = fetch_to_temp_with_mode(&base, &policy, 100, None, TransportMode::LocalHttp)
            .expect_err("redirect limit is enforced");
        assert!(error.contains("redirect_count"), "{error}");
        server_handle.join().unwrap();

        let (base, server_handle) = server(vec![
            "HTTP/1.1 302 Found\r\nLocation: ftp://example.test/file\r\nContent-Length: 0\r\n\r\n"
                .into(),
        ]);
        let error = fetch_to_temp_with_mode(
            &base,
            &FetchPolicy::default(),
            100,
            None,
            TransportMode::LocalHttp,
        )
        .expect_err("unsupported redirect scheme is rejected");
        assert!(error.contains("transport_scheme"), "{error}");
        server_handle.join().unwrap();
    }

    #[test]
    fn local_http_adapter_enforces_read_and_total_deadlines() {
        let (base, server_handle) = delayed_body_server(Duration::from_millis(80), 1, 1);
        let policy = FetchPolicy {
            read_timeout: Duration::from_millis(20),
            total_timeout: Duration::from_secs(1),
            ..FetchPolicy::default()
        };
        let error = fetch_to_temp_with_mode(&base, &policy, 100, None, TransportMode::LocalHttp)
            .expect_err("slow body exceeds per-read timeout");
        assert!(error.contains("read_timeout"), "{error}");
        server_handle.join().unwrap();

        let (base, server_handle) = delayed_body_server(Duration::from_millis(15), 3, 1);
        let policy = FetchPolicy {
            read_timeout: Duration::from_millis(100),
            total_timeout: Duration::from_millis(25),
            ..FetchPolicy::default()
        };
        let error = fetch_to_temp_with_mode(&base, &policy, 100, None, TransportMode::LocalHttp)
            .expect_err("stream exceeds monotonic total deadline");
        assert!(error.contains("total_timeout"), "{error}");
        server_handle.join().unwrap();
    }

    #[test]
    fn local_http_adapter_allows_bounded_cross_host_redirect_without_credentials() {
        let (target, target_handle) = server(vec![
            "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".into(),
        ]);
        let target = target.replace("127.0.0.1", "localhost");
        let (source, redirect_handle) = server(vec![format!(
            "HTTP/1.1 302 Found\r\nLocation: {target}\r\nContent-Length: 0\r\n\r\n"
        )]);
        let download = fetch_to_temp_with_mode(
            &source,
            &FetchPolicy::default(),
            100,
            None,
            TransportMode::LocalHttp,
        )
        .expect("cross-host CDN-style redirect is allowed by policy");
        assert_eq!(fs::read(download.path()).unwrap(), b"ok");
        redirect_handle.join().unwrap();
        target_handle.join().unwrap();
    }

    #[test]
    fn local_http_adapter_rejects_length_and_streaming_overflow() {
        let (base, server_handle) = server(vec![
            "HTTP/1.1 200 OK\r\nContent-Length: 101\r\n\r\n".into(),
        ]);
        let error = fetch_to_temp_with_mode(
            &base,
            &FetchPolicy::default(),
            100,
            None,
            TransportMode::LocalHttp,
        )
        .expect_err("large declared length is rejected");
        assert!(error.contains("current=101, limit=100"), "{error}");
        server_handle.join().unwrap();

        let (base, server_handle) = server(vec![
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n40\r\naaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\r\n40\r\nbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\r\n0\r\n\r\n".into(),
        ]);
        let error = fetch_to_temp_with_mode(
            &base,
            &FetchPolicy::default(),
            100,
            None,
            TransportMode::LocalHttp,
        )
        .expect_err("chunked body is bounded while streaming");
        assert!(error.contains("compressed_download_bytes"), "{error}");
        server_handle.join().unwrap();
    }

    fn delayed_body_server(
        delay: Duration,
        chunks: usize,
        chunk_bytes: usize,
    ) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = BufReader::new(stream.try_clone().unwrap());
            loop {
                let mut line = String::new();
                request.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
            }
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                chunks * chunk_bytes
            )
            .unwrap();
            stream.flush().unwrap();
            for _ in 0..chunks {
                thread::sleep(delay);
                if stream.write_all(&vec![b'x'; chunk_bytes]).is_err() {
                    break;
                }
                let _ = stream.flush();
            }
        });
        (format!("http://{address}/slow"), handle)
    }

    fn server(responses: Vec<String>) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = BufReader::new(stream.try_clone().unwrap());
                loop {
                    let mut line = String::new();
                    request.read_line(&mut line).unwrap();
                    if line == "\r\n" || line.is_empty() {
                        break;
                    }
                }
                stream.write_all(response.as_bytes()).unwrap();
            }
        });
        (format!("http://{address}/start"), handle)
    }
}
