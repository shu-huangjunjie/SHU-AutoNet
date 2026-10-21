use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use std::time::Duration;

pub struct NetworkManager;

impl NetworkManager {
    /// Checks if truly connected to the internet without captive portal interception.
    /// Uses HTTP 204 generation endpoint. Returns true if status code is 204.
    pub async fn check_online() -> bool {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            ),
        );

        let client = match reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(3))
            .redirect(reqwest::redirect::Policy::none()) // Important: captive portals redirect!
            .build()
        {
            Ok(c) => c,
            Err(_) => return false,
        };

        let endpoints = [
            "http://connect.rom.miui.com/generate_204",
            "http://www.qualcomm.cn/generate_204",
            "http://connectivitycheck.platform.hicloud.com/generate_204",
        ];

        for url in endpoints {
            if let Ok(resp) = client.get(url).send().await {
                if resp.status().as_u16() == 204 {
                    return true;
                }
            }
        }

        false
    }
}
