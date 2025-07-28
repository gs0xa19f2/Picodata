use picotest::*;
use std::time::{Duration, Instant};

mod common;
use common::test_ctx;
mod helpers;
use helpers::get_country_info;

/// Вспомогательная функция для извлечения значения count из табличного вывода picotest
fn get_count_from_output(output: &str) -> Option<i64> {
    output
        .lines()
        .find(|line| line.starts_with('|') && line.contains('|') && !line.contains("count"))
        .and_then(|line| line.split('|').nth(1))
        .and_then(|value| value.trim().parse::<i64>().ok())
}

#[picotest]
fn test_cache_miss_and_hit(test_ctx: &common::TestContext) {
    let country = "germany";
    let _ = cluster
        .run_query(&format!(
            "DELETE FROM countries WHERE country_name = '{}'",
            country
        ))
        .expect("Failed to execute delete query");

    // Должен быть промах кеша (первый запрос)
    let start = Instant::now();
    let first_response = test_ctx
        .rt
        .block_on(async { get_country_info(test_ctx, country).await.unwrap() });
    let first_duration = start.elapsed();

    // Корректно парсим вывод и проверяем значение
    let cache_query_result = cluster
        .run_query(&format!(
            "SELECT COUNT(*) as count FROM countries WHERE country_name = '{}'",
            country
        ))
        .expect("Failed to execute count query");

    let count = get_count_from_output(&cache_query_result)
        .expect("Failed to parse count from query output");
    assert_eq!(
        count, 1,
        "Expected count to be 1, but got: {}",
        cache_query_result
    );

    // Получаем timestamp после первой записи в кеш
    let first_timestamp_query_result = cluster
        .run_query(&format!(
            "SELECT created_at FROM countries WHERE country_name = '{}'",
            country
        ))
        .expect("Failed to get timestamp after first request");

    // Должно быть попадание в кеш (второй запрос)
    let start = Instant::now();
    let second_response = test_ctx
        .rt
        .block_on(async { get_country_info(test_ctx, country).await.unwrap() });
    let second_duration = start.elapsed();

    // Получаем timestamp после второго запроса
    let second_timestamp_query_result = cluster
        .run_query(&format!(
            "SELECT created_at FROM countries WHERE country_name = '{}'",
            country
        ))
        .expect("Failed to get timestamp after second request");

    // Убеждаемся, что timestamp не изменился
    assert_eq!(
        first_timestamp_query_result, second_timestamp_query_result,
        "Cache entry was updated on cache hit, but it should not have been."
    );

    // Проверяем, что ответы идентичны
    assert_eq!(first_response, second_response);
    println!("First request (miss) time: {:?}", first_duration);
    println!("Second request (hit) time: {:?}", second_duration);
}

#[picotest]
fn test_ttl_expiration(test_ctx: &common::TestContext) {
    // ttl in seconds
    let plugin_config_yaml = r#"
    countries_service:
      ttl: 5
    "#;
    let plugin_config: PluginConfigMap = serde_yaml::from_str(plugin_config_yaml).unwrap();
    cluster
        .apply_config(plugin_config)
        .expect("Failed to apply config");

    let country = "france";
    let _ = cluster
        .run_query(&format!(
            "DELETE FROM countries WHERE country_name = '{}'",
            country
        ))
        .expect("Failed to execute delete query");

    // Кеширование данных
    let _ = test_ctx
        .rt
        .block_on(async { get_country_info(test_ctx, country).await.unwrap() });

    // Проверяем, что в кеше появилась ровно одна запись
    let cache_query_result = cluster
        .run_query(&format!(
            "SELECT COUNT(*) as count FROM countries WHERE country_name = '{}'",
            country
        ))
        .expect("Failed to execute count query");
    let count_after_caching = get_count_from_output(&cache_query_result)
        .expect("Failed to parse count from query output after caching");
    assert_eq!(
        count_after_caching, 1,
        "Expected count to be 1 after caching, but got: {}",
        cache_query_result
    );

    let wait_timeout = Instant::now() + Duration::from_secs(10);
    loop {
        let query_result = cluster
            .run_query(&format!(
                "SELECT COUNT(*) as count FROM countries WHERE country_name = '{}'",
                country
            ))
            .expect("Failed to execute count query during wait");

        if let Some(count) = get_count_from_output(&query_result) {
            if count == 0 {
                break;
            }
        }

        if Instant::now() > wait_timeout {
            panic!("Timeout expired while waiting for cache entry to be deleted");
        }

        std::thread::sleep(Duration::from_millis(200));
    }

    let _ = test_ctx
        .rt
        .block_on(async { get_country_info(test_ctx, country).await.unwrap() });
}
