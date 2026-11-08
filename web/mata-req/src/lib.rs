//! HTTP(S) requests with a shared RFC 6265 cookie jar and Rustls-backed TLS.
//!
//! `mata-req` replaces the `ureq` client used by Mata Browser. Network I/O is
//! blocking: call it on a worker thread. TLS certificate/hostname verification
//! remains enabled. Compressed responses and redirects are handled by reqwest's
//! Rustls-backed transport. This crate never disables certificate validation.
use std::{
    io::{BufReader, Read, Write},
    sync::{Arc, Mutex},
    time::Duration,
};
use cookie_store::{CookieStore, RawCookie};
use reqwest::{blocking::Client, cookie::CookieStore as ReqwestCookieStore};
use url::Url;

pub use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
pub use reqwest::Method;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct Error(String);
impl std::fmt::Display for Error {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { fmt.write_str(&self.0) }
}
impl std::error::Error for Error {}
impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self { Self(error.to_string()) }
}
impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self { Self(error.to_string()) }
}

/// Cookie store shared by every clone of an [`Agent`]. Reqwest consults it
/// during each redirect, not just after the final response.
#[derive(Default)]
struct SharedCookies(Mutex<CookieStore>);
impl ReqwestCookieStore for SharedCookies {
    fn set_cookies(&self, cookie_headers: &mut dyn Iterator<Item = &HeaderValue>, url: &Url) {
        let cookies = cookie_headers.filter_map(|value| value.to_str().ok())
            .filter_map(|value| RawCookie::parse(value.to_owned()).ok())
            .map(|cookie| cookie.into_owned());
        if let Ok(mut jar) = self.0.lock() { jar.store_response_cookies(cookies, url); }
    }
    fn cookies(&self, url: &Url) -> Option<HeaderValue> {
        let jar = self.0.lock().ok()?;
        let text = jar.get_request_values(url)
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>().join("; ");
        if text.is_empty() { None } else { HeaderValue::from_str(&text).ok() }
    }
}

#[derive(Clone)]
pub struct Agent {
    client: Client,
    cookies: Arc<SharedCookies>,
    max_body: u64,
}
impl Agent {
    /// Construct an HTTPS-capable client with a 15-second timeout.
    pub fn new_with_defaults() -> Self { Self::config_builder().build() }
    pub fn config_builder() -> ConfigBuilder { ConfigBuilder::default() }
    pub fn post(&self, address: &str) -> RequestBuilder { self.request(Method::POST, address) }
    pub fn put(&self, address: &str) -> RequestBuilder { self.request(Method::PUT, address) }
    pub fn patch(&self, address: &str) -> RequestBuilder { self.request(Method::PATCH, address) }
    pub fn delete(&self, address: &str) -> RequestBuilder { self.request(Method::DELETE, address) }
    pub fn head(&self, address: &str) -> RequestBuilder { self.request(Method::HEAD, address) }
    pub fn get(&self, address: &str) -> RequestBuilder {
        RequestBuilder { client: self.clone(), inner: self.client.get(address) }
    }
    pub fn request(&self, method: Method, address: &str) -> RequestBuilder {
        RequestBuilder { client: self.clone(), inner: self.client.request(method, address) }
    }
    /// Load saved persistent cookies from JSON; only call with trusted local files.
    pub fn load_cookies(&self, reader: impl Read) -> Result<()> {
        let loaded = cookie_store::serde::json::load(BufReader::new(reader))
            .map_err(|e| Error(format!("Loading cookies: {e}")))?;
        *self.cookies.0.lock().map_err(|e| Error(e.to_string()))? = loaded;
        Ok(())
    }
    /// Save cookies; session cookies are omitted by the cookie_store serializer.
    pub fn save_cookies(&self, writer: impl Write) -> Result<()> {
        let guard = self.cookies.0.lock().map_err(|e| Error(e.to_string()))?;
        cookie_store::serde::json::save(&guard, writer)
            .map_err(|e| Error(format!("Saving cookies: {e}")))
    }
    /// Add a Set-Cookie style string associated with an HTTP(S) URL.
    pub fn add_cookie(&self, raw: &str, address: &Url) -> Result<()> {
        if !matches!(address.scheme(), "http" | "https") {
            return Err(Error("Cookies require an HTTP(S) URL".into()));
        }
        let cookie = RawCookie::parse(raw.to_owned()).map_err(|e| Error(e.to_string()))?.into_owned();
        self.cookies.0.lock().map_err(|e| Error(e.to_string()))?
            .store_response_cookies(std::iter::once(cookie), address);
        Ok(())
    }
    /// Cookie pairs sent on a request. Includes HttpOnly values: never expose
    /// this method's result to page JavaScript.
    pub fn cookie_header(&self, address: &Url) -> Option<String> {
        self.cookies.cookies(address)?.to_str().ok().map(str::to_owned)
    }
    /// Remove all network cookies, including session cookies.
    pub fn clear_cookies(&self) -> Result<()> {
        *self.cookies.0.lock().map_err(|e| Error(e.to_string()))? = CookieStore::default();
        Ok(())
    }
    /// Borrow the complete cookie store for read-only inspection.
    pub fn with_cookies<T>(&self, inspect: impl FnOnce(&CookieStore) -> T) -> Result<T> {
        let jar = self.cookies.0.lock().map_err(|e| Error(e.to_string()))?;
        Ok(inspect(&jar))
    }
    /// Mutate the cookie store. The closure must not make network requests.
    pub fn with_cookies_mut<T>(&self, edit: impl FnOnce(&mut CookieStore) -> T) -> Result<T> {
        let mut jar = self.cookies.0.lock().map_err(|e| Error(e.to_string()))?;
        Ok(edit(&mut jar))
    }
}

