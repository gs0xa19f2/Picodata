use picotest::fixture;
use reqwest::Client;
use tokio::runtime::Runtime;

// Общие ресурсы для тестов
pub struct TestContext {
    pub rt: Runtime,
    pub client: Client,
    pub base_url: String,
}

// Определяем фикстуру, где #[once] гарантирует, что код выполнится только один раз для всех тестов
#[fixture]
#[once]
pub fn test_ctx() -> TestContext {
    let rt = Runtime::new().unwrap();
    let client = Client::new();
    let host = "localhost";
    let http_port = 8001;
    let base_url = format!("http://{}:{}", host, http_port);

    TestContext {
        rt,
        client,
        base_url,
    }
}
