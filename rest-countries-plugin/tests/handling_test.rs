use picotest::*;
use std::time::Duration;
use tokio::runtime::Runtime;

#[picotest]
fn test_error_handling() {
    let rt = Runtime::new().unwrap();

    let host = "localhost";
    let http_port = 8001;

    // Запрос с пустым именем страны
    let response = rt.block_on(async {
        let client = reqwest::Client::new();
        let base_url = format!("http://{}:{}", host, http_port);

        let response = client
            .post(&format!("{}/country", base_url))
            .header("Content-Type", "application/json")
            .body(r#"{"name":""}"#)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .expect("Failed to send request");

        let status = response.status().as_u16();
        let text = response.text().await.expect("Failed to get response text");

        (status, text)
    });

    // Должны получить ошибку
    assert_eq!(response.0, 500);

    // Проверяем текст ошибки
    assert!(response.1.contains("Country name is required"));
}
