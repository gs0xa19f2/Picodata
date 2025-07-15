use picodata_plugin::background::CancellationToken;
use serde::Deserialize;
use serde::Serialize;

use once_cell::unsync::Lazy;
use picodata_plugin::plugin::prelude::*;
use picodata_plugin::system::tarantool::{clock::time, say_info};
use shors::transport::http::route::Builder;
use shors::transport::http::{route::Handler, server, Request, Response};
use shors::transport::Context;
use tarantool::say_error;

use std::cell::Cell;
use std::error::Error;
use std::time::Duration;

mod restcountries;

thread_local! {
    pub static HTTP_SERVER: Lazy<server::Server> = Lazy::new(server::Server::new);
}
thread_local! {
    pub static TIMEOUT: Cell<Duration> = Cell::new(Duration::from_secs(10));
}

const TTL_JOB_NAME: &str = "ttl-worker";

const SELECT_QUERY: &str = r#"
SELECT * FROM countries
WHERE
    country_name = ?;
"#;

const INSERT_QUERY: &str = r#"
INSERT INTO "countries"
VALUES(?, ?, ?)
"#;

const TTL_QUERY: &str = r#"
    DELETE FROM countries WHERE country_name IN (
        SELECT country_name FROM countries
            WHERE created_at <= ?
            LIMIT 10
    );
"#;

struct CountriesService;

#[derive(Serialize, Deserialize, Debug)]
struct ServiceCfg {
    timeout: u64,
    ttl: i64,
}

fn error_handler_middleware(handler: Handler<Box<dyn Error>>) -> Handler<Box<dyn Error>> {
    Handler(Box::new(move |ctx, request| {
        let inner_res = handler(ctx, request);
        let resp = inner_res.unwrap_or_else(|err| {
            say_error!("{err:?}");
            let mut resp: Response = Response::from(err.to_string());
            resp.status = 500;
            resp
        });

        return Ok(resp);
    }))
}

fn get_ttl_job(ttl: i64) -> impl Fn(CancellationToken) {
    move |ct: CancellationToken| {
        while ct.wait_timeout(Duration::from_secs(1)).is_err() {
            let expired = time() as i64 - ttl;
            match picodata_plugin::sql::query(&TTL_QUERY)
                .bind(expired)
                .execute()
            {
                Ok(rows_affected) => {
                    say_info!("Cleaned {rows_affected:?} expired country records");
                }
                Err(error) => {
                    say_error!("Error while cleaning expired country records: {error:?}")
                }
            };
        }
        say_info!("TTL worker stopped");
    }
}

impl Service for CountriesService {
    type Config = ServiceCfg;

    fn on_config_change(
        &mut self,
        ctx: &PicoContext,
        new_cfg: Self::Config,
        _old_cfg: Self::Config,
    ) -> CallbackResult<()> {
        TIMEOUT.set(Duration::from_secs(new_cfg.timeout));
        ctx.cancel_tagged_jobs(TTL_JOB_NAME, Duration::from_secs(1))
            .map_err(|err| format!("failed to cancel tagged jobs on config change: {err}"))?;
        ctx.register_tagged_job(get_ttl_job(new_cfg.ttl), TTL_JOB_NAME)
            .map_err(|err| format!("failed to register tagged jobs on config change: {err}"))?;
        Ok(())
    }

    fn on_start(&mut self, ctx: &PicoContext, cfg: Self::Config) -> CallbackResult<()> {
        say_info!("Countries cache service started with config: {cfg:?}");

        let hello_endpoint = Builder::new().with_method("GET").with_path("/hello").build(
            |_ctx: &mut Context, _: Request| -> Result<_, Box<dyn Error>> {
                Ok("Hello, World!".to_string())
            },
        );

        #[derive(Serialize, Deserialize)]
        pub struct CountryReq {
            name: String,
        }

        #[derive(Serialize, Deserialize, Debug, Clone)]
        pub struct CountryCache {
            country_name: String,
            response_data: String,
            created_at: i64,
        }

        let countries_endpoint = Builder::new()
            .with_method("POST")
            .with_path("/country")
            .with_middleware(error_handler_middleware)
            .build(
                |_ctx: &mut Context, request: Request| -> Result<_, Box<dyn Error>> {
                    let req: CountryReq = request.parse()?;
                    let country_name = req.name;

                    if country_name.is_empty() {
                        return Err("Country name is required".into());
                    }

                    // Проверяем, есть ли страна в кеше
                    let cached: Vec<CountryCache> = picodata_plugin::sql::query(&SELECT_QUERY)
                        .bind(country_name.clone())
                        .fetch::<CountryCache>()
                        .map_err(|err| format!("failed to retrieve data: {err}"))?;

                    // Если страна найдена в кеше, возвращаем данные из кеша
                    if !cached.is_empty() {
                        say_info!("Cache hit for country: {}", country_name);
                        return Ok(cached[0].response_data.clone());
                    }

                    // Иначе запрашиваем данные из API
                    say_info!(
                        "Cache miss for country: {}, fetching from API",
                        country_name
                    );
                    let timeout = TIMEOUT.get().as_secs();
                    let countries_resp = restcountries::countries_request(&country_name, timeout)?;

                    // Сохраняем полученные данные в кеш
                    let _ = picodata_plugin::sql::query(&INSERT_QUERY)
                        .bind(country_name.clone()) // Клонируем, чтобы передать по значению
                        .bind(countries_resp.clone()) // Клонируем, чтобы передать по значению
                        .bind(time() as i64)
                        .execute()
                        .map_err(|err| format!("failed to insert data: {err}"))?;

                    Ok(countries_resp)
                },
            );

        HTTP_SERVER.with(|srv| {
            srv.register(Box::new(hello_endpoint));
            srv.register(Box::new(countries_endpoint));
        });

        let ttl_job = get_ttl_job(cfg.ttl);
        ctx.register_tagged_job(ttl_job, TTL_JOB_NAME)
            .map_err(|err| format!("failed to register tagged job: {err}"))?;

        Ok(())
    }

    fn on_stop(&mut self, ctx: &PicoContext) -> CallbackResult<()> {
        say_info!("Countries service stopped");

        ctx.cancel_tagged_jobs(TTL_JOB_NAME, Duration::from_secs(1))
            .map_err(|err| format!("failed to cancel tagged jobs on stop: {err}"))?;

        Ok(())
    }

    /// Called after replicaset master is changed
    fn on_leader_change(&mut self, _ctx: &PicoContext) -> CallbackResult<()> {
        say_info!("Leader has changed!");
        Ok(())
    }
}

impl CountriesService {
    pub fn new() -> Self {
        CountriesService {}
    }
}

#[service_registrar]
pub fn service_registrar(reg: &mut ServiceRegistry) {
    reg.add(
        "countries_service",
        env!("CARGO_PKG_VERSION"),
        CountriesService::new,
    );
}
