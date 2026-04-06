use anyhow::{Context, Result};
use base64::Engine;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::thread;
use tiny_http::{Header, Method, Response, Server, StatusCode};

use crate::config::WebConfig;
use crate::llm::OllamaLlm;
use crate::pipeline::{detect_intent, Intent};
use crate::rag::RagEngine;
use crate::tts::VoicevoxTts;

const MAX_REQUEST_BYTES: usize = 32 * 1024;

pub struct WebApiState {
    pub llm: Arc<OllamaLlm>,
    pub tts: Arc<VoicevoxTts>,
    pub rag: Arc<Mutex<Option<RagEngine>>>,
}

#[derive(Debug, Deserialize)]
struct ChatRequest {
    text: String,
    #[serde(default)]
    speak: bool,
}

#[derive(Debug, Serialize)]
struct ChatResponse {
    response: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    audio_base64: Option<String>,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    api: &'static str,
    webui: &'static str,
}

pub fn start_web_server(config: &WebConfig, state: WebApiState) -> Result<thread::JoinHandle<()>> {
    let bind_addr = format!("{}:{}", config.bind_host, config.port);
    let server = Server::http(&bind_addr)
        .map_err(|e| anyhow::anyhow!("Webサーバー起動失敗 ({}): {}", bind_addr, e))?;

    info!("Web UI/API サーバー起動: http://{}/", bind_addr);

    let handle = thread::spawn(move || {
        for mut request in server.incoming_requests() {
            let method = request.method().clone();
            let url = request.url().to_string();

            let response = match (method, url.as_str()) {
                (Method::Get, "/") => html_response(index_html()),
                (Method::Get, "/api/health") => json_response(
                    StatusCode(200),
                    &HealthResponse {
                        status: "ok",
                        api: "ready",
                        webui: "ready",
                    },
                ),
                (Method::Post, "/api/chat") => handle_chat(&mut request, &state),
                _ => text_response(StatusCode(404), "Not Found"),
            };

            if let Err(e) = request.respond(response) {
                warn!("HTTPレスポンス送信失敗: {}", e);
            }
        }
    });

    Ok(handle)
}

fn handle_chat(
    request: &mut tiny_http::Request,
    state: &WebApiState,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let body = match read_body(request) {
        Ok(body) => body,
        Err(e) => {
            return json_error(StatusCode(400), &format!("不正なリクエスト: {}", e));
        }
    };

    let chat_req: ChatRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(e) => {
            return json_error(StatusCode(400), &format!("JSONパースエラー: {}", e));
        }
    };

    let text = chat_req.text.trim();
    if text.is_empty() {
        return json_error(StatusCode(400), "text は必須です");
    }

    let response_text = match build_response_text(text, state) {
        Ok(resp) => resp,
        Err(e) => {
            error!("API応答生成エラー: {}", e);
            return json_error(StatusCode(500), &format!("応答生成に失敗: {}", e));
        }
    };

    let audio_base64 = if chat_req.speak {
        match state.tts.synthesize(response_text.as_str()) {
            Ok(audio) => Some(base64::engine::general_purpose::STANDARD.encode(audio)),
            Err(e) => {
                warn!("TTS生成失敗 (テキストのみ返却): {}", e);
                None
            }
        }
    } else {
        None
    };

    json_response(
        StatusCode(200),
        &ChatResponse {
            response: response_text,
            audio_base64,
        },
    )
}

fn build_response_text(text: &str, state: &WebApiState) -> Result<String> {
    match detect_intent(text) {
        Intent::SaveFact(fact) => {
            let mut rag = state
                .rag
                .lock()
                .map_err(|_| anyhow::anyhow!("RAGロック取得失敗"))?;
            if let Some(ref mut engine) = *rag {
                engine.save_fact(&fact)?;
                Ok("覚えました。".to_string())
            } else {
                Ok("記憶機能は無効です。".to_string())
            }
        }
        Intent::Query => {
            let augmented_prompt = {
                let mut rag = state
                    .rag
                    .lock()
                    .map_err(|_| anyhow::anyhow!("RAGロック取得失敗"))?;
                if let Some(ref mut engine) = *rag {
                    match engine.retrieve_context(text) {
                        Ok(context) if !context.is_empty() => format!(
                            "以下の参考情報を踏まえて回答してください:\n\n{}\n\n質問: {}",
                            context, text
                        ),
                        Ok(_) => text.to_string(),
                        Err(e) => {
                            warn!("RAGコンテキスト取得失敗（RAGなしで続行）: {}", e);
                            text.to_string()
                        }
                    }
                } else {
                    text.to_string()
                }
            };

            state.llm.generate(&augmented_prompt)
        }
    }
}

