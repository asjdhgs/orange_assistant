use std::{convert::Infallible, sync::Arc, time::Duration};

use axum::{
    Json, Router,
    extract::State,
    http::{
        HeaderValue, Method,
        header::{ACCEPT, CONTENT_TYPE},
    },
    response::{
        IntoResponse, Sse,
        sse::{Event, KeepAlive},
    },
    routing::{get, post},
};
use futures_util::stream::{self, Stream};
use serde_json::{Value, json};
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};

use crate::{
    auth,
    error::{AppError, AppResult},
    frontend,
    llm::{chunk_text, fallback_chat, fallback_recommendation_summary},
    mbti,
    models::{
        AppState, ChatRequest, MbtiChoiceRequest, MbtiTypeRequest, StudentProfile,
        StudentUpdateResponse, TextRequest, UserCredentials,
    },
    portfolio, recommendation,
};

pub fn build_router(state: Arc<AppState>) -> Router {
    let origin = state
        .config
        .allow_origin
        .parse::<HeaderValue>()
        .unwrap_or_else(|_| HeaderValue::from_static("http://127.0.0.1:8000"));
    let cors = CorsLayer::new()
        .allow_origin([origin])
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([CONTENT_TYPE, ACCEPT])
        .allow_credentials(true);

    Router::new()
        .route("/", get(frontend::index))
        .route("/app.css", get(frontend::css))
        .route("/app.js", get(frontend::js))
        .nest_service("/assets", ServeDir::new("frontend/src"))
        .nest_service("/lib", ServeDir::new("frontend/lib"))
        .route("/api/orange/questions", post(questions))
        .route("/api/orange/result", post(mbti_result))
        .route("/api/orange/clear", post(clear_mbti))
        .route("/api/orange/seek", post(seek))
        .route("/api/orange/student", post(update_student))
        .route("/api/orange/getstudent", get(get_student))
        .route("/api/orange/smart_recommend", post(smart_recommend))
        .route("/api/orange/recommend_result", get(recommend_result))
        .route("/api/orange/recommend_summary", get(recommend_summary))
        .route("/api/orange/recommend_analysis", get(recommend_analysis))
        .route("/api/orange/register", post(register))
        .route("/api/orange/loader", post(login))
        .route("/api/orange/lookat", post(list_users))
        .route("/api/orange/chat/stream", post(chat_stream))
        .route("/api/orange/health", get(health))
        .route("/api/orange/score_distribution", get(score_distribution))
        .route("/api/orange/", post(root))
        .route("/process", post(process_major))
        .route("/get_dynamic_kg", post(dynamic_graph))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}

async fn questions(State(state): State<Arc<AppState>>) -> AppResult<Json<Value>> {
    let questions = mbti::load_questions(&state).await?;
    *state.mbti_questions.write().await = questions.clone();
    Ok(Json(json!({"question": questions})))
}

async fn mbti_result(
    State(state): State<Arc<AppState>>,
    Json(request): Json<MbtiChoiceRequest>,
) -> AppResult<Json<Value>> {
    let questions = {
        let loaded = state.mbti_questions.read().await;
        if loaded.is_empty() {
            drop(loaded);
            let questions = mbti::load_questions(&state).await?;
            *state.mbti_questions.write().await = questions.clone();
            questions
        } else {
            loaded.clone()
        }
    };
    let result = mbti::calculate_type(&questions, &request)?;
    *state.current_mbti.write().await = Some(result.clone());
    Ok(Json(json!(result)))
}

async fn clear_mbti(State(state): State<Arc<AppState>>) -> Json<Value> {
    *state.current_mbti.write().await = None;
    *state.mbti_questions.write().await = Vec::new();
    Json(json!({"message": "状态已清除"}))
}

async fn seek(
    State(state): State<Arc<AppState>>,
    Json(request): Json<MbtiTypeRequest>,
) -> AppResult<Json<Value>> {
    let description = mbti::career_recommendation(&state, &request.mbti_type).await?;
    *state.current_mbti.write().await = Some(request.mbti_type.to_ascii_uppercase());
    Ok(Json(json!({"description": description})))
}

async fn update_student(
    State(state): State<Arc<AppState>>,
    Json(profile): Json<StudentProfile>,
) -> AppResult<Json<StudentUpdateResponse>> {
    let _ = recommendation::generate(&state, &profile).await?;
    *state.student.write().await = Some(profile.clone());
    Ok(Json(StudentUpdateResponse {
        status: "success",
        message: "学生信息已更新",
        data: profile,
    }))
}

async fn get_student(State(state): State<Arc<AppState>>) -> AppResult<Json<String>> {
    let student = state.student.read().await;
    let content = serde_json::to_string_pretty(&*student)
        .map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(Json(content))
}

async fn smart_recommend(State(state): State<Arc<AppState>>) -> AppResult<Json<Value>> {
    let profile = state
        .student
        .read()
        .await
        .clone()
        .ok_or_else(|| AppError::Validation("请先提交学生信息".into()))?;
    let table = recommendation::generate(&state, &profile).await?;
    *state.recommendation.write().await = table.clone();
    Ok(Json(json!({
        "result": table,
        "time": chrono::Utc::now().timestamp_millis() as f64 / 1000.0
    })))
}

