use crate::config::AppConfig;
use crate::network::NetworkManager;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Duration;
use tokio::time::timeout;
use url::Url;

pub struct ShuConnect {
    config: AppConfig,
    portal_candidates: Mutex<Vec<String>>,
    client: reqwest::Client,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_http::{response, Server};

    fn config(base: &str) -> AppConfig {
        AppConfig {
            auth_server: "127.0.0.1".into(),
            auth_port: Url::parse(base).unwrap().port().unwrap().to_string(),
            user_id: "test-student".into(),
            password: "a+b& c".into(),
            ..AppConfig::default()
        }
    }

    #[tokio::test]
    async fn captures_portal_variants_and_submits_encoded_credentials() {
        let portal = Server::new(|request| {
            if request.starts_with("POST /eportal/InterFace.do?method=login ") {
                response("200 OK", "", r#"{"result":"success"}"#)
            } else {
                response("200 OK", "", r#"<script>location.href = "/eportal/index.jsp?wlanuserip=10.1.2.3&amp;wlanacname=SHU";</script>"#)
            }
        }).await;
        let auth = ShuConnect::new(config(&portal.base));
        let query = auth
            .catch_data_from_urls(vec![portal.base.clone()])
            .await
            .unwrap();
        assert_eq!(query, "wlanuserip=10.1.2.3&wlanacname=SHU");
        auth.login(&query).await.unwrap();
        let requests = portal.requests.lock().unwrap();
        let post = requests.iter().find(|r| r.starts_with("POST ")).unwrap();
        let fields: std::collections::HashMap<_, _> =
            url::form_urlencoded::parse(post.split_once("\r\n\r\n").unwrap().1.as_bytes())
                .into_owned()
                .collect();
        assert_eq!(fields["password"], "a+b& c");
        assert_eq!(fields["queryString"], query);
        assert_eq!(fields["userId"], "test-student");
    }

    #[tokio::test]
    async fn wrong_password_is_not_retried_on_other_endpoints() {
        let portal = Server::new(|_| {
            response(
                "200 OK",
                "",
                r#"{"result":"fail","message":"Invalid password"}"#,
            )
        })
        .await;
        let auth = ShuConnect::new(config(&portal.base));
        assert!(auth
            .login("wlanuserip=10.1.2.3")
            .await
            .unwrap_err()
            .contains("Invalid password"));
        assert_eq!(portal.requests.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn campus_gateway_requires_title_case_host_header() {
        // The real campus gateway closes requests with `host:` before sending
        // an HTTP response, while `Host:` returns the captive-portal script.
        let portal = Server::new(|request| {
            if !request.contains("\r\nHost: ") {
                return String::new();
            }
            if request.starts_with("POST ") {
                response("200 OK", "", r#"{"result":"success"}"#)
            } else {
                response("200 OK", "", "<script>top.self.location.href='http://127.0.0.1/eportal/index.jsp?wlanuserip=10.1.2.3&wlanacname=SHU'</script>")
            }
        }).await;
        assert!(reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .get(&portal.base)
            .send()
            .await
            .is_err());
        let auth = ShuConnect::new(config(&portal.base));
        let query = auth
            .catch_data_from_urls(vec![portal.base.clone()])
            .await
            .unwrap();
        assert!(query.starts_with("wlanuserip="));
        auth.login(&query).await.unwrap();
        assert!(portal
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|r| r.starts_with("POST ") && r.contains("\r\nHost: ")));
    }

    #[tokio::test]
    async fn an_error_page_on_one_port_does_not_hide_an_online_portal() {
        let error_page =
            Server::new(|_| response("200 OK", "", "<center>404 error</center>")).await;
        let portal = Server::new(|request| {
            if request.starts_with("GET /eportal/success.jsp ") {
                response("200 OK", "", "Online")
            } else {
                response("302 Found", "Location: /eportal/success.jsp\r\n", "")
            }
        })
        .await;
        let auth = ShuConnect::new(config(&error_page.base));
        assert_eq!(
            auth.check_connect_from_bases(vec![error_page.base.clone(), portal.base.clone()])
                .await,
            1
        );
        assert!(auth.candidate_login_urls()[0].starts_with(&portal.base));
    }

    #[tokio::test]
    async fn login_post_cannot_redirect_credentials_to_another_origin() {
        let foreign = Server::new(|_| response("200 OK", "", r#"{"result":"success"}"#)).await;
        let target = foreign.base.clone();
        let portal = Server::new(move |_| {
            response(
                "307 Temporary Redirect",
                &format!("Location: {}/capture\r\n", target),
                "",
            )
        })
        .await;
        let auth = ShuConnect::new(config(&portal.base));
        // Inspect the POST directly to avoid production port fallbacks in this test.
        let result = auth
            .client
            .post(&portal.base)
            .body("password=test-secret")
            .send()
            .await
            .unwrap();
        assert_eq!(result.status().as_u16(), 307);
        assert!(foreign.requests.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn portal_detection_does_not_wait_for_a_stalled_probe() {
        let stalled = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let portal =
            Server::new(|_| response("200 OK", "", "<script>location='index.jsp?a=1';</script>"))
                .await;
        let auth = ShuConnect::new(config(&portal.base));
        let urls = vec![
            format!("http://{}/", stalled.local_addr().unwrap()),
            portal.base.clone(),
        ];
        let result = timeout(Duration::from_secs(1), auth.catch_data_from_urls(urls))
            .await
            .unwrap();
        assert_eq!(result.as_deref(), Some("a=1"));
    }

    #[tokio::test]
    async fn legacy_parser_handles_invalid_offsets_without_panicking() {
        let mut cfg = config("http://127.0.0.1:12345");
        cfg.parse_start_token = "CUSTOM".into();
        cfg.parse_start_offset = usize::MAX;
        let auth = ShuConnect::new(cfg);
        assert_eq!(auth.extract_query_string_from_body("xCUSTOMdata"), None);
        assert!(!auth.is_auth_host("http://127.0.0.1.attacker.example/eportal/"));
    }
}

impl ShuConnect {
    pub fn new(config: AppConfig) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            ),
        );

        let client = reqwest::Client::builder()
            .no_proxy()
            // The campus gateway drops lowercase `host:` requests without an
            // HTTP response. Keep the browser-compatible header spelling.
            .http1_title_case_headers()
            .default_headers(headers)
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                // Never forward a login POST (307/308) to another origin.
                if attempt.previous().len() >= 10 {
                    attempt.stop()
                } else if matches!(attempt.status().as_u16(), 307 | 308)
                    && attempt
                        .previous()
                        .last()
                        .is_some_and(|previous| previous.origin() != attempt.url().origin())
                {
                    attempt.stop()
                } else {
                    attempt.follow()
                }
            }))
            .connect_timeout(Duration::from_secs(2))
            .danger_accept_invalid_certs(true)
            .timeout(Duration::from_secs(8))
            .build()
            .unwrap_or_default();

        let initial_base = Self::normalize_base_url(&format!(
            "http://{}:{}",
            config.auth_server,
            config.auth_port.trim()
        ));

        let portal_candidates = Mutex::new(if let Some(base) = initial_base {
            vec![base]
        } else {
            Vec::new()
        });

        Self {
            config,
            portal_candidates,
            client,
        }
    }

    fn normalize_base_url(raw: &str) -> Option<String> {
        let parsed = Url::parse(raw).ok()?;
        let host = parsed.host_str()?;
        let scheme = parsed.scheme();
        if let Some(port) = parsed.port() {
            Some(format!("{}://{}:{}", scheme, host, port))
        } else {
            Some(format!("{}://{}", scheme, host))
        }
    }

    /// Returns true only if the URL points at the configured authentication server.
    /// Credentials must never be sent to hosts learned from redirects or page content,
    /// otherwise a foreign captive portal could harvest them.
    fn is_auth_host(&self, url_str: &str) -> bool {
        Url::parse(url_str)
            .ok()
            .and_then(|u| {
                u.host_str()
                    .map(|h| h.eq_ignore_ascii_case(self.config.auth_server.trim()))
            })
            .unwrap_or(false)
    }

    fn remember_base_url(&self, url_str: &str) {
        if !self.is_auth_host(url_str) {
            return;
        }
        if let Some(base) = Self::normalize_base_url(url_str) {
            let mut list = self.portal_candidates.lock().unwrap();
            if !list.contains(&base) {
                list.insert(0, base);
            }
        }
    }

    fn capture_candidates_from_response(&self, final_url: &str, body: &str) {
        self.remember_base_url(final_url);

        if let Ok(re) = Regex::new(r"(?i)https?://[^\s\x22'<>]+/eportal/") {
            for cap in re.find_iter(body) {
                self.remember_base_url(cap.as_str());
            }
        }
    }

    fn extract_query_string_from_body(&self, body: &str) -> Option<String> {
        // Portal firmware varies in quote style and whitespace. Parse the URL
        // first, keeping the configurable legacy parser as a fallback.
        if let Ok(re) = Regex::new(r#"(?i)index\.jsp\?([^\s\x22'<>]+)"#) {
            if let Some(captures) = re.captures(body) {
                return Some(captures[1].replace("&amp;", "&"));
            }
        }
        let token = &self.config.parse_start_token;
        if token.is_empty() {
            return None;
        }

        let start_idx = body.find(token)?;
        let offset = if self.config.parse_start_offset == 0 {
            token.len()
        } else {
            self.config.parse_start_offset
        };

        let content_start = start_idx.checked_add(offset)?;
        if content_start >= body.len() {
            return None;
        }

        // `get` avoids a panic when a user-supplied offset lands inside a multi-byte character
        let remaining = body.get(content_start..)?;
        let end_token = &self.config.parse_end_token;

        let query = if !end_token.is_empty() {
            if let Some(end_idx) = remaining.find(end_token) {
                remaining[..end_idx].trim()
            } else {
                return None;
            }
        } else {
            remaining.trim()
        };

        if !query.is_empty() {
            Some(query.to_string())
        } else {
            None
        }
    }

    fn candidate_probe_urls(&self) -> Vec<String> {
        let mut seen = HashSet::new();
        let list = vec![
            self.config.test_url.clone(),
            "http://1.1.1.1/".to_string(),
            format!("http://{}/", self.config.auth_server),
            format!(
                "http://{}:{}/",
                self.config.auth_server,
                self.config.auth_port.trim()
            ),
            format!("http://{}:8080/", self.config.auth_server),
            format!("http://{}:80/", self.config.auth_server),
        ];

        let mut result = Vec::new();
        for url in list {
            if !url.is_empty() && seen.insert(url.clone()) {
                result.push(url);
            }
        }
        result
    }

    fn candidate_base_urls(&self) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut result = Vec::new();

        {
            let list = self.portal_candidates.lock().unwrap();
            for item in list.iter() {
                if seen.insert(item.clone()) {
                    result.push(item.clone());
                }
            }
        }

        let defaults = vec![
            Self::normalize_base_url(&format!(
                "http://{}:{}",
                self.config.auth_server,
                self.config.auth_port.trim()
            )),
            Self::normalize_base_url(&format!("http://{}:8080", self.config.auth_server)),
            Self::normalize_base_url(&format!("http://{}:80", self.config.auth_server)),
            Self::normalize_base_url(&format!("http://{}", self.config.auth_server)),
        ];

        for opt in defaults.into_iter().flatten() {
            if seen.insert(opt.clone()) {
                result.push(opt);
            }
        }

        result
    }

    fn candidate_login_urls(&self) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut candidates = Vec::new();

        for base in self.candidate_base_urls() {
            let login_url = format!(
                "{}/eportal/InterFace.do?method=login",
                base.trim_end_matches('/')
            );
            if seen.insert(login_url.clone()) {
                candidates.push(login_url);
            }
        }

        candidates
    }

