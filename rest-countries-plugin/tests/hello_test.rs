use picotest::*;
use std::time::Duration;
use tokio::runtime::Runtime;

#[picotest]
fn test_hello_endpoint() {
    let rt = Runtime::new().unwrap();

    let host = "localhost";
    let http_port = 8001;

    // Делаем запрос к hello endpoint
    let response = rt.block_on(async {
        let client = reqwest::Client::new();
        let base_url = format!("http://{}:{}", host, http_port);

        let response = client
            .get(&format!("{}/hello", base_url))
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .expect("Failed to send request");

        let status = response.status().as_u16();
        let text = response.text().await.expect("Failed to get response text");

        (status, text)
    });

    // Проверяем статус ответа
    assert_eq!(response.0, 200);

    // Проверяем тело ответа, учитывая возможные форматы
    if response.1.starts_with('"') && response.1.ends_with('"') {
        // Если ответ в формате JSON-строки
        let json_str: String =
            serde_json::from_str(&response.1).expect("Failed to parse JSON response");
        assert_eq!(json_str, "Hello, World!");
    } else {
        // Если ответ в виде обычного текста
        assert_eq!(response.1, "Hello, World!");
    }
}
