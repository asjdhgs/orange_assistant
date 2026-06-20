use serde::{Deserialize, Serialize};

use crate::{
    config::AppConfig,
    error::{AppError, AppResult},
    models::{ChatRequest, RecommendationTable, StudentProfile},
};

#[derive(Clone)]
pub struct LlmClient {
    client: reqwest::Client,
    api_key: Option<String>,
    endpoint: String,
    model: String,
}

#[derive(Debug, Clone, Serialize)]
struct CompletionRequest<'a> {
    model: &'a str,
    messages: Vec<Message<'a>>,
    temperature: f32,
    max_tokens: u32,
    stream: bool,
}

#[derive(Debug, Clone, Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Deserialize)]
struct CompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: CompletionMessage,
}

#[derive(Debug, Deserialize)]
struct CompletionMessage {
    content: Option<String>,
}

impl LlmClient {
    pub fn new(config: &AppConfig) -> Self {
        Self {
            client: reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(8))
                .timeout(std::time::Duration::from_secs(90))
                .build()
                .expect("reqwest client configuration is valid"),
            api_key: config.deepseek_api_key.clone(),
            endpoint: format!(
                "{}/chat/completions",
                config.deepseek_base_url.trim_end_matches('/')
            ),
            model: config.deepseek_model.clone(),
        }
    }

    pub fn is_configured(&self) -> bool {
        self.api_key.is_some()
    }

    pub async fn complete(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        max_tokens: u32,
    ) -> AppResult<String> {
        let api_key = self
            .api_key
            .as_deref()
            .ok_or_else(|| AppError::Llm("未配置 DEEPSEEK_API_KEY".into()))?;
        let body = CompletionRequest {
            model: &self.model,
            messages: vec![
                Message {
                    role: "system",
                    content: system_prompt,
                },
                Message {
                    role: "user",
                    content: user_prompt,
                },
            ],
            temperature: 0.35,
            max_tokens: max_tokens.clamp(64, 4096),
            stream: false,
        };
        let response = self
            .client
            .post(&self.endpoint)
            .bearer_auth(api_key)
            .json(&body)
            .send()
            .await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            return Err(AppError::Llm(format!(
                "模型服务返回 HTTP {}：{}",
                status.as_u16(),
                truncate(&text, 300)
            )));
        }
        let parsed: CompletionResponse = serde_json::from_str(&text)
            .map_err(|error| AppError::Llm(format!("无法解析模型响应：{error}")))?;
        parsed
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .filter(|content| !content.trim().is_empty())
            .ok_or_else(|| AppError::Llm("模型返回了空内容".into()))
    }

    pub async fn chat(
        &self,
        request: &ChatRequest,
        student: Option<&StudentProfile>,
        recommendations: &RecommendationTable,
    ) -> AppResult<String> {
        let context = build_context(student, recommendations, &request.history);
        let system = "角色：小橘助手，高考志愿填报顾问。\
            回答依据为学生资料和系统推荐结果；信息不足时直接说明。\
            不承诺录取结果，不补造学校数据，并提醒用户以官方招生章程为准。";
        let prompt = format!("{context}\n\n用户问题：{}", request.message.trim());
        self.complete(system, &prompt, request.max_tokens).await
    }

    pub async fn recommendation_summary(
        &self,
        student: Option<&StudentProfile>,
        recommendations: &RecommendationTable,
    ) -> AppResult<String> {
        let context = build_context(student, recommendations, &[]);
        let system = "角色：小橘助手，高考志愿填报顾问。\
            基于学生资料和推荐表生成志愿填报建议。\
            输出使用 Markdown 二级标题、短段落和项目符号列表。\
            不直接重复整张志愿表，不承诺录取，不补造学校数据。\
            结尾提醒用户以当年官方招生计划和院校章程为准。";
        let prompt = format!(
            "{context}\n\n输出结构：\n\
            ## 推荐结论\n\
            用 2-3 句话概括适合的志愿策略。\n\
            ## 冲稳保建议\n\
            分别说明冲、稳、保各应重点关注哪些学校以及原因。\n\
            ## 风险提醒\n\
            用要点列出位次波动、专业限制、城市偏好等风险。\n\
            ## 下一步建议\n\
            给出 3 条可执行建议。"
        );
        self.complete(system, &prompt, 1600).await
    }

    pub async fn explain_major(&self, query: &str, categories: &[String]) -> AppResult<String> {
        let system = "角色：大学专业探索顾问。用中文说明用户兴趣与专业类别的联系，\
            列举代表性专业、课程特点和可能的职业方向。内容仅作探索参考，避免绝对化判断。";
        let prompt = format!(
            "用户输入：{query}\n系统匹配的专业类别：{}\n请给出结构化但简洁的分析。",
            categories.join("、")
        );
        self.complete(system, &prompt, 1200).await
    }
}

