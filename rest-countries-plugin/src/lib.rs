use serde::Deserialize;
use serde::Serialize;

use once_cell::unsync::Lazy;
use picodata_plugin::background::CancellationToken;
use picodata_plugin::plugin::prelude::*;
use picodata_plugin::system::tarantool::{clock::time, say_error, say_info};
use shors::transport::http::{route::Builder, route::Handler, server, Request, Response};
use shors::transport::Context;

use std::cell::RefCell;
use std::error::Error;
use std::fmt;
use std::time::Duration;

use picodata_plugin::sql::query;

mod restcountries;
mod ttl;

#[derive(Debug)]
struct AppError {
    status: u16,
    message: String,
}

impl AppError {
    fn bad_request(message: &str) -> Box<Self> {
        Box::new(Self {
            status: 400,
            message: message.to_string(),
        })
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for AppError {}

thread_local! {
    pub static HTTP_SERVER: Lazy<server::Server> = Lazy::new(server::Server::new);
    pub static COUNTRIES_URL: RefCell<String> = RefCell::new(String::new());
}

const SELECT_QUERY: &str = r#"
SELECT * FROM countries
WHERE
    country_name = ?;
"#;

const INSERT_QUERY: &str = r#"
INSERT INTO "countries"
VALUES(?, ?, ?)
"#;

struct CountriesService;

#[derive(Serialize, Deserialize, Debug, Default)]
struct ServiceCfg {
    // ttl in seconds
    ttl: i64,
    countries_url: Option<String>,
}

fn error_handler_middleware(handler: Handler<Box<dyn Error>>) -> Handler<Box<dyn Error>> {
    Handler(Box::new(move |ctx, request| {
        let inner_res = handler(ctx, request);
        let resp = inner_res.unwrap_or_else(|err| {
            if let Some(app_err) = err.downcast_ref::<AppError>() {
                let mut resp: Response = Response::from(app_err.to_string());
                resp.status = app_err.status as u32;
                resp
            } else {
                say_error!("{err:?}");
                let mut resp: Response = Response::from(err.to_string());
                resp.status = 500;
                resp
            }
        });

        return Ok(resp);
    }))
}

fn apply_config(cfg: &ServiceCfg) {
    let url = cfg
        .countries_url
        .clone()
        .unwrap_or_else(|| "https://restcountries.com".to_string());
    COUNTRIES_URL.with(|cell| *cell.borrow_mut() = url);
}

impl Service for CountriesService {
    type Config = ServiceCfg;

    fn on_config_change(
        &mut self,
        ctx: &PicoContext,
        new_cfg: Self::Config,
        _old_cfg: Self::Config,
    ) -> CallbackResult<()> {
        apply_config(&new_cfg);

        // ttl in seconds
        ctx.cancel_tagged_jobs(ttl::TTL_JOB_NAME, Duration::from_secs(1))
            .map_err(|err| format!("failed to cancel tagged jobs on config change: {err}"))?;
        ctx.register_tagged_job(ttl::get_ttl_job(new_cfg.ttl), ttl::TTL_JOB_NAME)
            .map_err(|err| format!("failed to register tagged jobs on config change: {err}"))?;
        Ok(())
    }

    fn on_start(&mut self, ctx: &PicoContext, cfg: Self::Config) -> CallbackResult<()> {
        say_info!("Countries cache service started with config: {cfg:?}");
        apply_config(&cfg);

        #[derive(Deserialize)]
        struct CountryQueryParams {
            name: String,
        }

        #[derive(Serialize, Deserialize, Debug, Clone)]
        pub struct CountryCache {
            country_name: String,
            response_data: String,
            created_at: i64,
        }

        let pico_ctx = unsafe { ctx.clone() };

        let countries_endpoint = Builder::new()
            .with_method("GET")
            .with_path("/name")
            .with_middleware(error_handler_middleware)
            .build(
                move |_ctx: &mut Context, request: Request| -> Result<_, Box<dyn Error>> {
                    let params: CountryQueryParams = request.query()?;
                    let country_name = params.name;

                    if country_name.is_empty() {
                        return Err(AppError::bad_request("Country name is required"));
                    }

                    let cached: Vec<CountryCache> = query(&SELECT_QUERY)
                        .bind(country_name.clone())
                        .fetch::<CountryCache>()
                        .map_err(|err| format!("failed to retrieve data: {err}"))?;

                    if let Some(cache_entry) = cached.into_iter().next() {
                        say_info!("Cache hit for country: {}", country_name);
                        return Ok(cache_entry.response_data);
                    }

                    say_info!(
                        "Cache miss for country: {}, fetching from API",
                        country_name
                    );
                    let countries_resp = restcountries::countries_request(&country_name)?;

                    let cache_job_pico_ctx = unsafe { pico_ctx.clone() };
                    let response_to_cache = countries_resp.clone();

                    let cache_job = move |_: CancellationToken| {
                        say_info!("Starting background job to cache country: {}", country_name);
                        let res = query(&INSERT_QUERY)
                            .bind(country_name)
                            .bind(response_to_cache)
                            .bind(time() as i64)
                            .execute();

                        if let Err(e) = res {
                            say_error!("Failed to cache data in background job: {}", e);
                        }
                    };

                    cache_job_pico_ctx
                        .register_job(cache_job)
                        .map_err(|e| format!("failed to register cache job: {}", e))?;

                    Ok(countries_resp)
                },
            );

        HTTP_SERVER.with(|srv| {
            srv.register(Box::new(countries_endpoint));
        });

        // ttl in seconds
        let ttl_job = ttl::get_ttl_job(cfg.ttl);
        ctx.register_tagged_job(ttl_job, ttl::TTL_JOB_NAME)
            .map_err(|err| format!("failed to register tagged job: {err}"))?;

        Ok(())
    }

    fn on_stop(&mut self, ctx: &PicoContext) -> CallbackResult<()> {
        say_info!("Countries service stopped");

        // ttl in seconds
        ctx.cancel_tagged_jobs(ttl::TTL_JOB_NAME, Duration::from_secs(1))
            .map_err(|err| format!("failed to cancel tagged jobs on stop: {err}"))?;

        Ok(())
    }

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
