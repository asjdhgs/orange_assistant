use std::{env, net::IpAddr, str::FromStr};

use sqlx::{
    MySql, Pool,
    mysql::{MySqlConnectOptions, MySqlPoolOptions},
};

use crate::error::{AppError, AppResult};

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub server_host: String,
    pub server_port: u16,
    pub database_url: String,
    pub users_database_url: String,
    pub mbti_database_url: String,
    pub score_database_url: String,
    pub deepseek_api_key: Option<String>,
    pub deepseek_base_url: String,
    pub deepseek_model: String,
    pub admin_phone: String,
    pub admin_password: String,
    pub knowledge_graph_path: String,
    pub allow_origin: String,
}

#[derive(Clone, Default)]
pub struct Databases {
    pub college: Option<Pool<MySql>>,
    pub users: Option<Pool<MySql>>,
    pub mbti: Option<Pool<MySql>>,
    pub scores: Option<Pool<MySql>>,
    pub warnings: Vec<String>,
}

impl AppConfig {
    pub fn from_env() -> AppResult<Self> {
        let server_host = env_var("SERVER_HOST", "0.0.0.0");
        IpAddr::from_str(&server_host)
            .map_err(|_| AppError::Config(format!("SERVER_HOST 无效：{server_host}")))?;
        let server_port = env_var("SERVER_PORT", "8000")
            .parse::<u16>()
            .map_err(|_| AppError::Config("SERVER_PORT 必须是 1-65535 的端口号".into()))?;

        Ok(Self {
            server_host,
            server_port,
            database_url: env_var(
                "DATABASE_URL",
                "mysql://root:password@127.0.0.1:3306/tianjin",
            ),
            users_database_url: env_var(
                "USERS_DATABASE_URL",
                "mysql://root:password@127.0.0.1:3306/users",
            ),
            mbti_database_url: env_var(
                "MBTI_DATABASE_URL",
                "mysql://root:password@127.0.0.1:3306/mbti_careers",
            ),
            score_database_url: env_var(
                "SCORE_DATABASE_URL",
                "mysql://root:password@127.0.0.1:3306/score_distribution",
            ),
            deepseek_api_key: env::var("DEEPSEEK_API_KEY")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            deepseek_base_url: env_var("DEEPSEEK_BASE_URL", "https://api.deepseek.com"),
            deepseek_model: env_var("DEEPSEEK_MODEL", "deepseek-chat"),
            admin_phone: env_var("ADMIN_PHONE", "11000110001"),
            admin_password: env_var("ADMIN_PASSWORD", "1103"),
            knowledge_graph_path: env_var("KNOWLEDGE_GRAPH_PATH", "data/knowledge_graph.txt"),
            allow_origin: env_var("ALLOW_ORIGIN", "http://127.0.0.1:8000"),
        })
    }

    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.server_host, self.server_port)
    }

    pub async fn connect_databases(&self) -> Databases {
        let mut result = Databases::default();
        result.college =
            connect_optional("院校数据库", &self.database_url, &mut result.warnings).await;
        result.users =
            connect_optional("用户数据库", &self.users_database_url, &mut result.warnings).await;
        result.mbti =
            connect_optional("MBTI 数据库", &self.mbti_database_url, &mut result.warnings).await;
        result.scores = connect_optional(
            "一分一段数据库",
            &self.score_database_url,
            &mut result.warnings,
        )
        .await;
        result
    }
}

fn env_var(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_owned())
}

async fn connect_optional(
    label: &str,
    url: &str,
    warnings: &mut Vec<String>,
) -> Option<Pool<MySql>> {
    let options = match MySqlConnectOptions::from_str(url) {
        Ok(options) => options,
        Err(error) => {
            warnings.push(format!(
                "{label}连接字符串无效，将使用内置演示数据：{error}"
            ));
            return None;
        }
    };
    match MySqlPoolOptions::new()
        .min_connections(0)
        .max_connections(8)
        .acquire_timeout(std::time::Duration::from_secs(3))
        .connect_with(options)
        .await
    {
        Ok(pool) => Some(pool),
        Err(error) => {
            warnings.push(format!("{label}暂不可用，将使用内置演示数据：{error}"));
            None
        }
    }
}
