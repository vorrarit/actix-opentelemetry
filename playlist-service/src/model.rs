use std::fmt::Display;
use actix_web::ResponseError;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct Video {
    pub id: i32,
    pub link: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct Playlist {
    pub id: i32,
    #[serde(skip_serializing)]
    pub video_ids: Vec<i32>,
    pub videos: Option<Vec<Video>>,
}

#[derive(Debug)]
pub struct MyError {
    pub err: anyhow::Error
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