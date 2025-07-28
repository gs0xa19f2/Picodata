use crate::common::TestContext;
use std::time::Duration;

/// Создает запрос к API плагина для получения информации о стране
pub async fn get_country_info(
    test_ctx: &TestContext,
    country_name: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let url = format!("{}/name?name={}", test_ctx.base_url, country_name);

    let response = test_ctx
        .client
        .get(&url)
        .timeout(Duration::from_secs(10))
        .send()
        .await?;

    let response_text = response.text().await?;
    Ok(response_text)
}