fn read_body(request: &mut tiny_http::Request) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    request
        .as_reader()
        .take(MAX_REQUEST_BYTES as u64 + 1)
        .read_to_end(&mut body)
        .context("リクエスト本文の読み取りに失敗")?;

    if body.len() > MAX_REQUEST_BYTES {
        anyhow::bail!("リクエストサイズが上限を超えています");
    }

    Ok(body)
}

fn html_response(html: &'static str) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(html)
        .with_status_code(200)
        .with_header(content_type_header("text/html; charset=utf-8"))
}

fn text_response(status: StatusCode, body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(body.to_string())
        .with_status_code(status)
        .with_header(content_type_header("text/plain; charset=utf-8"))
}

fn json_response<T: Serialize>(
    status: StatusCode,
    payload: &T,
) -> Response<std::io::Cursor<Vec<u8>>> {
    match serde_json::to_vec(payload) {
        Ok(bytes) => Response::from_data(bytes)
            .with_status_code(status)
            .with_header(content_type_header("application/json; charset=utf-8")),
        Err(e) => text_response(StatusCode(500), &format!("JSONシリアライズエラー: {}", e)),
    }
}

fn json_error(status: StatusCode, message: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    #[derive(Serialize)]
    struct ErrorResponse<'a> {
        error: &'a str,
    }

    json_response(status, &ErrorResponse { error: message })
}

fn content_type_header(value: &str) -> Header {
    Header::from_bytes(b"Content-Type", value.as_bytes()).unwrap_or_else(|_| {
        Header::from_bytes(b"Content-Type", b"text/plain; charset=utf-8")
            .expect("Content-Type ヘッダ生成失敗")
    })
}

fn index_html() -> &'static str {
    r#"<!doctype html>
<html lang=\"ja\">
<head>
  <meta charset=\"UTF-8\" />
  <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\" />
  <title>Smart Speaker Host</title>
  <style>
    body { font-family: sans-serif; margin: 2rem; max-width: 800px; }
    textarea { width: 100%; min-height: 120px; }
    button { margin-top: 1rem; padding: 0.6rem 1rem; }
    .result { margin-top: 1.2rem; white-space: pre-wrap; background:#f4f4f4; padding:1rem; border-radius:8px; }
    .row { display:flex; gap:1rem; align-items:center; margin-top:0.6rem; }
  </style>
</head>
<body>
  <h1>Smart Speaker Host</h1>
  <p>ローカルネットワーク上のブラウザ/アプリから API を呼び出せます。</p>
  <textarea id=\"prompt\" placeholder=\"質問や指示を入力してください\"></textarea>
  <div class=\"row\">
    <label><input id=\"speak\" type=\"checkbox\" /> 音声データも返す</label>
    <button id=\"send\">送信</button>
  </div>
  <div id=\"result\" class=\"result\">ここに応答が表示されます。</div>
  <audio id=\"audio\" controls style=\"margin-top:1rem; width:100%; display:none;\"></audio>
  <script>
    const result = document.getElementById('result');
    const audio = document.getElementById('audio');
    document.getElementById('send').addEventListener('click', async () => {
      const text = document.getElementById('prompt').value.trim();
      const speak = document.getElementById('speak').checked;
      if (!text) {
        result.textContent = '入力してください。';
        return;
      }
      result.textContent = '処理中...';
      audio.style.display = 'none';
      audio.removeAttribute('src');

      try {
        const res = await fetch('/api/chat', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ text, speak }),
        });
        const json = await res.json();
        if (!res.ok) {
          result.textContent = `エラー: ${json.error ?? 'unknown'}`;
          return;
        }
        result.textContent = json.response;

        if (json.audio_base64) {
          audio.src = `data:audio/wav;base64,${json.audio_base64}`;
          audio.style.display = 'block';
        }
      } catch (e) {
        result.textContent = `通信エラー: ${e}`;
      }
    });
  </script>
</body>
</html>"#
}