    /// Checks reachability of auth server:
    /// 1 = Already online
    /// 2 = Offline / Needs authentication
    /// 3 = Unreachable
    pub async fn check_connect(&self) -> u8 {
        self.check_connect_from_bases(self.candidate_base_urls())
            .await
    }

    async fn check_connect_from_bases(&self, bases: Vec<String>) -> u8 {
        let mut probes = tokio::task::JoinSet::new();
        for base in bases {
            let client = self.client.clone();
            probes.spawn(async move {
                client
                    .get(base)
                    .timeout(Duration::from_secs(4))
                    .send()
                    .await
            });
        }
        let mut reachable = false;
        while let Some(result) = probes.join_next().await {
            if let Ok(Ok(resp)) = result {
                reachable = true;
                let final_url = resp.url();
                if self.is_auth_host(final_url.as_str()) {
                    self.remember_base_url(final_url.as_str());
                    if resp.status().is_success() && final_url.path().ends_with("/success.jsp") {
                        return 1;
                    }
                }
            }
        }
        if reachable {
            2
        } else {
            3
        }
    }

    /// Probes URLs to capture redirect and authentication queryString
    pub async fn catch_data(&self) -> Option<String> {
        self.catch_data_from_urls(self.candidate_probe_urls()).await
    }

    async fn catch_data_from_urls(&self, urls: Vec<String>) -> Option<String> {
        let mut probes = tokio::task::JoinSet::new();
        for url in urls {
            let client = self.client.clone();
            probes.spawn(async move {
                let resp = client
                    .get(url)
                    .timeout(Duration::from_secs(4))
                    .send()
                    .await
                    .ok()?;
                let final_url = resp.url().as_str().to_string();
                let body = resp.text().await.ok()?;
                Some((final_url, body))
            });
        }
        while let Some(result) = probes.join_next().await {
            if let Ok(Some((final_url, body))) = result {
                self.capture_candidates_from_response(&final_url, &body);

                if self.is_auth_host(&final_url)
                    && Url::parse(&final_url).ok().is_some_and(|url| {
                        url.path().ends_with("/index.jsp") && url.query().is_some()
                    })
                {
                    if let Some(qs) = final_url.split_once('?').map(|x| x.1.to_string()) {
                        if !qs.is_empty() {
                            return Some(qs);
                        }
                    }
                }

                if let Some(qs) = self.extract_query_string_from_body(&body) {
                    return Some(qs);
                }
            }
        }
        None
    }