async fn recommend_result(State(state): State<Arc<AppState>>) -> AppResult<Json<String>> {
    let table = state.recommendation.read().await;
    if table.is_empty() {
        return Err(AppError::NotFound("推荐结果尚未生成".into()));
    }
    let value =
        serde_json::to_string(&*table).map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(Json(value))
}

async fn recommend_summary(State(state): State<Arc<AppState>>) -> AppResult<Json<Value>> {
    let table = state.recommendation.read().await.clone();
    if table.is_empty() {
        return Err(AppError::NotFound("推荐结果尚未生成".into()));
    }
    let student = state.student.read().await.clone();
    let summary = if state.llm.is_configured() {
        match state
            .llm
            .recommendation_summary(student.as_ref(), &table)
            .await
        {
            Ok(answer) => answer,
            Err(error) => format!(
                "## 模型调用失败\n\n{error}\n\n已检测到你配置了 DEEPSEEK_API_KEY，因此系统不会改用本地说明。请检查 API Key、模型名、代理网络或 DEEPSEEK_BASE_URL。"
            ),
        }
    } else {
        fallback_recommendation_summary(student.as_ref(), &table)
    };
    Ok(Json(json!({ "summary": summary })))
}

async fn recommend_analysis(State(state): State<Arc<AppState>>) -> AppResult<Json<Value>> {
    let table = state.recommendation.read().await.clone();
    if table.is_empty() {
        return Err(AppError::NotFound("推荐结果尚未生成".into()));
    }
    let student = state.student.read().await.clone();
    Ok(Json(json!(portfolio::analyze(&table, student.as_ref()))))
}

async fn register(
    State(state): State<Arc<AppState>>,
    Json(credentials): Json<UserCredentials>,
) -> AppResult<Json<crate::models::ApiMessage<String>>> {
    Ok(Json(auth::register(&state, credentials).await?))
}

async fn login(
    State(state): State<Arc<AppState>>,
    Json(credentials): Json<UserCredentials>,
) -> AppResult<Json<crate::models::ApiMessage<String>>> {
    Ok(Json(auth::login(&state, credentials).await?))
}

async fn list_users(
    State(state): State<Arc<AppState>>,
    Json(credentials): Json<UserCredentials>,
) -> AppResult<Json<Value>> {
    Ok(Json(auth::list_users(&state, credentials).await?))
}

async fn process_major(
    State(state): State<Arc<AppState>>,
    Json(request): Json<TextRequest>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let analysis = state.graph.analyze(&request.text);
    let answer = if state.llm.is_configured() {
        match state
            .llm
            .explain_major(&request.text, &analysis.matched_categories)
            .await
        {
            Ok(answer) => answer,
            Err(error) => format!(
                "模型调用失败：{error}\n\n已检测到你配置了 DEEPSEEK_API_KEY，因此系统不会改用本地说明。请检查 API Key、模型名、代理网络或 DEEPSEEK_BASE_URL。"
            ),
        }
    } else {
        state
            .graph
            .fallback_explanation(&request.text, &analysis.matched_categories)
    };
    sse_text(answer)
}

async fn dynamic_graph(
    State(state): State<Arc<AppState>>,
    Json(request): Json<TextRequest>,
) -> Json<Value> {
    let analysis = state.graph.analyze(&request.text);
    Json(json!({
        "kg_data": analysis.related_entities,
        "matched_categories": analysis.matched_categories
    }))
}

async fn chat_stream(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ChatRequest>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let student = state.student.read().await.clone();
    let recommendations = state.recommendation.read().await.clone();
    let answer = if state.llm.is_configured() {
        match state
            .llm
            .chat(&request, student.as_ref(), &recommendations)
            .await
        {
            Ok(answer) => answer,
            Err(error) => format!(
                "模型调用失败：{error}\n\n已检测到你配置了 DEEPSEEK_API_KEY，因此系统不会改用本地说明。请检查 API Key、模型名、代理网络或 DEEPSEEK_BASE_URL。"
            ),
        }
    } else {
        fallback_chat(&request, student.as_ref(), &recommendations)
    };
    sse_text(answer)
}

fn sse_text(text: String) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let chunks = chunk_text(&text, 24);
    let events = chunks
        .into_iter()
        .map(|content| {
            Ok(Event::default().data(
                json!({
                    "type": "content",
                    "content": content
                })
                .to_string(),
            ))
        })
        .chain(std::iter::once(Ok(
            Event::default().data(json!({"type": "end", "content": ""}).to_string())
        )));
    Sse::new(stream::iter(events)).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

async fn score_distribution(State(state): State<Arc<AppState>>) -> AppResult<Json<Value>> {
    let rows = recommendation::score_distribution(&state).await?;
    Ok(Json(json!({"rows": rows})))
}

async fn root() -> Json<Value> {
    Json(json!({"message": "Orange Assistant Rust API is running"}))
}

async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(json!({
        "status": "healthy",
        "service": "Orange Assistant Rust API",
        "version": env!("CARGO_PKG_VERSION"),
        "database": {
            "college": state.databases.college.is_some(),
            "users": state.databases.users.is_some(),
            "mbti": state.databases.mbti.is_some(),
            "scores": state.databases.scores.is_some()
        },
        "llm": state.llm.is_configured(),
        "knowledge_graph": {
            "entities": state.graph.entity_count(),
            "relations": state.graph.relation_count()
        }
    }))
}
