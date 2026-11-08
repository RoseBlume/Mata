# mata-req

Synchronous HTTP client for Mata, replacing the browser's direct `ureq` dependency.

The HTTPS transport uses **rustls** (via reqwest's Rustls backend, with default TLS backends disabled).
Provides certificate verification, HTTPS redirects, gzip/Brotli/deflate decoding,
connection reuse, a shared RFC 6265 cookie store and bounded response bodies.

```rust
use mata_req::Agent;
let agent = Agent::new_with_defaults();
let mut response = agent.get("https://example.com")
    .header("User-Agent", "Mata/0.1")
    .call()?;
let final_url = response.url().clone();
let html = response.body_mut().with_config().limit(4*1024*1024).read_to_vec()?;
```

`Agent` is `Clone`, sharing the HTTP connection pool and cookie jar. `cookie_header`
includes HttpOnly cookies and must not be exposed to page JavaScript. `Agent::save_cookies`
and `load_cookies` accept a writer and a reader respectively. Call this synchronous API
on a worker thread, not the UI rendering thread.

`mata-req` uses `reqwest` as the underlying HTTP/1.1 and HTTP/2 transport; it does **not**
implement a new TLS stack from scratch. No `ureq` dependency remains in `mata-browser`.
