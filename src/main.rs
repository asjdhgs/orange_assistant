mod auth;
mod config;
mod error;
mod frontend;
mod knowledge;
mod llm;
mod mbti;
mod models;
mod portfolio;
mod recommendation;
mod routes;

use std::sync::Arc;

use tokio::net::TcpListener;
use tracing::{info, warn};

use crate::{
    config::AppConfig, knowledge::KnowledgeGraph, llm::LlmClient, models::AppState,
    routes::build_router,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "orange_assistant=info,tower_http=info".into()),
        )
        .init();

    let config = AppConfig::from_env()?;
    let databases = config.connect_databases().await;
    for warning in &databases.warnings {
        warn!("{warning}");
    }

    let graph = KnowledgeGraph::load(&config.knowledge_graph_path).await?;
    info!(
        entities = graph.entity_count(),
        relations = graph.relation_count(),
        "知识图谱加载完成"
    );

    let state = Arc::new(AppState::new(
        config.clone(),
        databases,
        graph,
        LlmClient::new(&config),
    ));
    let app = build_router(state);
    let address = config.bind_address();
    let listener = TcpListener::bind(&address).await?;
    info!("小橘助手 Rust 后端已启动：http://{address}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            warn!("无法安装 Ctrl+C 信号处理器：{error}");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => warn!("无法安装 terminate 信号处理器：{error}"),
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    info!("正在安全关闭服务");
}
