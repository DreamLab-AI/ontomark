//! [`Url`]: blocking HTTP(S) pages (feature `http`).

use crate::error::{Result, SourceError};
use crate::render::{render, MediaKind};
use crate::{Fetched, Source, SourceItem};
use std::io::Read;
use std::time::Duration;

/// Default body limit: 5 MiB.
const DEFAULT_MAX_BYTES: u64 = 5 * 1024 * 1024;

/// One or more web pages fetched with blocking GET requests.
///
/// The response `Content-Type` selects the renderer (Markdown, plain text or
/// HTML); anything else, or a missing header, is refused with
/// [`SourceError::UnsupportedMediaType`]. Only `http` and `https` URLs without
/// embedded credentials are accepted. No cookies are kept and no
/// authentication header is ever sent. Redirects are followed (at most five).
/// The body limit applies to the decoded body.
///
/// [`Source::list`] makes a best-effort `HEAD` request per URL to record the
/// `ETag` (else `Last-Modified`) as the change marker; if the server refuses
/// or the request fails the marker is empty, which [`crate::Ledger::can_skip`]
/// never treats as skippable. [`Source::fetch`] reports the real failure.
///
/// ```
/// use ontomark_source::Url;
/// let source = Url::new("https://example.org/page").and("https://example.org/other");
/// assert_eq!(source.urls.len(), 2);
/// ```
#[derive(Debug, Clone)]
pub struct Url {
    /// The pages, listed in this order.
    pub urls: Vec<String>,
    /// Timeout for each whole request.
    pub timeout: Duration,
    /// Bodies larger than this many bytes are refused with [`SourceError::TooLarge`].
    pub max_bytes: u64,
}

impl Url {
    /// A source over one URL, with a 20 second timeout and a 5 MiB body limit.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            urls: vec![url.into()],
            timeout: Duration::from_secs(20),
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }

    /// Add another URL.
    pub fn and(mut self, url: impl Into<String>) -> Self {
        self.urls.push(url.into());
        self
    }

    fn agent(&self) -> ureq::Agent {
        ureq::AgentBuilder::new()
            .timeout(self.timeout)
            .redirects(5)
            .user_agent(concat!("ontomark-source/", env!("CARGO_PKG_VERSION")))
            .build()
    }
}

fn check(url: &str) -> Result<()> {
    let bad = |reason: &str| SourceError::BadUrl {
        locator: url.to_string(),
        reason: reason.to_string(),
    };
    let rest = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .ok_or_else(|| bad("only http and https are accepted"))?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() {
        return Err(bad("no host"));
    }
    if authority.contains('@') {
        return Err(bad("credentials in the URL are not accepted"));
    }
    Ok(())
}

fn marker(resp: &ureq::Response) -> String {
    if let Some(etag) = resp.header("etag") {
        return format!("etag:{etag}");
    }
    if let Some(lm) = resp.header("last-modified") {
        return format!("last-modified:{lm}");
    }
    String::new()
}

impl Source for Url {
    fn describe(&self) -> String {
        format!("url source ({} pages)", self.urls.len())
    }

    fn list(&self) -> Result<Vec<SourceItem>> {
        let agent = self.agent();
        let mut out = Vec::with_capacity(self.urls.len());
        for url in &self.urls {
            check(url)?;
            let head = agent.head(url).call().ok();
            out.push(SourceItem {
                id: url.clone(),
                locator: url.clone(),
                change_marker: head.as_ref().map(marker).unwrap_or_default(),
                size: head
                    .as_ref()
                    .and_then(|r| r.header("content-length"))
                    .and_then(|v| v.parse().ok()),
            });
        }
        Ok(out)
    }

