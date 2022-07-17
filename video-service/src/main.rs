mod configuration;

use std::fmt::Display;

use actix_web::{get, web, App, HttpServer, Responder, ResponseError, HttpResponse};
use anyhow::{anyhow};
use opentelemetry::{global, sdk::propagation::TraceContextPropagator};
use tracing_bunyan_formatter::BunyanFormattingLayer;
use tracing_subscriber::{Registry, prelude::__tracing_subscriber_SubscriberExt, EnvFilter};

use configuration::{Settings, get_configuration};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct Video {
    id: i32,
    link: String,
}

#[derive(Debug, Clone)]
struct VideoService {
    videos: Vec<Video>
}

impl VideoService {
    fn new_from_list(list: Vec<Video>) -> VideoService {
        VideoService {
            videos: list
        }
    }

    fn get_by_id(&self, id: i32) -> Option<Video> {
        self.videos.iter().filter(|v| {
            v.id == id
        }).collect::<Vec<_>>().first().cloned().cloned()
    }
}

#[get("/hello/{name}")]
async fn greet(name: web::Path<String>) -> impl Responder {
    format!("Hello {}!", name)
}

#[tracing::instrument(skip(video_service))]
#[get("/video/{id}")]
async fn video_handler(id: web::Path<i32>, video_service: web::Data<VideoService>) -> Result<HttpResponse, MyError> {
    let video = video_service.get_by_id(id.clone());
    match video {
        Some(v) => Ok(HttpResponse::Ok().json(v)),
        None => Err(MyError{ err: anyhow!("Cannot find") })
    }
}

#[derive(Debug)]
struct MyError {
    err: anyhow::Error
}

impl From<anyhow::Error> for MyError {
    fn from(err: anyhow::Error) -> Self {
        MyError { err }
    }
}

impl ResponseError for MyError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
    }
}

impl Display for MyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}'n", self)?;
        Ok(())
    }
}

fn init_telemetry(config: &Settings) {
    let app_name = "video-service";

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

    let videos = vec![
        Video { id: 1, link: "111".to_string() },
        Video { id: 2, link: "222".to_string() }
    ];
    let video_serivce = VideoService::new_from_list(videos);

    HttpServer::new(move || {
        App::new()
            // .wrap(Logger::default())
            // .wrap(RequestTracing::new())
            .wrap(request_metrics.clone())
            .wrap(tracing_actix_web::TracingLogger::default())
            .app_data(web::Data::new(video_serivce.clone()))
            .route("/hello", web::get().to(|| async { "Hello World!" }))
            .service(greet)
            .service(video_handler)
    })
    .bind(("0.0.0.0", config.application_port))?
    .run()
    .await?;

    opentelemetry::global::shutdown_tracer_provider();

    Ok(())
}