pub struct ConfigBuilder {
    timeout: Duration,
    redirects: usize,
    max_body: u64,
}
impl Default for ConfigBuilder {
    fn default() -> Self {
        Self { timeout: Duration::from_secs(15), redirects: 8, max_body: 32 * 1024 * 1024 }
    }
}
impl ConfigBuilder {
    pub fn timeout_global(mut self, value: Option<Duration>) -> Self {
        if let Some(timeout) = value { self.timeout = timeout; }
        self
    }
    pub fn max_redirects(mut self, value: usize) -> Self { self.redirects = value; self }
    pub fn max_response_bytes(mut self, value: u64) -> Self { self.max_body = value; self }
    pub fn try_build(self) -> Result<Agent> {
        let cookies = Arc::new(SharedCookies::default());
        let client = Client::builder()
            .use_rustls_tls()
            .timeout(self.timeout)
            .redirect(reqwest::redirect::Policy::limited(self.redirects))
            .cookie_provider(cookies.clone())
            .build()?;
        Ok(Agent { client, cookies, max_body: self.max_body })
    }
    /// Uses the operating system TLS trust store, falling back to the shipped
    /// root list. Panics only when the client cannot be initialized.
    pub fn build(self) -> Agent { self.try_build().expect("unable to initialize mata-req client") }
}

pub struct RequestBuilder { client: Agent, inner: reqwest::blocking::RequestBuilder }
impl RequestBuilder {
    pub fn header(mut self, key: &str, value: &str) -> Self {
        self.inner = self.inner.header(key, value);
        self
    }
    pub fn headers(mut self, headers: HeaderMap) -> Self {
        self.inner = self.inner.headers(headers); self
    }
    pub fn body(mut self, data: impl Into<Vec<u8>>) -> Self {
        self.inner = self.inner.body(data.into()); self
    }
    pub fn call(self) -> Result<Response> {
        let mut response = self.inner.send()?;
        let status = response.status();
        let address = response.url().to_owned();
        let headers = response.headers().clone();
        if !status.is_success() {
            return Err(Error(format!("HTTP {status} from {address}")));
        }
        let mut bytes = Vec::new();
        response.by_ref().take(self.client.max_body.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > self.client.max_body {
            return Err(Error(format!("Response body exceeded {} bytes", self.client.max_body)));
        }
        Ok(Response { status, address, headers, body: ResponseBody { bytes, max: u64::MAX } })
    }
}

pub struct Response {
    pub status: reqwest::StatusCode,
    address: Url,
    headers: HeaderMap,
    body: ResponseBody,
}
impl Response {
    pub fn url(&self) -> &Url { &self.address }
    pub fn headers(&self) -> &HeaderMap { &self.headers }
    pub fn body_mut(&mut self) -> &mut ResponseBody { &mut self.body }
}
pub struct ResponseBody { bytes: Vec<u8>, max: u64 }
impl ResponseBody {
    pub fn with_config(&mut self) -> &mut Self { self }
    pub fn limit(&mut self, limit: u64) -> &mut Self { self.max = limit; self }
    pub fn read_to_vec(&mut self) -> Result<Vec<u8>> {
        if self.bytes.len() as u64 > self.max {
            return Err(Error(format!("Response body exceeds {} bytes", self.max)));
        }
        Ok(std::mem::take(&mut self.bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn cookie_scope_and_http_only_request() {
        let agent = Agent::new_with_defaults();
        let a = Url::parse("https://example.org/a").unwrap();
        let b = Url::parse("https://unrelated.example/").unwrap();
        agent.add_cookie("auth=abc; Secure; HttpOnly; Path=/", &a).unwrap();
        assert_eq!(agent.cookie_header(&a).as_deref(), Some("auth=abc"));
        assert!(agent.cookie_header(&b).is_none());
    }
    #[test] fn configured_client() {
        let _ = Agent::config_builder().max_redirects(4)
            .timeout_global(Some(Duration::from_secs(3))).try_build().unwrap();
    }
}
