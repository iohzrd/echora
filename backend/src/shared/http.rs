use std::time::Duration;

const USER_AGENT: &str = "EchoraBot/1.0";
const MAX_REDIRECTS: usize = 3;

pub fn create_http_client(timeout_secs: u64) -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS))
        .user_agent(USER_AGENT)
        .build()
}
