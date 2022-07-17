mod model;
mod configuration;

use actix_web::{get, web, App, HttpServer, Responder, HttpResponse};
use anyhow::Context;
use futures::{future::join_all};
use opentelemetry::{sdk::propagation::TraceContextPropagator, global};
use reqwest_middleware::ClientBuilder;
use reqwest_tracing::TracingMiddleware;
use tracing_bunyan_formatter::BunyanFormattingLayer;
use tracing_subscriber::{Registry, prelude::__tracing_subscriber_SubscriberExt, EnvFilter};

use model::{Video, Playlist, MyError};
use crate::configuration::{get_configuration, Settings};

#[derive(Debug, Clone)]
struct PlaylistService {
    playlists: Vec<Playlist>,
    config: Settings
}

impl PlaylistService {
    fn new_from_list(list: Vec<Playlist>, config: Settings) -> PlaylistService {
        PlaylistService {
            playlists: list,
            config
        }
    }

    async fn get_by_id(&self, id: i32) -> Option<Playlist> {
        if let Some(x) = self.playlists.iter().filter(|v| {
            (**v).id == id
        }).map(|x| { x.clone() }).collect::<Vec<_>>().first() {
            let mut playlist = x.clone();
            let videos: Vec<Video> = join_all(
            playlist.video_ids.iter().map(|video_id| async move {
                let client = ClientBuilder::new(reqwest::Client::new())
                    .with(TracingMiddleware::default())
                    .build();

                    let response_result = client.get(format!("{}/video/{}", self.config.video_service, video_id)).send().await;
                    if let Ok(response) = response_result {
                        if let Ok(video) = response.json::<Video>().await {
                            return Some(video)
                        }
                    }
                    None
                }
            )
            ).await.iter().filter(|x| { (**x).is_some() }).map(|x| (*x).clone().unwrap() ).collect();

            playlist.videos = Some(videos);
            return Some(playlist);
        } else {
            return None;
        }
    }
}

#[get("/hello/{name}")]
async fn greet(name: web::Path<String>) -> impl Responder {
    format!("Hello {}!", name)
}

#[tracing::instrument(skip(playlist_service))]
#[get("/playlist/{id}")]
async fn playlist_handler(id: web::Path<i32>, playlist_service: web::Data<PlaylistService>) -> Result<HttpResponse, MyError> {
    tracing::info!("get playlist");
    let playlist = playlist_service.get_by_id(id.clone()).await.context("Cannot find")?;
    Ok(HttpResponse::Ok().json(playlist))
}

fn init_telemetry(config: &Settings) {
    let app_name = "playlist-service";

    // Start a new Jaeger trace pipeline.
    // Spans are exported in batch - recommended setup for a production application.
    global::set_text_map_propagator(TraceContextPropagator::new());
    // let tracer = opentelemetry_jaeger::new_pipeline()
    //     .with_service_name(app_name)
    //     .install_batch(TokioCurrentThread)
    //     .expect("Failed to install OpenTelemetry tracer.");

    let tracer = opentelemetry_jaeger::new_pipeline()
        .with_service_name(app_name)
        .with_agent_endpoint(&config.jaeger_agent_endpoint)
        .install_simple()
        .unwrap();


    // Filter based on level - trace, debug, info, warn, error
    // Tunable via `RUST_LOG` env variable
    let env_filter = EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new("info"));
    // Create a `tracing` layer using the Jaeger tracer
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);
    // Create a `tracing` layer to emit spans as structured logs to stdout
    let formatting_layer = BunyanFormattingLayer::new(app_name.into(), std::io::stdout);
    // Combined them all together in a `tracing` subscriber
    let subscriber = Registry::default()
        .with(env_filter)
        .with(telemetry)
        .with(tracing_bunyan_formatter::JsonStorageLayer)
        .with(formatting_layer);
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to install `tracing` subscriber.")
}

fn init_configuration() -> Settings {
    get_configuration().expect("Failed to get configuration.")
}

#[actix_web::main] // or #[tokio::main]
async fn main() -> std::io::Result<()> {
    let metrics_exporter = opentelemetry_prometheus::exporter().init();
    let request_metrics = actix_web_opentelemetry::RequestMetrics::new(
        opentelemetry::global::meter("actix_http_tracing"),
        Some(|req: &actix_web::dev::ServiceRequest| {
            req.path() == "/metrics" && req.method() == actix_web::http::Method::GET
        }),
        Some(metrics_exporter),
    );

    let config = init_configuration();

    init_telemetry(&config);


    let playlists = vec![
        Playlist { id: 1, video_ids: vec![1, 2], videos: None },
        Playlist { id: 2, video_ids: vec![1], videos: None}
    ];
    let playlist_serivce = PlaylistService::new_from_list(playlists, config.clone());

    HttpServer::new(move || {
        App::new()
            .wrap(request_metrics.clone())
            .wrap(tracing_actix_web::TracingLogger::default())
            .app_data(web::Data::new(playlist_serivce.clone()))
            .route("/hello", web::get().to(|| async { "Hello World!" }))
            .service(greet)
            .service(playlist_handler)
    })
    .bind(("0.0.0.0", config.application_port))?
    .run()
    .await?;

    opentelemetry::global::shutdown_tracer_provider();

    Ok(())
}