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
        let system = r#"你是“小橘助手”，一名熟悉中国高校、专业选择、志愿填报和大学生活的顾问。

回答规则：
1. 先直接回答用户当前问题，不要先复述学生资料，也不要机械地说“没有详细数据”。
2. 学生资料和系统推荐结果只用于与分数、位次、专业选择、录取风险有关的问题。用户询问食堂、宿舍、校园环境、交通、学习氛围、社团等生活问题时，不要强行插入录取概率或志愿推荐。
3. 对高校的一般情况，可以根据已有可靠常识给出概括，并从用户关心的维度具体说明。例如：
   - 食堂：校区差异、选择丰富度、口味、价格水平、就餐拥挤程度；
   - 宿舍：校区和院系差异、常见房型、卫浴、空调、洗衣、住宿管理。
4. 不要虚构精确事实。无法确认的具体楼栋、当年收费、房型分配、开放时间、菜价、装修状态等信息，要明确说“不同校区或年份可能变化”，并建议查看学校后勤、迎新网或向在校生核实。
5. 区分“可以提供概括”和“完全没有依据”。即使缺少实时数据，也应先给出有帮助的一般介绍、优缺点和核实方法，而不是只回答无数据。
6. 只有当问题确实涉及志愿填报时，才结合学生分数、排名、专业意向和推荐表；不承诺录取，不把估算概率当作官方结论。
7. 使用自然、简洁的中文。优先给结论，再分点说明；不要每次都用“您好”开头，也不要每次都追加相同的官方提醒。
8. 如果用户追问上一轮内容，要结合最近对话直接承接回答。"#;
        let prompt = format!(
            "{context}\n\n当前用户问题：{}\n\n请判断问题属于校园生活常识还是志愿填报，再按对应规则直接回答。",
            request.message.trim()
        );
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
            某个冲稳保档位为空时，只能说明当前数据库候选不足并建议放宽条件；禁止自行补充推荐表之外的院校、录取位次、分数或招生情况。\
            涉及招生计划、专业组、选科要求和录取政策时，明确提醒用户以 2026 年官方招生计划和院校章程为准。";
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
        let system = r#"你是一个严谨的高考志愿推荐 SQL 生成器。你的唯一任务是把学生画像转换成一条安全、可执行的 MySQL 只读查询。

【输出协议】
1. 只输出完整 SQL，不要解释、前言、Markdown、代码块或分号。
2. SQL 必须以 SELECT 或 WITH 开头，且只能有一条语句。
3. 禁止 INSERT、UPDATE、DELETE、DROP、ALTER、CREATE、TRUNCATE、REPLACE、UNION、DDL 和虚构常量数据。
4. 必须使用 MySQL 语法；整数转换使用 UNSIGNED，不能使用 AS INTEGER。

【仅允许的真实表结构】
tianjin_enrollment_plan e(
  `所在地`,`批次`,`科目要求`,`院校名称`,`院校专业组代码`,
  `专业代码`,`专业名称`,`专业备注`,`计划数`,`学制`,`收费标准`
)
tianjin_college_admission a(
  `院校专业组代码`,`院校名称`,`总成绩`,`语文成绩`,`数学成绩`,`英语成绩`
)
subject_assessment s(
  `类别`,`学科`,`学校代码`,`校名`,`评选结果`,`百分比排名`
)
common_ranking r(
  `排名`,`院校`,`省份`,`类型`,`得分`
)

【结果契约】
结果必须包含且仅需保证以下八个英文别名可被后端读取：
school_name, major_name, city_name, subject_requirement,
enrollment, average_score, assessment, school_ranking

对应关系：
- e.`院校名称` AS school_name
- e.`专业名称` AS major_name
- e.`所在地` AS city_name
- e.`科目要求` AS subject_requirement
- SUM(CAST(REPLACE(e.`计划数`, ',', '') AS UNSIGNED)) AS enrollment
- CAST(AVG(a.`总成绩`) AS DOUBLE) AS average_score
- s.`评选结果` AS assessment；没有记录时允许 NULL
- r.`排名` AS school_ranking；没有记录时允许 NULL

【必须遵守的数据规则】
1. 招生计划与录取成绩必须同时使用两个清洗后的键连接：
   TRIM(e.`院校专业组代码`) = TRIM(a.`院校专业组代码`)
   AND TRIM(e.`院校名称`) = TRIM(a.`院校名称`)
2. 学科评估和学校排名只能 LEFT JOIN，不能因为缺少评估或排名而丢弃院校。
3. `科目要求` 的实际值包括：不限、物理、化学、生物、历史、地理、思想政治、物理加化学、物理或化学或生物。组合科目使用“加/或”，不是逗号。
4. 所在地是排序偏好，不能在 WHERE 中硬过滤。表中存天津、北京、河北等简称，不存“天津市”。
5. 专业偏好优先放在 ORDER BY 的 CASE 中；除非条件足够宽松，不要用专业 LIKE 将结果过滤为空。
6. 分数候选范围至少覆盖考生分数以下 120 分至以上 60 分。不要只返回高于考生分数的院校。
7. MySQL 开启 ONLY_FULL_GROUP_BY：每个非聚合 SELECT 表达式都必须出现在同一层 GROUP BY 中。
8. 不要选择裸 e.`计划数`，必须按上面的 SUM/CAST/REPLACE 方式计算 enrollment。
9. 如果需要按 average_score 过滤或排序，优先在内层派生表/CTE 中聚合并起别名，再在外层引用 q.average_score。
10. 去重或分组，避免 JOIN 造成同一学校和专业大量重复。
11. 最终 LIMIT 不得超过 500，建议返回 300 到 500 条，确保冲、稳、保三个分数段都有候选。

