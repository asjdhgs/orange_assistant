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

    pub async fn recommendation_sql(&self, profile: &StudentProfile) -> AppResult<String> {
        let system = "角色：高考志愿数据库 SQL 生成器。只能输出一条 MySQL SELECT/WITH 查询，不要 Markdown，不要解释，不要分号。\
            禁止 INSERT、UPDATE、DELETE、DROP、ALTER、CREATE、TRUNCATE、REPLACE 和多语句。\
            只能使用这些表和列：\
            tianjin_enrollment_plan e(`所在地`,`批次`,`科目要求`,`院校名称`,`院校专业组代码`,`专业代码`,`专业名称`,`专业备注`,`计划数`,`学制`,`收费标准`);\
            tianjin_college_admission a(`院校专业组代码`,`院校名称`,`总成绩`,`语文成绩`,`数学成绩`,`英语成绩`);\
            subject_assessment s(`类别`,`学科`,`学校代码`,`校名`,`评选结果`,`百分比排名`);\
            common_ranking r(`排名`,`院校`,`省份`,`类型`,`得分`)。\
            查询结果必须包含这些英文别名列：school_name, major_name, city_name, subject_requirement, enrollment, average_score, assessment, school_ranking；也可以按 Python 版固定列序返回：院校名称、专业名称、所在地、招生人数、平均分、学科评估、学校排名。\
            必须从数据库真实数据查询，不能输出虚构常量行，不能使用 UNION 构造预设数据。\
            必须用 TRIM(e.`院校专业组代码`) = TRIM(a.`院校专业组代码`) AND TRIM(e.`院校名称`) = TRIM(a.`院校名称`) 连接招生计划和录取成绩，录取成绩代码含尾随空格，裸等号可能返回 0 行。\
            `科目要求` 的实际值包含：不限、物理、化学、生物、历史、地理、思想政治、物理加化学、物理或化学或生物等；组合科目用“加/或”，不是逗号。\
            用户所在地只是偏好，不能在 WHERE 中硬过滤所在地；tianjin_enrollment_plan.`所在地` 存的是省市简称，例如天津、北京、河北，不是天津市。如需考虑所在地，只能在 ORDER BY 中用 CASE WHEN TRIM(e.`所在地`) LIKE '天津%' THEN 0 ELSE 1 END 加权。\
            专业偏好和分数范围都要避免过严；专业可以作为排序优先项，分数至少覆盖用户分数下 120 分到上 60 分。\
            如果使用 AVG/SUM/MIN/MAX 等聚合并需要按 average_score 过滤或排序，必须先在子查询中计算别名，再在外层引用别名；不要在同一层 HAVING 或 ORDER BY 中引用聚合别名。\
            正确形态示例：SELECT * FROM (SELECT ..., CAST(AVG(a.`总成绩`) AS DOUBLE) AS average_score FROM ... GROUP BY ...) q WHERE q.average_score <= 590 ORDER BY ABS(q.average_score - 560) LIMIT 500。\
            enrollment 必须使用 SUM(CAST(REPLACE(e.`计划数`, ',', '') AS UNSIGNED))，不要直接选择裸 e.`计划数`。\
            院校推荐 SQL 必须 LIMIT 500 以内。";
        let prompt = format!(
            "学生画像：{}\n\n\
            生成推荐候选 SQL。学生分数为 {:?}，建议覆盖学生分数以下 90 分到以上 35 分。\
            专业偏好可以用 LIKE 扩展同义方向，例如“计算机”可匹配 计算机、软件、人工智能、数据、网络、电子信息；如果条件过严会无结果，应适度放宽。\
            不要写 WHERE TRIM(e.`所在地`) = '天津市' 这类城市硬过滤；城市只作为排序偏好。\
            选科要求用 `科目要求` 过滤时要宽松：`不限` 一定可选，`物理加化学` 可由物理、化学、生物组合满足。\
            可 LEFT JOIN 学科评估和排名表增强排序，但不要因为评估表缺失而过滤掉院校。\
            输出只允许是一条 SELECT/WITH SQL。",
            serde_json::to_string_pretty(profile)
                .map_err(|error| AppError::Internal(error.to_string()))?,
            profile.score_value()
        );
        let answer = self.complete(system, &prompt, 1800).await?;
        extract_sql(&answer).ok_or_else(|| AppError::Llm("模型没有返回可执行 SQL".into()))
    }

    pub async fn score_distribution_sql(&self) -> AppResult<String> {
        let system = "角色：MySQL 查询生成器。只能输出一条 SELECT 查询，不要 Markdown，不要解释，不要分号。\
            可用表：Tianjin_score_distribution(`分数`,`人数`,`累计人数`)。\
            禁止任何写入、删除、DDL、UNION 构造常量数据或多语句。";
        let prompt = "生成查询天津一分一段表的 SQL，返回全部列，按分数从高到低排序。注意 `分数` 是 text，包含 '680 以上'，排序时不要使用不存在的 score 列，可用 CAST(REPLACE(`分数`, ' 以上', '') AS UNSIGNED) 排序。";
        let answer = self.complete(system, prompt, 500).await?;
        extract_sql(&answer).ok_or_else(|| AppError::Llm("模型没有返回一分一段查询 SQL".into()))
    }

    pub async fn repair_sql(&self, failed_sql: &str, error: &str) -> AppResult<String> {
        let system = "角色：MySQL SQL 修复器。只能输出修复后的一条 SELECT/WITH 查询，不要 Markdown，不要解释，不要分号。\
            禁止 INSERT、UPDATE、DELETE、DROP、ALTER、CREATE、TRUNCATE、REPLACE、UNION 构造常量数据和多语句。\
            可用院校表结构：\
            tianjin_enrollment_plan(`所在地`,`批次`,`科目要求`,`院校名称`,`院校专业组代码`,`专业代码`,`专业名称`,`专业备注`,`计划数`,`学制`,`收费标准`);\
            tianjin_college_admission(`院校专业组代码`,`院校名称`,`总成绩`,`语文成绩`,`数学成绩`,`英语成绩`);\
            subject_assessment(`类别`,`学科`,`学校代码`,`校名`,`评选结果`,`百分比排名`);\
            common_ranking(`排名`,`院校`,`省份`,`类型`,`得分`);\
            一分一段表：Tianjin_score_distribution(`分数`,`人数`,`累计人数`)。\
            院校推荐结果必须包含别名 school_name, major_name, city_name, subject_requirement, enrollment, average_score, assessment, school_ranking；也可以按 Python 版固定列序返回：院校名称、专业名称、所在地、招生人数、平均分、学科评估、学校排名。\
            若院校推荐返回 0 行，必须删除 WHERE 中的所在地硬过滤；用户所在地只能用于 ORDER BY 偏好，且数据表存天津、北京、河北等简称，不存天津市。然后放宽专业 LIKE 和分数范围。\
            计划数必须用 SUM(CAST(REPLACE(e.`计划数`, ',', '') AS UNSIGNED)) 聚合，不要选择裸 e.`计划数`。\
            若错误涉及聚合别名，例如 Reference 'average_score' not supported，请改成派生表：内层计算 average_score，外层 WHERE/ORDER BY 引用 q.average_score。";
        let prompt = format!(
            "失败 SQL：\n{failed_sql}\n\n数据库错误或问题：\n{error}\n\n请修复为可执行 SQL。"
        );
        let answer = self.complete(system, &prompt, 1600).await?;
        extract_sql(&answer).ok_or_else(|| AppError::Llm("模型没有返回修复后的 SQL".into()))
    }

    pub async fn mbti_questions_sql(&self) -> AppResult<String> {
        let system = "角色：MySQL 查询生成器。只能输出一条 SELECT 查询，不要 Markdown，不要解释，不要分号。\
            可用表：mbti_questions(question_text, option1, option2, dimension)。禁止任何写入、删除、DDL 或多语句。";
        let prompt = "生成随机抽取 40 道 MBTI 题目的 SQL，必须返回 question_text, option1, option2, dimension。";
        let answer = self.complete(system, prompt, 500).await?;
        extract_sql(&answer).ok_or_else(|| AppError::Llm("模型没有返回 MBTI 题目查询 SQL".into()))
    }

    pub async fn mbti_careers_sql(&self, mbti_type: &str) -> AppResult<String> {
        let system = "角色：MySQL 查询生成器。只能输出一条 SELECT 查询，不要 Markdown，不要解释，不要分号。\
            可用表：mbti_types(mbti_id,mbti_code), mbti_career_mapping(mbti_id,career_id,compatibility_score,is_core), careers(career_id,career_name,career_description)。\
            禁止任何写入、删除、DDL 或多语句。";
        let prompt = format!(
            "生成查询 MBTI 类型 {mbti_type} 对应职业推荐的 SQL，必须返回 career_name, career_description，按 compatibility_score 和 is_core 优先，最多 12 条。"
        );
        let answer = self.complete(system, &prompt, 700).await?;
        extract_sql(&answer).ok_or_else(|| AppError::Llm("模型没有返回 MBTI 职业查询 SQL".into()))
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

fn extract_sql(value: &str) -> Option<String> {
    let trimmed = value
        .trim()
        .trim_start_matches("```sql")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .trim_end_matches(';')
        .trim();
    let lower = trimmed.to_ascii_lowercase();
    (lower.starts_with("select ") || lower.starts_with("with ")).then(|| trimmed.to_owned())
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
