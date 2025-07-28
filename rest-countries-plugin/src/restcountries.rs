use crate::COUNTRIES_URL;
use fibreq::{Client, ClientBuilder};
use once_cell::unsync::Lazy;
use serde_json::Value;
use std::error::Error;

thread_local! {
    static CLIENT: Lazy<Client> = Lazy::new(|| {
        ClientBuilder::new().build()
    })
}

pub fn countries_request(country_name: &str) -> Result<String, Box<dyn Error>> {
    CLIENT.with(|http_client| {
        COUNTRIES_URL.with(|url_cell| {
            let url = format!(
                "{url}/v3.1/name/{name}",
                url = url_cell.borrow(),
                name = country_name
            );

            let http_req = http_client.get(url)?;
            let mut http_resp = http_req.send()?;

            let status = http_resp.status();
            let resp_body = http_resp.text()?;

            if status != 200 {
                return Err(format!(
                    "Upstream API returned non-OK status: {}. Body: {}",
                    status, resp_body
                )
                .into());
            }

            let _: Value = serde_json::from_str(&resp_body)?;

            Ok(resp_body)
        })
    })
}