    /// Executes login authentication
    pub async fn connect(&self) -> Result<(), String> {
        let query_string = match self.catch_data().await {
            Some(qs) => qs,
            None => {
                return Err("Failed to capture login queryString from the portal page".to_string())
            }
        };

        self.login(&query_string).await
    }

    async fn login(&self, query_string: &str) -> Result<(), String> {
        let form_params = [
            ("userId", self.config.user_id.as_str()),
            ("password", self.config.password.as_str()),
            ("passwordEncrypt", "false"),
            ("queryString", query_string),
            ("service", "shu"),
            ("operatorPwd", ""),
            ("operatorUserId", ""),
            ("validcode", ""),
        ];

        let mut errors = Vec::new();

        for url in self.candidate_login_urls() {
            match self
                .client
                .post(&url)
                .form(&form_params)
                .timeout(Duration::from_secs(10))
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();

                    match serde_json::from_str::<Value>(&text) {
                        Ok(json) => {
                            if json.get("result").and_then(|r| r.as_str()) == Some("success") {
                                self.remember_base_url(&url);
                                return Ok(());
                            } else {
                                // The portal answered definitively (e.g. wrong password);
                                // retrying other endpoints would only risk an account lockout.
                                let msg = json
                                    .get("message")
                                    .and_then(|m| m.as_str())
                                    .unwrap_or("Authentication failed");
                                return Err(format!("POST {} -> {}", url, msg));
                            }
                        }
                        Err(e) => {
                            errors.push(format!(
                                "POST {} -> JSON error: {} | Status: {}",
                                url, e, status
                            ));
                        }
                    }
                }
                Err(e) => {
                    errors.push(format!("POST {} -> Network error: {}", url, e));
                }
            }
        }

        Err(if errors.is_empty() {
            "No login endpoint succeeded".to_string()
        } else {
            errors.join(" | ")
        })
    }

    /// Complete flow: check connect -> login if offline
    pub async fn start_connect(&self) -> Result<String, String> {
        timeout(Duration::from_secs(30), self.connect_flow())
            .await
            .map_err(|_| {
                "Reconnect timed out after 30s; check the campus connection and proxy/TUN routing."
                    .to_string()
            })?
    }

    async fn connect_flow(&self) -> Result<String, String> {
        if NetworkManager::check_online().await {
            return Ok("Already Online".to_string());
        }
        let status = self.check_connect().await;
        match status {
            1 => Ok("Already Online".to_string()),
            2 => {
                self.connect().await?;
                Ok("Login Success".to_string())
            }
            _ => Err("Unable to reach the campus authentication server.".to_string()),
        }
    }
}
