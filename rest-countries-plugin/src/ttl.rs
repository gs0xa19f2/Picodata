use picodata_plugin::background::CancellationToken;
use picodata_plugin::sql::query;
use picodata_plugin::system::tarantool::{clock::time, say_error, say_info};
use std::time::Duration;

pub const TTL_JOB_NAME: &str = "ttl-worker";

const TTL_QUERY: &str = r#"
    DELETE FROM countries WHERE country_name IN (
        SELECT country_name FROM countries
            WHERE created_at <= ?
            LIMIT 10
    );
"#;

// ttl in seconds
pub fn get_ttl_job(ttl: i64) -> impl Fn(CancellationToken) {
    move |ct: CancellationToken| {
        while ct.wait_timeout(Duration::from_secs(1)).is_err() {
            let expired = time() as i64 - ttl;
            match query(&TTL_QUERY).bind(expired).execute() {
                Ok(rows_affected) => {
                    if rows_affected > 0 {
                        say_info!("Cleaned {rows_affected:?} expired country records");
                    }
                }
                Err(error) => {
                    say_error!("Error while cleaning expired country records: {error:?}")
                }
            };
        }
        say_info!("TTL worker stopped");
    }
}