    fn fetch(&self, item: &SourceItem) -> Result<Fetched> {
        let url = &item.locator;
        check(url)?;
        let resp = self
            .agent()
            .get(url)
            .call()
            .map_err(|e| SourceError::Http {
                locator: url.clone(),
                message: e.to_string(),
            })?;
        let content_type = resp.header("content-type").unwrap_or("").to_string();
        let kind = MediaKind::from_content_type(&content_type).ok_or_else(|| {
            SourceError::UnsupportedMediaType {
                locator: url.clone(),
                media_type: content_type
                    .split(';')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string(),
            }
        })?;
        let too_large = || SourceError::TooLarge {
            locator: url.clone(),
            limit: self.max_bytes,
        };
        if resp
            .header("content-length")
            .and_then(|v| v.parse::<u64>().ok())
            .is_some_and(|n| n > self.max_bytes)
        {
            return Err(too_large());
        }
        let mut bytes = Vec::new();
        resp.into_reader()
            .take(self.max_bytes + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| SourceError::Http {
                locator: url.clone(),
                message: e.to_string(),
            })?;
        if bytes.len() as u64 > self.max_bytes {
            return Err(too_large());
        }
        let text = render(kind, &bytes, url)?;
        Ok(Fetched::new(&item.id, kind.media_type(), text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    /// Serve `n` connections: respond according to (method, path).
    fn serve(n: usize) -> (String, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let h = std::thread::spawn(move || {
            let mut seen = Vec::new();
            for _ in 0..n {
                let (mut s, _) = listener.accept().unwrap();
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut line = String::new();
                r.read_line(&mut line).unwrap();
                let mut headers = String::new();
                loop {
                    let mut l = String::new();
                    r.read_line(&mut l).unwrap();
                    if l.trim().is_empty() {
                        break;
                    }
                    headers.push_str(&l.to_ascii_lowercase());
                }
                seen.push(format!("{}|{}", line.trim(), headers));
                let mut parts = line.split_whitespace();
                let (method, path) = (parts.next().unwrap(), parts.next().unwrap());
                let (ctype, body, extra) = match path {
                    "/page" => (
                        "text/html; charset=utf-8",
                        "<h1>Hi</h1><p>x &amp; y</p><script>bad()</script>".to_string(),
                        "ETag: \"v1\"\r\n",
                    ),
                    "/md" => ("text/markdown", "# M\r\n".to_string(), ""),
                    "/pdf" => ("application/pdf", "%PDF".to_string(), ""),
                    "/big" => ("text/plain", "x".repeat(100), ""),
                    _ => ("text/plain", String::new(), ""),
                };
                let status = if path == "/missing" {
                    "404 Not Found"
                } else {
                    "200 OK"
                };
                let _ = write!(
                    s,
                    "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                if method != "HEAD" {
                    let _ = s.write_all(body.as_bytes());
                }
            }
            seen
        });
        (base, h)
    }

    #[test]
    fn lists_etag_and_fetches_html_as_markdown() {
        let (base, h) = serve(2);
        let src = Url::new(format!("{base}/page"));
        let items = src.list().unwrap();
        assert_eq!(items[0].change_marker, "etag:\"v1\"");
        let f = src.fetch(&items[0]).unwrap();
        assert_eq!(f.text, "# Hi\n\nx & y\n");
        assert_eq!(f.media_type, "text/html");
        let seen = h.join().unwrap();
        for s in &seen {
            assert!(!s.contains("cookie") && !s.contains("authorization"), "{s}");
        }
    }

    #[test]
    fn markdown_and_unsupported_and_limits() {
        let (base, h) = serve(4);
        let mut src = Url::new(format!("{base}/md"));
        let item = |p: &str| SourceItem {
            id: format!("{base}{p}"),
            locator: format!("{base}{p}"),
            change_marker: String::new(),
            size: None,
        };
        assert_eq!(src.fetch(&item("/md")).unwrap().text, "# M\n");
        assert!(matches!(
            src.fetch(&item("/pdf")),
            Err(SourceError::UnsupportedMediaType { media_type, .. }) if media_type == "application/pdf"
        ));
        src.max_bytes = 10;
        assert!(matches!(
            src.fetch(&item("/big")),
            Err(SourceError::TooLarge { .. })
        ));
        src.max_bytes = 1000;
        assert!(matches!(
            src.fetch(&item("/missing")),
            Err(SourceError::Http { .. })
        ));
        h.join().unwrap();
    }

    #[test]
    fn head_failure_gives_an_empty_marker() {
        let src = Url::new("http://127.0.0.1:1/never");
        let items = src.list().unwrap();
        assert_eq!(items[0].change_marker, "");
        assert!(matches!(
            src.fetch(&items[0]),
            Err(SourceError::Http { .. })
        ));
    }

    #[test]
    fn unsafe_urls_are_refused_before_any_request() {
        for u in [
            "file:///etc/passwd",
            "ftp://x/y",
            "http://u:p@host/x",
            "http:///x",
        ] {
            assert!(
                matches!(Url::new(u).list(), Err(SourceError::BadUrl { .. })),
                "{u}"
            );
        }
    }
}
