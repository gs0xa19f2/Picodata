use picotest::*;
mod common;
use common::test_ctx;

#[picotest]
fn test_error_handling(test_ctx: &common::TestContext) {
    // Пустое имя страны
    let response = test_ctx.rt.block_on(async {
        let url = format!("{}/name?name=", test_ctx.base_url);
        test_ctx.client.get(&url).send().await.unwrap()
    });
    assert_eq!(response.status().as_u16(), 400);
    let text = test_ctx
        .rt
        .block_on(async { response.text().await.unwrap() });
    assert!(text.contains("Country name is required"));

    // Несуществующая страна
    let response = test_ctx.rt.block_on(async {
        let url = format!("{}/name?name=nonexistentcountry", test_ctx.base_url);
        test_ctx.client.get(&url).send().await.unwrap()
    });
    assert_eq!(response.status().as_u16(), 500);
    let text = test_ctx
        .rt
        .block_on(async { response.text().await.unwrap() });
    assert!(text.contains("Upstream API returned non-OK status"));
}
