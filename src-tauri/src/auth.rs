use crate::config::AppConfig;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Duration;
use tokio::net::{lookup_host, TcpStream};
use tokio::time::timeout;
use url::Url;

pub struct ShuConnect {
    config: AppConfig,
    portal_candidates: Mutex<Vec<String>>,
    client: reqwest::Client,
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
            .default_headers(headers)
            .redirect(reqwest::redirect::Policy::limited(10))
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
            .and_then(|u| u.host_str().map(|h| h.eq_ignore_ascii_case(self.config.auth_server.trim())))
            .unwrap_or(false)
    }

    fn remember_base_url(&self, url_str: &str) {
        if !self.is_auth_host(url_str) {
            return;
        }
        if let Some(base) = Self::normalize_base_url(url_str) {
            let mut list = self.portal_candidates.lock().unwrap();
            if !list.contains(&base) {
                list.push(base);
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

        let content_start = start_idx + offset;
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
            format!("http://{}:{}/", self.config.auth_server, self.config.auth_port.trim()),
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
            Self::normalize_base_url(&format!("http://{}:{}", self.config.auth_server, self.config.auth_port.trim())),
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
            let login_url = format!("{}/eportal/InterFace.do?method=login", base.trim_end_matches('/'));
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
        let mut reachable_base = None;

        for base in self.candidate_base_urls() {
            if let Ok(parsed) = Url::parse(&base) {
                if let Some(host) = parsed.host_str() {
                    let port = parsed.port().unwrap_or(if parsed.scheme() == "https" { 443 } else { 80 });
                    let addr = format!("{}:{}", host, port);

                    if let Ok(Ok(socket_addrs)) = timeout(Duration::from_secs(2), lookup_host(addr)).await {
                        for sa in socket_addrs {
                            if let Ok(Ok(_)) = timeout(Duration::from_secs(2), TcpStream::connect(sa)).await {
                                reachable_base = Some(base.clone());
                                break;
                            }
                        }
                    }
                }
            }
            if reachable_base.is_some() {
                break;
            }
        }

        let base_url = match reachable_base {
            Some(url) => {
                self.remember_base_url(&url);
                url
            }
            None => return 3,
        };

        match self.client.get(&base_url).timeout(Duration::from_secs(5)).send().await {
            Ok(resp) => {
                let final_url = resp.url().as_str();
                if final_url.contains("success.jsp") {
                    1
                } else {
                    2
                }
            }
            Err(_) => 2,
        }
    }

    /// Probes URLs to capture redirect and authentication queryString
    pub async fn catch_data(&self) -> Option<String> {
        for url in self.candidate_probe_urls() {
            if let Ok(resp) = self.client.get(&url).send().await {
                let final_url = resp.url().as_str().to_string();
                let body = resp.text().await.unwrap_or_default();

                self.capture_candidates_from_response(&final_url, &body);

                if final_url.contains('?') && (final_url.contains(&self.config.auth_server) || final_url.contains("eportal")) {
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
            None => return Err("Failed to capture login queryString from the portal page".to_string()),
        };

        let form_params = [
            ("userId", self.config.user_id.as_str()),
            ("password", self.config.password.as_str()),
            ("passwordEncrypt", "false"),
            ("queryString", query_string.as_str()),
            ("service", "shu"),
            ("operatorPwd", ""),
            ("operatorUserId", ""),
            ("validcode", ""),
        ];

        let mut errors = Vec::new();

        for url in self.candidate_login_urls() {
            match self.client.post(&url).form(&form_params).timeout(Duration::from_secs(10)).send().await {
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
                                let msg = json.get("message").and_then(|m| m.as_str()).unwrap_or("Authentication failed");
                                return Err(format!("POST {} -> {}", url, msg));
                            }
                        }
                        Err(e) => {
                            errors.push(format!("POST {} -> JSON error: {} | Status: {}", url, e, status));
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