fn build_context(
    student: Option<&StudentProfile>,
    recommendations: &RecommendationTable,
    history: &[(String, String)],
) -> String {
    let student_text = student
        .and_then(|profile| serde_json::to_string_pretty(profile).ok())
        .unwrap_or_else(|| "尚未提交学生资料".into());
    let recommendation_text = if recommendations.is_empty() {
        "尚未生成推荐表".into()
    } else {
        serde_json::to_string_pretty(recommendations).unwrap_or_else(|_| "推荐表序列化失败".into())
    };
    let history_text = history
        .iter()
        .rev()
        .take(6)
        .rev()
        .map(|(user, assistant)| format!("用户：{user}\n助手：{assistant}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "学生资料：\n{student_text}\n\n系统推荐结果：\n{recommendation_text}\n\n近期对话：\n{history_text}"
    )
}

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

pub fn chunk_text(text: &str, chunk_chars: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    for character in text.chars() {
        current.push(character);
        if current.chars().count() >= chunk_chars || matches!(character, '。' | '！' | '？' | '\n')
        {
            chunks.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

pub fn fallback_chat(
    request: &ChatRequest,
    student: Option<&StudentProfile>,
    recommendations: &RecommendationTable,
) -> String {
    let question = request.message.trim();
    if recommendations.is_empty() {
        return format!(
            "我收到你的问题“{question}”。目前还没有生成志愿推荐表，请先在“主页”填写分数、位次、选科和专业偏好并提交。完成后我可以结合冲、稳、保三档结果继续分析。"
        );
    }
    let score = student
        .and_then(StudentProfile::score_value)
        .map(|value| format!("{value:.0}"))
        .unwrap_or_else(|| "未提供".into());
    let reach = recommendations
        .reach
        .first()
        .map(|item| item.school_name.as_str())
        .unwrap_or("暂无");
    let match_school = recommendations
        .match_list
        .first()
        .map(|item| item.school_name.as_str())
        .unwrap_or("暂无");
    let safe = recommendations
        .safe
        .first()
        .map(|item| item.school_name.as_str())
        .unwrap_or("暂无");
    format!(
        "结合当前资料（高考总分 {score}），系统的代表性结果是：冲刺可关注 {reach}，稳妥可关注 {match_school}，保底可关注 {safe}。\n\n针对“{question}”，建议重点比较专业培养方案、选科要求、往年位次波动和城市成本。录取概率是辅助估算，最终请以当年官方招生计划与章程为准。"
    )
}

pub fn fallback_recommendation_summary(
    student: Option<&StudentProfile>,
    recommendations: &RecommendationTable,
) -> String {
    if recommendations.is_empty() {
        return "## 推荐结论\n\n当前还没有生成志愿推荐表，请先在主页填写学生信息并提交。".into();
    }
    let score = student
        .and_then(StudentProfile::score_value)
        .map(|value| format!("{value:.0}"))
        .unwrap_or_else(|| "未提供".into());
    let rank = student
        .and_then(StudentProfile::rank_value)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "未提供".into());
    let profile_text = student
        .map(|profile| {
            format!(
                "选科为 {}，偏好专业为 {}，策略为 {}",
                profile.subjects, profile.want_major, profile.strategy
            )
        })
        .unwrap_or_else(|| "学生偏好信息暂不完整".into());
    let reach = tier_names(&recommendations.reach);
    let match_list = tier_names(&recommendations.match_list);
    let safe = tier_names(&recommendations.safe);
    format!(
        "## 推荐结论\n\n根据当前资料，学生总分为 {score}，位次为 {rank}，{profile_text}。建议采用“稳妥为主、适度冲刺、保底兜底”的填报思路。\n\n\
        ## 冲稳保建议\n\n\
        - **冲一冲**：可重点关注 {reach}，这类院校有一定挑战性，适合放在前部志愿尝试。\n\
        - **稳一稳**：可重点关注 {match_list}，这类院校与当前分数和偏好匹配度更高，建议作为志愿表核心。\n\
        - **保一保**：可重点关注 {safe}，用于降低滑档风险，保障志愿组合安全性。\n\n\
        ## 风险提醒\n\n\
        - 录取概率是基于现有数据的估算，不能等同于最终录取结果。\n\
        - 需要继续核对目标专业的选科要求、招生人数和近年位次波动。\n\
        - 如果特别看重城市或专业，应适当牺牲部分院校层次，避免志愿目标过于集中。\n\n\
        ## 下一步建议\n\n\
        - 优先查看“稳一稳”院校的专业组和招生章程。\n\
        - 从“冲、稳、保”三档各保留若干备选，形成梯度。\n\
        - 最终填报前以当年官方招生计划和院校章程为准。"
    )
}

fn tier_names(items: &[crate::models::SchoolRecommendation]) -> String {
    let names = items
        .iter()
        .take(3)
        .map(|item| item.school_name.as_str())
        .collect::<Vec<_>>();
    if names.is_empty() {
        "暂无推荐院校".into()
    } else {
        names.join("、")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_unicode_without_breaking_characters() {
        let chunks = chunk_text("这是一个中文句子。第二句。", 4);
        assert_eq!(chunks.concat(), "这是一个中文句子。第二句。");
        assert!(chunks.iter().all(|chunk| !chunk.is_empty()));
    }
}