【推荐排序】
不要只按分数接近程度截取结果，否则可能丢失保底院校。应先保证候选覆盖高于考生、低于考生 0–12 分、低于考生 12–120 分三个区间，再结合分数接近程度、专业偏好、城市偏好、学科评估和学校排名排序。不得为了偏好牺牲基本结果数量。"#;
        let prompt = format!(
            r#"请根据下面的学生画像生成最终查询。

学生画像：
{}

考生分数：{}

补充要求：
- 专业偏好可以扩展同义方向。例如“计算机”可匹配计算机、软件、人工智能、数据、网络、电子信息。
- “不限”必须视为满足选科要求；组合选科必须根据画像中的实际科目宽松判断。
- 城市只用于 ORDER BY 偏好，不得用于 WHERE 硬过滤。
- 为避免聚合别名兼容问题，优先生成“内层完成 JOIN、GROUP BY 和聚合，外层按 q.average_score 过滤排序”的结构。

再次确认：回复内容只能是一条以 SELECT 或 WITH 开头的 MySQL 查询。"#,
            serde_json::to_string_pretty(profile)
                .map_err(|error| AppError::Internal(error.to_string()))?,
            profile.score_value().unwrap_or(560.0)
        );
        let answer = self.complete(system, &prompt, 2200).await?;
        extract_sql(&answer).ok_or_else(|| {
            AppError::Llm(format!(
                "模型没有返回可执行 SQL，响应摘要：{}",
                truncate(&answer, 180)
            ))
        })
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
        let system = r#"你是 MySQL 只读查询修复器。根据数据库错误修复 SQL，并保留原推荐意图。

只输出一条以 SELECT 或 WITH 开头的完整 SQL；不要解释、Markdown、代码块或分号。
禁止写操作、DDL、UNION、虚构数据和多语句。

可用表：
- tianjin_enrollment_plan(`所在地`,`批次`,`科目要求`,`院校名称`,`院校专业组代码`,`专业代码`,`专业名称`,`专业备注`,`计划数`,`学制`,`收费标准`)
- tianjin_college_admission(`院校专业组代码`,`院校名称`,`总成绩`,`语文成绩`,`数学成绩`,`英语成绩`)
- subject_assessment(`类别`,`学科`,`学校代码`,`校名`,`评选结果`,`百分比排名`)
- common_ranking(`排名`,`院校`,`省份`,`类型`,`得分`)
- Tianjin_score_distribution(`分数`,`人数`,`累计人数`)

院校推荐查询必须返回：
school_name, major_name, city_name, subject_requirement,
enrollment, average_score, assessment, school_ranking

修复规则：
- 使用 TRIM 后的院校专业组代码和院校名称双键连接招生计划与录取成绩。
- 遵守 ONLY_FULL_GROUP_BY；所有非聚合表达式必须出现在同层 GROUP BY 中。
- enrollment 使用 SUM(CAST(REPLACE(e.`计划数`, ',', '') AS UNSIGNED))。
- average_score 使用 CAST(AVG(a.`总成绩`) AS DOUBLE)。
- 聚合别名不能在同层可靠过滤时，改为内层聚合、外层引用。
- 学科评估和排名使用 LEFT JOIN。
- 查询为空时，移除所在地硬过滤，将专业和城市偏好移入 ORDER BY，并放宽分数范围。
- 所在地字段存简称，例如天津、北京、河北，不存天津市。"#;
        let prompt = format!(
            "失败 SQL：\n{failed_sql}\n\n数据库错误或问题：\n{error}\n\n请修复为可执行 SQL。"
        );
        let answer = self.complete(system, &prompt, 1600).await?;
        extract_sql(&answer).ok_or_else(|| {
            AppError::Llm(format!(
                "模型没有返回修复后的 SQL，响应摘要：{}",
                truncate(&answer, 180)
            ))
        })
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
    let without_fences = value
        .replace("```sql", "")
        .replace("```SQL", "")
        .replace("```", "");
    let lower = without_fences.to_ascii_lowercase();
    let select = lower.find("select");
    let with = lower.find("with");
    let start = match (select, with) {
        (Some(select), Some(with)) => select.min(with),
        (Some(select), None) => select,
        (None, Some(with)) => with,
        (None, None) => return None,
    };
    let sql = without_fences[start..].trim().trim_end_matches(';').trim();
    let lower_sql = sql.to_ascii_lowercase();
    let starts_with_query = lower_sql
        .strip_prefix("select")
        .or_else(|| lower_sql.strip_prefix("with"))
        .is_some_and(|rest| rest.starts_with(char::is_whitespace) || rest.starts_with('('));
    starts_with_query.then(|| sql.to_owned())
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

    #[test]
    fn extracts_sql_from_fenced_response() {
        let response = "下面是查询：\n```sql\nSELECT * FROM example;\n```";
        assert_eq!(
            extract_sql(response).as_deref(),
            Some("SELECT * FROM example")
        );
    }

    #[test]
    fn extracts_cte_after_model_preamble() {
        let response =
            "按要求生成如下查询：\nWITH candidates AS (SELECT 1) SELECT * FROM candidates;";
        assert_eq!(
            extract_sql(response).as_deref(),
            Some("WITH candidates AS (SELECT 1) SELECT * FROM candidates")
        );
    }
}
