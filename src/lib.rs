use askama::Template;
use async_stream::try_stream;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{
        Html, IntoResponse, Response, Sse,
        sse::{Event, KeepAlive},
    },
    routing::get,
};
use futures_util::stream::Stream;
use serde::{Deserialize, Serialize};
use std::{
    convert::Infallible,
    env,
    fs::{self, File},
    io::{Read, Write},
    net::SocketAddr,
    path::Path,
};
use tokio::sync::broadcast;
use tower_http::services::ServeDir;
use tracing::info;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TicketUpdate {
    pub ticket_number: u32,
    pub counter: u32,
}

#[derive(Template)]
#[template(path = "index.html")]
struct TemplateIndex {
    initial_html: String,
}

#[derive(Template)]
#[template(path = "components/ticket_update.html")]
struct TemplateTicketUpdate {
    ticket_number: u32,
    counter: u32,
}

#[derive(Debug, displaydoc::Display, thiserror::Error)]
enum AppError {
    /// could not render template
    Render(#[from] askama::Error),
    /// something went wrong
    FS(#[from] std::io::Error),
    /// parsing error
    SERDE(#[from] serde_json::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        #[derive(Debug, Template)]
        #[template(path = "error.html")]
        struct Tmpl {}

        let status = match &self {
            AppError::Render(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::FS(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::SERDE(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let tmpl = Tmpl {};
        if let Ok(body) = tmpl.render() {
            (status, Html(body)).into_response()
        } else {
            (status, "Something went wrong").into_response()
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub tx: broadcast::Sender<TicketUpdate>,
}

async fn update_ticket(
    State(state): State<AppState>,
    Json(payload): Json<TicketUpdate>,
) -> Result<&'static str, AppError> {
    let json_string = serde_json::to_string_pretty(&payload)?;
    let mut file = File::create(Path::new("state.json"))?;

    file.write_all(json_string.as_bytes())?;

    tracing::info!("state.json updated!");

    let _ = state.tx.send(payload.clone());

    Ok("Updated")
}

fn load_state() -> Option<TicketUpdate> {
    let path = Path::new("state.json");

    let mut file = fs::File::open(&path).ok()?;
    let mut content = String::new();

    file.read_to_string(&mut content).ok()?;

    if content.trim().is_empty() {
        return None;
    }

    let data: TicketUpdate = serde_json::from_str(&content).ok()?;

    Some(data)
}

async fn index() -> Result<impl IntoResponse, AppError> {
    let tmp = match load_state() {
        Some(data) => TemplateIndex {
            initial_html: TemplateTicketUpdate {
                counter: data.counter,
                ticket_number: data.ticket_number,
            }
            .render()?,
        },
        None => TemplateIndex {
            initial_html: String::new(),
        },
    };

    Ok(Html(tmp.render()?))
}

async fn sse_ticket(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut rx = state.tx.subscribe();

    Sse::new(try_stream! {
        loop {
            match rx.recv().await {
                Ok(msg) => {
                    let tmp = TemplateTicketUpdate {
                        ticket_number: msg.ticket_number,
                        counter: msg.counter
                    };
                    let data = match tmp.render() {
                        Ok(html) => html,
                        Err(err) => format!("Error occured!: {}", err)
                    };

                    yield Event::default().data(data);
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    })
    .keep_alive(KeepAlive::default())
}

pub async fn main_entry() -> () {
    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3000);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    let static_service = ServeDir::new("static");

    let (tx, _rx) = tokio::sync::broadcast::channel::<TicketUpdate>(100);
    let state = AppState { tx: tx.clone() };
    let app = Router::new()
        .nest_service("/static", static_service)
        .route("/", get(index).post(update_ticket))
        .route("/events", get(sse_ticket))
        .with_state(state);

    let server = tokio::net::TcpListener::bind(addr).await.unwrap();

    info!("Server running at {}", server.local_addr().unwrap());

    axum::serve(server, app).await.unwrap();
}
