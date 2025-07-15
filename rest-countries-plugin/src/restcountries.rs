use serde_json::Value;
use std::{error::Error, time::Duration};

static COUNTRIES_URL: once_cell::sync::Lazy<String> = once_cell::sync::Lazy::new(|| {
    std::env::var("COUNTRIES_URL").unwrap_or(String::from("https://restcountries.com"))
});

pub fn countries_request(
    country_name: &str,
    request_timeout: u64,
) -> Result<String, Box<dyn Error>> {
    let http_client = fibreq::ClientBuilder::new().build();
    let url = format!(
        "{url}/v3.1/name/{name}",
        url = COUNTRIES_URL.as_str(),
        name = country_name
    );

    let http_req = http_client.get(url)?;

    let mut http_resp = http_req
        .request_timeout(Duration::from_secs(request_timeout))
        .send()?;

    let resp_body = http_resp.text()?;

    let _: Value = serde_json::from_str(&resp_body)?;

    Ok(resp_body)
}
