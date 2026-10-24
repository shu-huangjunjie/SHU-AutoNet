use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use std::time::Duration;

pub struct NetworkManager;

impl NetworkManager {
    /// Checks if truly connected to the internet without captive portal interception.
    /// Uses HTTP 204 generation endpoint. Returns true if status code is 204.
    pub async fn check_online() -> bool {
        let client = match Self::direct_client() {
            Some(client) => client,
            None => return false,
        };
        let endpoints = [
            "http://connect.rom.miui.com/generate_204",
            "http://www.qualcomm.cn/generate_204",
            "http://connectivitycheck.platform.hicloud.com/generate_204",
        ];

        Self::probe_endpoints(client, &endpoints).await
    }

    fn direct_client() -> Option<reqwest::Client> {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            ),
        );

        reqwest::Client::builder()
            // Campus connectivity must work even when a local proxy is unavailable.
            .no_proxy()
            .http1_title_case_headers()
            .default_headers(headers)
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(3))
            .redirect(reqwest::redirect::Policy::none()) // Important: captive portals redirect!
            .build()
            .ok()
    }

    async fn probe_endpoints(client: reqwest::Client, endpoints: &[&str]) -> bool {
        let mut probes = tokio::task::JoinSet::new();
        for url in endpoints {
            let client = client.clone();
            let url = url.to_string();
            probes.spawn(async move {
                client
                    .get(url)
                    .send()
                    .await
                    .map(|r| r.status() == reqwest::StatusCode::NO_CONTENT)
                    .unwrap_or(false)
            });
        }
        while let Some(result) = probes.join_next().await {
            if matches!(result, Ok(true)) {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_http::{response, Server};

    #[tokio::test]
    async fn only_an_unredirected_204_counts_as_online() {
        let portal = Server::new(|request| {
            if request.starts_with("GET /redirect ") {
                response("302 Found", "Location: /online\r\n", "")
            } else if request.starts_with("GET /online ") {
                response("204 No Content", "", "")
            } else {
                response("200 OK", "", "Login required")
            }
        })
        .await;
        let client = NetworkManager::direct_client().unwrap();
        let redirect = format!("{}/redirect", portal.base);
        let online = format!("{}/online", portal.base);
        assert!(!NetworkManager::probe_endpoints(client.clone(), &[&redirect, &portal.base]).await);
        let stalled = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let slow = format!("http://{}/", stalled.local_addr().unwrap());
        assert!(tokio::time::timeout(
            Duration::from_secs(1),
            NetworkManager::probe_endpoints(client, &[&slow, &online])
        )
        .await
        .unwrap());
    }

    #[test]
    fn configured_dead_proxy_does_not_break_probes_or_authentication() {
        const CHILD: &str = "SHU_AUTONET_PROXY_TEST";
        if std::env::var_os(CHILD).is_none() {
            let result = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "network::tests::configured_dead_proxy_does_not_break_probes_or_authentication",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .env("HTTP_PROXY", "http://127.0.0.1:1")
                .env("HTTPS_PROXY", "http://127.0.0.1:1")
                .env("ALL_PROXY", "http://127.0.0.1:1")
                .env("NO_PROXY", "")
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            return;
        }
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let portal = Server::new(|request| {
                if request.starts_with("GET /probe ") {
                    response("204 No Content", "", "")
                } else {
                    response("200 OK", "", "<script>location='index.jsp?a=1';</script>")
                }
            })
            .await;
            let url = format!("{}/probe", portal.base);
            assert!(
                NetworkManager::probe_endpoints(NetworkManager::direct_client().unwrap(), &[&url])
                    .await
            );
            let parsed = url::Url::parse(&portal.base).unwrap();
            let auth = crate::auth::ShuConnect::new(crate::config::AppConfig {
                auth_server: "127.0.0.1".into(),
                auth_port: parsed.port().unwrap().to_string(),
                test_url: portal.base.clone(),
                ..crate::config::AppConfig::default()
            });
            assert_eq!(auth.catch_data().await.as_deref(), Some("a=1"));
        });
    }

    #[tokio::test]
    #[ignore = "requires a live internet connection; run explicitly during local diagnosis"]
    async fn live_direct_connectivity() {
        let started = std::time::Instant::now();
        assert!(
            NetworkManager::check_online().await,
            "Direct internet probe failed"
        );
        println!("Direct internet probe passed in {:?}", started.elapsed());
    }
}
