use std::{collections::HashMap, sync::Arc};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::{
    config::{AppConfig, Databases},
    knowledge::KnowledgeGraph,
    llm::LlmClient,
};

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub databases: Databases,
    pub graph: Arc<KnowledgeGraph>,
    pub llm: LlmClient,
    pub student: Arc<RwLock<Option<StudentProfile>>>,
    pub recommendation: Arc<RwLock<RecommendationTable>>,
    pub memory_users: Arc<RwLock<HashMap<String, String>>>,
    pub mbti_questions: Arc<RwLock<Vec<Vec<String>>>>,
    pub current_mbti: Arc<RwLock<Option<String>>>,
}

impl AppState {
    pub fn new(
        config: AppConfig,
        databases: Databases,
        graph: KnowledgeGraph,
        llm: LlmClient,
    ) -> Self {
        Self {
            config,
            databases,
            graph: Arc::new(graph),
            llm,
            student: Arc::new(RwLock::new(None)),
            recommendation: Arc::new(RwLock::new(RecommendationTable::default())),
            memory_users: Arc::new(RwLock::new(HashMap::new())),
            mbti_questions: Arc::new(RwLock::new(Vec::new())),
            current_mbti: Arc::new(RwLock::new(None)),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UserCredentials {
    pub phone_number: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StudentProfile {
    pub score: String,
    pub live_city: String,
    pub rank: String,
    pub want_major: String,
    pub unwant_major: String,
    pub hobby: String,
    pub future_goal: String,
    pub strategy: String,
    pub subjects: String,
}

impl StudentProfile {
    pub fn score_value(&self) -> Option<f64> {
        self.score.parse().ok()
    }

    pub fn rank_value(&self) -> Option<u32> {
        self.rank.parse().ok()
    }

    pub fn subject_list(&self) -> Vec<&str> {
        self.subjects
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .collect()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MbtiTypeRequest {
    pub mbti_type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MbtiChoiceRequest {
    pub operation: HashMap<String, u8>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TextRequest {
    pub text: String,
    #[serde(default)]
    #[serde(rename = "extra")]
    pub _extra: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default)]
    pub history: Vec<(String, String)>,
}

fn default_max_tokens() -> u32 {
    1000
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SchoolRecommendation {
    #[serde(rename = "院校名称")]
    pub school_name: String,
    #[serde(rename = "总招生人数")]
    pub enrollment: u32,
    #[serde(rename = "平均分")]
    pub average_score: f64,
    #[serde(rename = "录取概率")]
    pub probability: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct RecommendationTable {
    #[serde(rename = "冲")]
    pub reach: Vec<SchoolRecommendation>,
    #[serde(rename = "稳")]
    pub match_list: Vec<SchoolRecommendation>,
    #[serde(rename = "保")]
    pub safe: Vec<SchoolRecommendation>,
}

impl RecommendationTable {
    pub fn is_empty(&self) -> bool {
        self.reach.is_empty() && self.match_list.is_empty() && self.safe.is_empty()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiMessage<T: Serialize> {
    pub state: u16,
    pub message: T,
}

#[derive(Debug, Clone, Serialize)]
pub struct StudentUpdateResponse {
    pub status: &'static str,
    pub message: &'static str,
    pub data: StudentProfile,
}
