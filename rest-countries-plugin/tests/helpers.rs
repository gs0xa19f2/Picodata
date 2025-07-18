use reqwest;
use std::time::Duration;

/// Создает запрос к API плагина для получения информации о стране
pub async fn get_country_info(
    host: &str,
    port: u16,
    country_name: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let base_url = format!("http://{}:{}", host, port);

    let response = client
        .post(&format!("{}/country", base_url))
        .header("Content-Type", "application/json")
        .body(format!(r#"{{"name":"{}"}}"#, country_name))
        .timeout(Duration::from_secs(10))
        .send()
        .await?;

    let response_text = response.text().await?;
    Ok(response_text)
}
