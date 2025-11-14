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
use std::{convert::Infallible, env, net::SocketAddr};
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
struct TemplateIndex;

#[derive(Template)]
#[template(path = "components/ticket_update.html")]
pub struct TemplateTicketUpdate {
    pub ticket_number: u32,
    pub counter: u32,
}

#[derive(Debug, displaydoc::Display, thiserror::Error)]
enum AppError {
    /// could not render template
    Render(#[from] askama::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        #[derive(Debug, Template)]
        #[template(path = "error.html")]
        struct Tmpl {}

        let status = match &self {
            AppError::Render(_) => StatusCode::INTERNAL_SERVER_ERROR,
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
    let _ = state.tx.send(payload.clone());

    Ok("Updated")
}

async fn index() -> Result<impl IntoResponse, AppError> {
    let tmp = TemplateIndex {};

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
