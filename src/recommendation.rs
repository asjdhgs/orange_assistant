use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use sqlx::{Column, Row};
use tracing::{info, warn};

use crate::{
    error::{AppError, AppResult},
    models::{AppState, RecommendationTable, SchoolRecommendation, StudentProfile},
};

#[derive(Debug, Clone)]
struct Candidate {
    school: String,
    major: String,
    city: String,
    enrollment: u32,
    average_score: f64,
    assessment: Option<String>,
    ranking: Option<u32>,
    subject_requirement: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Strategy {
    Major,
    School,
    City,
}

impl Strategy {
    fn parse(value: &str) -> Self {
        if value.starts_with("城市优先") {
            Self::City
        } else if value == "院校优先" {
            Self::School
        } else {
            Self::Major
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Weights {
    score: f64,
    assessment: f64,
    ranking: f64,
    enrollment: f64,
    city: f64,
    major: f64,
}

impl Weights {
    fn for_strategy(strategy: Strategy) -> Self {
        match strategy {
            Strategy::Major => Self {
                score: 0.20,
                assessment: 0.50,
                ranking: 0.10,
                enrollment: 0.10,
                city: 0.0,
                major: 0.10,
            },
            Strategy::School => Self {
                score: 0.20,
                assessment: 0.10,
                ranking: 0.50,
                enrollment: 0.10,
                city: 0.0,
                major: 0.10,
            },
            Strategy::City => Self {
                score: 0.20,
                assessment: 0.10,
                ranking: 0.10,
                enrollment: 0.10,
                city: 0.40,
                major: 0.10,
            },
        }
    }
}

#[derive(Debug, Clone)]
struct ScoredCandidate {
    candidate: Candidate,
    score: f64,
}

#[derive(Debug, Clone, Default)]
struct SchoolAggregate {
    enrollment: u32,
    weighted_score_sum: f64,
    weight_sum: f64,
    best_algorithm_score: f64,
}

pub async fn generate(
    state: &Arc<AppState>,
    profile: &StudentProfile,
) -> AppResult<RecommendationTable> {
    validate_profile(profile)?;
    let pool = state
        .databases
        .college
        .as_ref()
        .ok_or_else(|| AppError::Config("院校数据库不可用，无法生成推荐结果".into()))?;
    if !state.llm.is_configured() {
        return Err(AppError::Llm(
            "未配置 DEEPSEEK_API_KEY，无法由大模型生成查询 SQL".into(),
        ));
    }
    let sql = state.llm.recommendation_sql(profile).await?;
    info!(sql = %sql, "LLM generated recommendation SQL");
    validate_recommendation_sql(&sql)?;
    let candidates = match fetch_candidates(pool, profile, &sql).await {
        Ok(candidates) if !candidates.is_empty() => candidates,
        Ok(_) => {
            let hint = empty_recommendation_hint(profile);
            let fixed_sql = state.llm.repair_sql(&sql, &hint).await?;
            info!(sql = %fixed_sql, "LLM repaired empty recommendation SQL");
            validate_recommendation_sql(&fixed_sql)?;
            let fixed_candidates = fetch_candidates(pool, profile, &fixed_sql).await?;
            if fixed_candidates.is_empty() {
                let second_hint = strict_empty_recommendation_hint(profile);
                let second_sql = state.llm.repair_sql(&fixed_sql, &second_hint).await?;
                info!(sql = %second_sql, "LLM repaired empty recommendation SQL again");
                validate_recommendation_sql(&second_sql)?;
                fetch_candidates(pool, profile, &second_sql).await?
            } else {
                fixed_candidates
            }
        }
        Err(AppError::Database(error)) => {
            let fixed_sql = state.llm.repair_sql(&sql, &error).await?;
            info!(sql = %fixed_sql, "LLM repaired failed recommendation SQL");
            validate_recommendation_sql(&fixed_sql)?;
            fetch_candidates(pool, profile, &fixed_sql).await?
        }
        Err(error) => return Err(error),
    };
    if candidates.is_empty() {
        return Err(AppError::NotFound(
            "大模型生成的 SQL 未查询到匹配院校数据".into(),
        ));
    }
    let mut all_candidates = candidates;
    let mut table = rank_candidates(profile, all_candidates.clone());
    if table.safe.is_empty() {
        let hint = missing_safe_recommendation_hint(profile);
        match state.llm.repair_sql(&sql, &hint).await {
            Ok(safe_sql) => {
                info!(sql = %safe_sql, "LLM expanded recommendation SQL for safe tier");
                if let Err(error) = validate_recommendation_sql(&safe_sql) {
                    warn!(%error, "expanded safe-tier SQL failed validation");
                } else {
                    match fetch_candidates(pool, profile, &safe_sql).await {
                        Ok(safe_candidates) => {
                            all_candidates.extend(safe_candidates);
                            table = rank_candidates(profile, all_candidates);
                        }
                        Err(error) => {
                            warn!(%error, "expanded safe-tier SQL failed to fetch candidates");
                        }
                    }
                }
            }
            Err(error) => {
                warn!(%error, "failed to generate expanded safe-tier SQL");
            }
        }
    }
    Ok(table)
}

fn missing_safe_recommendation_hint(profile: &StudentProfile) -> String {
    let score = profile.score_value().unwrap_or(560.0);
    format!(
        "原 SQL 能查询到院校，但后端分档后“保”为空，说明候选池缺少明显低于考生分数的院校。\
         请重写为覆盖冲、稳、保三个分数区间的宽松候选 SQL，尤其必须返回足够的保底候选：\
         1. 保底候选 average_score 应主要覆盖 {safe_min:.0} 到 {safe_max:.0} 分，即低于考生约 15 到 120 分。\
         2. 不得在 WHERE 中使用所在地条件。\
         3. 不得用严格的专业名称 LIKE 排除其他相关专业；专业偏好只能在 ORDER BY 中加权。\
         4. 选科条件只保留“不限”或考生三门选科确实满足的要求。\
         5. 使用派生表先计算 average_score，再在外层覆盖完整分数范围。\
         6. LIMIT 使用 300 到 500，避免只返回最接近考生分数的一小批高分院校。\
         7. 仍须返回 school_name, major_name, city_name, subject_requirement, enrollment, average_score, assessment, school_ranking。\
         学生画像：{}",
        serde_json::to_string(profile).unwrap_or_default(),
        safe_min = (score - 120.0).max(0.0),
        safe_max = score - 15.0,
    )
}

fn empty_recommendation_hint(profile: &StudentProfile) -> String {
    format!(
        "SQL 执行成功但返回 0 行。必须重新生成更宽松的院校推荐 SQL，并遵守：\
         1. 不要把 live_city/所在地作为 WHERE 硬过滤；用户所在地是偏好，只能用于 ORDER BY 加权。\
         2. 数据表 tianjin_enrollment_plan.`所在地` 存的是省市简称，例如天津、北京、河北，不是天津市；如果要排序偏好，应使用 TRIM(e.`所在地`) LIKE '天津%'。\
         3. 不要要求 TRIM(e.`所在地`) = '{}'。\
         4. 专业偏好可以作为排序优先项，不要因为专业 LIKE 过严导致 0 行；至少保留计算机、软件、数据、电子信息、自动化等宽泛方向。\
         5. 分数范围至少覆盖学生分数下 120 分到上 60 分，必要时不要在 WHERE 中限制分数，只在 ORDER BY 中按 ABS(average_score - 用户分数) 排序。\
         6. 计划数必须用 SUM(CAST(REPLACE(e.`计划数`, ',', '') AS UNSIGNED)) 聚合。\
         用户画像：{}",
        profile.live_city.trim(),
        serde_json::to_string(profile).unwrap_or_default()
    )
}

fn strict_empty_recommendation_hint(profile: &StudentProfile) -> String {
    format!(
        "上一次修复后的 SQL 仍然返回 0 行。请大幅放宽：\
         必须移除所有 e.`所在地` / city_name 的 WHERE 条件；\
         必须移除 WHERE 中的专业名称 LIKE 组合，改为 ORDER BY 中优先排序；\
         不要在 WHERE 中限制 average_score，只按 ABS(average_score - {}) 排序；\
         只保留必要的 JOIN、选科可选条件和 LIMIT。\
         输出仍需包含 school_name, major_name, city_name, subject_requirement, enrollment, average_score, assessment, school_ranking。\
         用户画像：{}",
        profile.score_value().unwrap_or(560.0),
        serde_json::to_string(profile).unwrap_or_default()
    )
}

pub async fn score_distribution(
    state: &Arc<AppState>,
) -> AppResult<Vec<HashMap<String, serde_json::Value>>> {
    let pool = state
        .databases
        .scores
        .as_ref()
        .ok_or_else(|| AppError::Config("一分一段数据库不可用".into()))?;
    if !state.llm.is_configured() {
        return Err(AppError::Llm(
            "未配置 DEEPSEEK_API_KEY，无法由大模型生成一分一段查询 SQL".into(),
        ));
    }
    let sql = state.llm.score_distribution_sql().await?;
    validate_readonly_sql(&sql, &["Tianjin_score_distribution"])?;
    let rows = match sqlx::query(&sql).fetch_all(pool).await {
        Ok(rows) => rows,
        Err(error) => {
            let fixed_sql = state.llm.repair_sql(&sql, &error.to_string()).await?;
            validate_readonly_sql(&fixed_sql, &["Tianjin_score_distribution"])?;
            sqlx::query(&fixed_sql).fetch_all(pool).await?
        }
    };
    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
        let mut item = HashMap::new();
        for column in row.columns() {
            let name = column.name();
            let value = row
                .try_get::<String, _>(name)
                .map(serde_json::Value::String)
                .or_else(|_| {
                    row.try_get::<i64, _>(name)
                        .map(|value| serde_json::Value::Number(value.into()))
                })
                .unwrap_or(serde_json::Value::Null);
            item.insert(name.to_owned(), value);
        }
        result.push(item);
    }
    Ok(result)
}

fn validate_profile(profile: &StudentProfile) -> AppResult<()> {
    let score = profile
        .score_value()
        .ok_or_else(|| AppError::Validation("高考总分必须是数字".into()))?;
    if !(50.0..=750.0).contains(&score) {
        return Err(AppError::Validation("高考总分必须在 50 到 750 之间".into()));
    }
    if profile.rank_value().is_none() {
        return Err(AppError::Validation("全省排名必须是正整数".into()));
    }
    if profile.subject_list().len() != 3 {
        return Err(AppError::Validation("选考科目必须且只能选择 3 门".into()));
    }
    Ok(())
}

async fn fetch_candidates(
    pool: &sqlx::MySqlPool,
    profile: &StudentProfile,
    sql: &str,
) -> AppResult<Vec<Candidate>> {
    let rows = sqlx::query(sql).fetch_all(pool).await?;
    let raw_count = rows.len();

    let candidates = rows
        .into_iter()
        .filter_map(|row| {
            Some(Candidate {
                school: string_value(&row, &["school_name", "院校名称"], Some(0))?,
                major: string_value(&row, &["major_name", "专业名称"], Some(1))?,
                city: string_value(&row, &["city_name", "所在地"], Some(2)).unwrap_or_default(),
                enrollment: numeric_u32_any(
                    &row,
                    &["enrollment", "招生人数", "总招生人数"],
                    Some(3),
                ),
                average_score: numeric_f64_any(&row, &["average_score", "平均分"], Some(4)),
                assessment: string_value(&row, &["assessment", "学科评估"], Some(5)),
                ranking: optional_u32_any(&row, &["school_ranking", "学校排名"], Some(6)),
                subject_requirement: string_value(&row, &["subject_requirement", "科目要求"], None)
                    .unwrap_or_default(),
            })
        })
        .filter(|candidate| subject_matches(&candidate.subject_requirement, profile))
        .collect::<Vec<_>>();
    info!(
        raw_count,
        parsed_count = candidates.len(),
        "recommendation SQL rows parsed"
    );
    Ok(candidates)
}

fn rank_candidates(profile: &StudentProfile, candidates: Vec<Candidate>) -> RecommendationTable {
    let strategy = Strategy::parse(&profile.strategy);
    let weights = Weights::for_strategy(strategy);
    let user_score = profile.score_value().unwrap_or(500.0);
    let preferred_city = preferred_city(profile);
    let wanted_major = normalized_optional(&profile.want_major);
    let unwanted_major = normalized_optional(&profile.unwant_major);

    let mut scored: Vec<ScoredCandidate> = candidates
        .into_iter()
        .filter(|candidate| unwanted_major.is_empty() || !candidate.major.contains(&unwanted_major))
        .map(|candidate| {
            let score_match =
                (1.0 - (candidate.average_score - user_score).abs() / 80.0).clamp(0.0, 1.0);
            let assessment = assessment_score(candidate.assessment.as_deref());
            let ranking = (1.0 - candidate.ranking.unwrap_or(500) as f64 / 600.0).clamp(0.0, 1.0);
            let enrollment =
                ((candidate.enrollment.max(1) as f64 + 1.0).ln() / 100.0_f64.ln()).clamp(0.0, 1.0);
            let city = if preferred_city.is_empty() || !candidate.city.contains(&preferred_city) {
                0.0
            } else {
                1.0
            };
            let major = if wanted_major.is_empty() || candidate.major.contains(&wanted_major) {
                1.0
            } else {
                keyword_overlap(&candidate.major, &wanted_major)
            };
            let algorithm_score = score_match * weights.score
                + assessment * weights.assessment
                + ranking * weights.ranking
                + enrollment * weights.enrollment
                + city * weights.city
                + major * weights.major;
            ScoredCandidate {
                candidate,
                score: algorithm_score,
            }
        })
        .collect();
    scored.sort_by(|left, right| {
        right.score.total_cmp(&left.score).then_with(|| {
            (left.candidate.average_score - user_score)
                .abs()
                .total_cmp(&(right.candidate.average_score - user_score).abs())
        })
    });

    let grouped = aggregate_schools(scored);
    split_table(grouped, user_score)
}

fn aggregate_schools(scored: Vec<ScoredCandidate>) -> Vec<(String, SchoolAggregate)> {
    let mut grouped: HashMap<String, SchoolAggregate> = HashMap::new();
    for item in scored {
        let entry = grouped.entry(item.candidate.school).or_default();
        entry.enrollment = entry
            .enrollment
            .saturating_add(item.candidate.enrollment.max(1));
        let weight = item.candidate.enrollment.max(1) as f64;
        entry.weighted_score_sum += item.candidate.average_score * weight;
        entry.weight_sum += weight;
        entry.best_algorithm_score = entry.best_algorithm_score.max(item.score);
    }
    let mut result: Vec<_> = grouped.into_iter().collect();
    result.sort_by(|left, right| {
        right
            .1
            .best_algorithm_score
            .total_cmp(&left.1.best_algorithm_score)
    });
    result
}

fn split_table(grouped: Vec<(String, SchoolAggregate)>, user_score: f64) -> RecommendationTable {
    let mut table = RecommendationTable::default();
    let mut seen = HashSet::new();
    for (school, aggregate) in grouped {
        if !seen.insert(school.clone()) || aggregate.weight_sum <= 0.0 {
            continue;
        }
        let average = aggregate.weighted_score_sum / aggregate.weight_sum;
        let (category, mode) = if average > user_score {
            (&mut table.reach, AdmissionMode::Reach)
        } else if average >= user_score - 12.0 {
            (&mut table.match_list, AdmissionMode::Match)
        } else {
            (&mut table.safe, AdmissionMode::Safe)
        };
        let probability = admission_probability(user_score, average, aggregate.enrollment, mode);
        category.push(SchoolRecommendation {
            school_name: school,
            enrollment: aggregate.enrollment,
            average_score: round_one(average),
            probability: format!("{probability:.1}%"),
        });
    }
    table.reach.truncate(12);
    table.match_list.truncate(16);
    table.safe.truncate(12);
    table
}

#[derive(Debug, Clone, Copy)]
enum AdmissionMode {
    Reach,
    Match,
    Safe,
}

fn admission_probability(
    user_score: f64,
    average: f64,
    enrollment: u32,
    mode: AdmissionMode,
) -> f64 {
    let scale_bonus = (enrollment.max(1) as f64).ln() * 1.8;
    match mode {
        AdmissionMode::Reach => {
            (42.0 - (average - user_score) * 3.5 + scale_bonus).clamp(12.0, 55.0)
        }
        AdmissionMode::Match => {
            (62.0 + (user_score - average) * 2.6 + scale_bonus).clamp(52.0, 88.0)
        }
        AdmissionMode::Safe => {
            (87.0 + (user_score - average) * 0.35 + scale_bonus).clamp(78.0, 99.0)
        }
    }
}

fn assessment_score(value: Option<&str>) -> f64 {
    match value.unwrap_or("").trim().to_ascii_uppercase().as_str() {
        "A+" => 1.0,
        "A" => 0.92,
        "A-" => 0.84,
        "B+" => 0.74,
        "B" => 0.64,
        "B-" => 0.54,
        "C+" => 0.44,
        "C" => 0.36,
        _ => 0.28,
    }
}

fn preferred_city(profile: &StudentProfile) -> String {
    profile
        .strategy
        .split_once([':', '：'])
        .map(|(_, city)| city.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| profile.live_city.trim_end_matches(['市', '省']).to_owned())
}

fn normalized_optional(value: &str) -> String {
    let value = value.trim();
    if matches!(value, "" | "无" | "不限" | "暂无") {
        String::new()
    } else {
        value.to_owned()
    }
}

fn subject_matches(requirement: &str, profile: &StudentProfile) -> bool {
    let requirement = requirement.trim();
    if requirement.is_empty() || requirement.contains("不限") {
        return true;
    }
    let selected = profile.subject_list();
    if selected.is_empty() {
        return true;
    }
    let matches_selected = |required: &str| {
        selected
            .iter()
            .any(|subject| required.contains(subject) || subject.contains(required))
    };
    if requirement.contains('或') {
        return requirement
            .split('或')
            .map(str::trim)
            .filter(|subject| !subject.is_empty())
            .any(matches_selected);
    }
    requirement
        .split(['加', ',', '，', '/', '、', ' '])
        .map(str::trim)
        .filter(|subject| !subject.is_empty())
        .all(matches_selected)
}

fn keyword_overlap(candidate: &str, wanted: &str) -> f64 {
    let wanted_chars: HashSet<char> = wanted.chars().filter(|c| !c.is_whitespace()).collect();
    if wanted_chars.is_empty() {
        return 1.0;
    }
    let common = candidate
        .chars()
        .filter(|c| wanted_chars.contains(c))
        .collect::<HashSet<_>>()
        .len();
    common as f64 / wanted_chars.len() as f64
}

fn round_one(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn numeric_u32(row: &sqlx::mysql::MySqlRow, name: &str) -> u32 {
    row.try_get::<u64, _>(name)
        .map(|value| value.min(u32::MAX as u64) as u32)
        .or_else(|_| {
            row.try_get::<i64, _>(name)
                .map(|value| value.max(0).min(u32::MAX as i64) as u32)
        })
        .or_else(|_| {
            row.try_get::<String, _>(name)
                .map(|value| value.replace(',', "").parse().unwrap_or(0))
        })
        .unwrap_or(0)
}

fn numeric_u32_at(row: &sqlx::mysql::MySqlRow, index: usize) -> u32 {
    row.try_get::<u64, _>(index)
        .map(|value| value.min(u32::MAX as u64) as u32)
        .or_else(|_| {
            row.try_get::<i64, _>(index)
                .map(|value| value.max(0).min(u32::MAX as i64) as u32)
        })
        .or_else(|_| {
            row.try_get::<String, _>(index)
                .map(|value| value.replace(',', "").parse().unwrap_or(0))
        })
        .unwrap_or(0)
}

fn numeric_u32_any(row: &sqlx::mysql::MySqlRow, names: &[&str], index: Option<usize>) -> u32 {
    names
        .iter()
        .map(|name| numeric_u32(row, name))
        .chain(index.map(|index| numeric_u32_at(row, index)))
        .find(|value| *value > 0)
        .unwrap_or(0)
}

fn optional_u32_any(
    row: &sqlx::mysql::MySqlRow,
    names: &[&str],
    index: Option<usize>,
) -> Option<u32> {
    let value = numeric_u32_any(row, names, index);
    (value > 0).then_some(value)
}

fn numeric_f64(row: &sqlx::mysql::MySqlRow, name: &str) -> f64 {
    row.try_get::<f64, _>(name)
        .or_else(|_| row.try_get::<f32, _>(name).map(f64::from))
        .or_else(|_| {
            row.try_get::<String, _>(name)
                .map(|value| value.parse().unwrap_or(0.0))
        })
        .unwrap_or(0.0)
}

fn numeric_f64_at(row: &sqlx::mysql::MySqlRow, index: usize) -> f64 {
    row.try_get::<f64, _>(index)
        .or_else(|_| row.try_get::<f32, _>(index).map(f64::from))
        .or_else(|_| {
            row.try_get::<String, _>(index)
                .map(|value| value.parse().unwrap_or(0.0))
        })
        .unwrap_or(0.0)
}

fn numeric_f64_any(row: &sqlx::mysql::MySqlRow, names: &[&str], index: Option<usize>) -> f64 {
    names
        .iter()
        .map(|name| numeric_f64(row, name))
        .chain(index.map(|index| numeric_f64_at(row, index)))
        .find(|value| *value > 0.0)
        .unwrap_or(0.0)
}

fn string_value(
    row: &sqlx::mysql::MySqlRow,
    names: &[&str],
    index: Option<usize>,
) -> Option<String> {
    names
        .iter()
        .find_map(|name| row.try_get(*name).ok())
        .or_else(|| index.and_then(|index| row.try_get(index).ok()))
}

pub fn validate_readonly_sql(sql: &str, allowed_tables: &[&str]) -> AppResult<()> {
    let trimmed = sql.trim().trim_end_matches(';').trim();
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("select ") && !lower.starts_with("with ") {
        return Err(AppError::Validation(
            "大模型只能生成 SELECT/WITH 查询".into(),
        ));
    }
    let forbidden = [
        ";",
        "--",
        "/*",
        "*/",
        " insert ",
        " update ",
        " delete ",
        " drop ",
        " alter ",
        " create ",
        " truncate ",
        " replace ",
        " union ",
        " grant ",
        " revoke ",
        " load_file",
        " into outfile",
    ];
    if forbidden.iter().any(|token| lower.contains(token)) {
        return Err(AppError::Validation(
            "大模型生成的 SQL 包含不允许的操作".into(),
        ));
    }
    let mentions_allowed_table = allowed_tables
        .iter()
        .any(|table| lower.contains(&table.to_ascii_lowercase()));
    if !mentions_allowed_table {
        return Err(AppError::Validation(
            "大模型生成的 SQL 未访问允许的数据表".into(),
        ));
    }
    Ok(())
}

fn validate_recommendation_sql(sql: &str) -> AppResult<()> {
    validate_readonly_sql(
        sql,
        &[
            "tianjin_enrollment_plan",
            "tianjin_college_admission",
            "subject_assessment",
            "common_ranking",
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(score: &str, strategy: &str) -> StudentProfile {
        StudentProfile {
            score: score.into(),
            live_city: "天津市".into(),
            rank: "10000".into(),
            want_major: "计算机".into(),
            unwant_major: "无".into(),
            hobby: "编程".into(),
            future_goal: "软件工程师".into(),
            strategy: strategy.into(),
            subjects: "物理,化学,生物".into(),
        }
    }

    #[test]
    fn rejects_impossible_score() {
        assert!(validate_profile(&profile("900", "科目优先")).is_err());
    }

    #[test]
    fn requires_exactly_three_subjects() {
        let mut profile = profile("560", "科目优先");
        profile.subjects = "物理,化学".into();
        assert!(validate_profile(&profile).is_err());
        profile.subjects = "物理,化学,生物,历史".into();
        assert!(validate_profile(&profile).is_err());
    }

    #[test]
    fn produces_three_tiers() {
        let profile = profile("560", "院校优先");
        let candidates = vec![
            Candidate {
                school: "A大学".into(),
                major: "计算机科学与技术".into(),
                city: "天津".into(),
                enrollment: 20,
                average_score: 570.0,
                assessment: Some("B+".into()),
                ranking: Some(100),
                subject_requirement: "不限".into(),
            },
            Candidate {
                school: "B大学".into(),
                major: "软件工程".into(),
                city: "天津".into(),
                enrollment: 30,
                average_score: 555.0,
                assessment: Some("B".into()),
                ranking: Some(150),
                subject_requirement: "不限".into(),
            },
            Candidate {
                school: "C大学".into(),
                major: "数据科学".into(),
                city: "天津".into(),
                enrollment: 40,
                average_score: 530.0,
                assessment: Some("C+".into()),
                ranking: Some(200),
                subject_requirement: "不限".into(),
            },
        ];
        let table = rank_candidates(&profile, candidates);
        assert!(!table.is_empty());
        assert!(table.reach.iter().all(|item| item.average_score > 560.0));
        assert!(table.safe.iter().all(|item| item.average_score < 548.0));
    }

    #[test]
    fn rejects_dangerous_ai_sql() {
        assert!(validate_readonly_sql("DELETE FROM users", &["users"]).is_err());
        assert!(
            validate_readonly_sql(
                "SELECT * FROM tianjin_enrollment_plan LIMIT 1",
                &["tianjin_enrollment_plan"]
            )
            .is_ok()
        );
    }

    #[test]
    fn subject_requirement_is_subset_of_selected_subjects() {
        let profile = profile("560", "院校优先");
        assert!(subject_matches("物理加化学", &profile));
        assert!(subject_matches("历史或物理", &profile));
        assert!(subject_matches("不限", &profile));
        assert!(!subject_matches("历史加地理", &profile));
        assert!(!subject_matches("历史或地理", &profile));
    }

    #[test]
    fn probabilities_stay_bounded() {
        for mode in [
            AdmissionMode::Reach,
            AdmissionMode::Match,
            AdmissionMode::Safe,
        ] {
            let value = admission_probability(550.0, 560.0, 30, mode);
            assert!((0.0..=100.0).contains(&value));
        }
    }
}
