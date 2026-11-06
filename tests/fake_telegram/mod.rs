//! Faux serveur Bot API Telegram (et faux hébergeur externe) pour les tests de bout en bout.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::Router;
use serde_json::{json, Value};

/// Un appel reçu : méthode Bot API (ou chemin hors API) et corps brut (JSON ou multipart).
#[derive(Debug, Clone)]
pub struct Call {
    pub method: String,
    pub body: String,
}

#[derive(Default)]
struct Inner {
    updates: VecDeque<Value>,
    calls: Vec<Call>,
    next_id: i64,
}

/// Poignée sur le faux serveur.
#[derive(Clone)]
pub struct FakeTelegram {
    inner: Arc<Mutex<Inner>>,
    /// Base de l'API (TELEGRAM_API_URL).
    pub url: String,
}

fn user(id: u64) -> Value {
    json!({"id": id, "is_bot": false, "first_name": "u"})
}

fn message(id: i64, chat: u64, text: Option<&str>) -> Value {
    let mut m = json!({
        "message_id": id, "date": 1,
        "chat": {"id": chat, "type": "private", "first_name": "u"},
        "from": user(chat),
    });
    if let Some(t) = text {
        m["text"] = json!(t);
    }
    m
}

impl FakeTelegram {
    pub async fn start() -> Self {
        let inner = Arc::new(Mutex::new(Inner {
            next_id: 1,
            ..Default::default()
        }));
        let app = Router::new().fallback(handle).with_state(inner.clone());
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", l.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
        Self { inner, url }
    }

    fn push(&self, kind: &str, payload: Value) {
        let mut g = self.inner.lock().unwrap();
        let id = g.next_id;
        g.next_id += 1;
        g.updates.push_back(json!({"update_id": id, kind: payload}));
    }

    /// Injecte un message texte de `user`.
    pub fn send_text(&self, user: u64, text: &str) {
        let id = self.inner.lock().unwrap().next_id + 1000;
        self.push("message", message(id, user, Some(text)));
    }

    /// Injecte un message ÉDITÉ de `user`.
    pub fn send_edited(&self, user: u64, text: &str) {
        let id = self.inner.lock().unwrap().next_id + 1000;
        let mut m = message(id, user, Some(text));
        m["edit_date"] = json!(1);
        self.push("edited_message", m);
    }

    /// Injecte un clic de bouton.
    pub fn click(&self, user: u64, data: &str) {
        let q = json!({
            "id": "cb1", "from": user_json(user), "chat_instance": "ci", "data": data,
            "message": message(7, user, Some("consentement")),
        });
        self.push("callback_query", q);
    }

    /// Copie des appels reçus, dans l'ordre.
    pub fn calls(&self) -> Vec<Call> {
        self.inner.lock().unwrap().calls.clone()
    }

    /// Appels d'une méthode donnée.
    pub fn of(&self, method: &str) -> Vec<Call> {
        self.calls()
            .into_iter()
            .filter(|c| c.method == method)
            .collect()
    }

    /// Attend (5 s max par défaut) qu'un appel vérifie `pred`.
    pub async fn wait_for(&self, secs: u64, pred: impl Fn(&Call) -> bool) -> Option<Call> {
        let end = tokio::time::Instant::now() + std::time::Duration::from_secs(secs);
        while tokio::time::Instant::now() < end {
            if let Some(c) = self.calls().into_iter().find(&pred) {
                return Some(c);
            }
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        }
        None
    }
}

fn user_json(id: u64) -> Value {
    user(id)
}

/// Identifiant de chat d'une requête (JSON ou multipart).
fn chat_of(body: &str) -> u64 {
    if let Ok(v) = serde_json::from_str::<Value>(body) {
        return v["chat_id"].as_u64().unwrap_or(1);
    }
    body.split("name=\"chat_id\"")
        .nth(1)
        .and_then(|r| r.split_whitespace().next())
        .and_then(|s| s.parse().ok())
        .unwrap_or(1)
}

async fn handle(State(st): State<Arc<Mutex<Inner>>>, uri: Uri, body: Bytes) -> Response {
    let path = uri.path().to_string();
    let body = String::from_utf8_lossy(&body).into_owned();
    // Faux hébergeur externe (repli d'envoi).
    // PUT <base>/<sha256><ext> comme curl.libriciel.fr ; l'appel est rangé sous le préfixe.
    let up = ["/upload500", "/upload"]
        .into_iter()
        .find(|p| path == *p || path.starts_with(&format!("{p}/")));
    if let Some(prefix) = up {
        st.lock().unwrap().calls.push(Call {
            method: prefix.to_string(),
            body: format!("{path}\n{body}"),
        });
        return if prefix == "/upload" {
            "https://files.example/abc\n".into_response()
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, "boom").into_response()
        };
    }
    // Jeton refusé, comme Telegram pour un BOT_TOKEN invalide : 401 « Unauthorized ».
    if path.contains(":REFUSE/") {
        let refus = json!({"ok": false, "error_code": 401, "description": "Unauthorized"});
        return (StatusCode::UNAUTHORIZED, axum::Json(refus)).into_response();
    }
    let method = path.rsplit('/').next().unwrap_or("").to_lowercase();
    if method == "getupdates" {
        // Long polling raccourci : on rend la file, ou rien après une courte attente.
        for _ in 0..10 {
            let ups: Vec<Value> = st.lock().unwrap().updates.drain(..).collect();
            if !ups.is_empty() {
                return axum::Json(json!({"ok": true, "result": ups})).into_response();
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        return axum::Json(json!({"ok": true, "result": []})).into_response();
    }
    let chat = chat_of(&body);
    let mut g = st.lock().unwrap();
    let id = g.next_id + 5000;
    g.next_id += 1;
    let n = g
        .calls
        .iter()
        .filter(|c| c.method.starts_with("send"))
        .count();
    g.calls.push(Call {
        method: method.clone(),
        body,
    });
    drop(g);
    let result = match method.as_str() {
        "getme" => json!({
            "id": 42, "is_bot": true, "first_name": "bot", "username": "test_bot",
            "can_join_groups": false, "can_read_all_group_messages": false,
            "supports_inline_queries": false, "can_connect_to_business": false,
            "has_main_web_app": false,
        }),
        "getwebhookinfo" => {
            json!({"url": "", "has_custom_certificate": false, "pending_update_count": 0})
        }
        "sendmessage" | "editmessagetext" => message(id, chat, Some("ok")),
        "sendvideo" => {
            let mut m = message(id, chat, None);
            m["video"] = json!({"file_id": format!("FILE-{n}"), "file_unique_id": format!("U{n}"),
                "width": 1, "height": 1, "duration": 5, "mime_type": "video/mp4"});
            m
        }
        "sendaudio" => {
            let mut m = message(id, chat, None);
            m["audio"] = json!({"file_id": format!("FILE-{n}"), "file_unique_id": format!("U{n}"), "duration": 5, "mime_type": "audio/mpeg"});
            m
        }
        "senddocument" => {
            let mut m = message(id, chat, None);
            m["document"] = json!({"file_id": format!("FILE-{n}"), "file_unique_id": format!("U{n}"), "mime_type": "video/mp4"});
            m
        }
        _ => json!(true),
    };
    axum::Json(json!({"ok": true, "result": result})).into_response()
}
