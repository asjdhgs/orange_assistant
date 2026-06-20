use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use sqlx::{Column, Row};

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
    let candidates = if let Some(pool) = &state.databases.college {
        match fetch_candidates(pool, profile).await {
            Ok(rows) if !rows.is_empty() => rows,
            Ok(_) | Err(_) => fallback_candidates(profile),
        }
    } else {
        fallback_candidates(profile)
    };
    Ok(rank_candidates(profile, candidates))
}

pub async fn score_distribution(
    state: &Arc<AppState>,
) -> AppResult<Vec<HashMap<String, serde_json::Value>>> {
    let Some(pool) = &state.databases.scores else {
        return Ok(fallback_score_distribution());
    };
    let rows = sqlx::query("SELECT * FROM Tianjin_score_distribution ORDER BY 1 DESC")
        .fetch_all(pool)
        .await?;
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
    if profile.subject_list().len() > 3 {
        return Err(AppError::Validation("选考科目不能超过 3 门".into()));
    }
    Ok(())
}

async fn fetch_candidates(
    pool: &sqlx::MySqlPool,
    profile: &StudentProfile,
) -> AppResult<Vec<Candidate>> {
    let major = normalized_optional(&profile.want_major);
    let unwanted = normalized_optional(&profile.unwant_major);
    let score = profile.score_value().unwrap_or_default();

    // 所有用户输入都通过绑定参数传入，避免 SQL 注入风险。
    let rows = sqlx::query(
        r#"
        SELECT
            e.院校名称 AS school_name,
            e.专业名称 AS major_name,
            e.所在地 AS city_name,
            e.科目要求 AS subject_requirement,
            SUM(CAST(REPLACE(e.计划数, ',', '') AS UNSIGNED)) AS enrollment,
            CAST(AVG(a.总成绩) AS DOUBLE) AS average_score,
            MAX(s.评选结果) AS assessment,
            MIN(CAST(r.排名 AS UNSIGNED)) AS school_ranking
        FROM tianjin_enrollment_plan e
        JOIN tianjin_college_admission a ON e.院校名称 = a.院校名称
        LEFT JOIN subject_assessment s
            ON e.院校名称 = s.校名
            AND (? = '' OR s.学科 LIKE CONCAT('%', ?, '%'))
        LEFT JOIN common_ranking r ON e.院校名称 = r.院校
        WHERE CAST(REPLACE(e.计划数, ',', '') AS UNSIGNED) > 0
          AND (? = '' OR e.专业名称 LIKE CONCAT('%', ?, '%'))
          AND (? = '' OR e.专业名称 NOT LIKE CONCAT('%', ?, '%'))
        GROUP BY e.院校名称, e.专业名称, e.所在地, e.科目要求
        HAVING average_score <= ? + 30
        ORDER BY ABS(average_score - ?) ASC
        LIMIT 500
        "#,
    )
    .bind(&major)
    .bind(&major)
    .bind(&major)
    .bind(&major)
    .bind(&unwanted)
    .bind(&unwanted)
    .bind(score)
    .bind(score)
    .fetch_all(pool)
    .await?;

    let candidates = rows
        .into_iter()
        .filter_map(|row| {
            Some(Candidate {
                school: row.try_get("school_name").ok()?,
                major: row.try_get("major_name").ok()?,
                city: row.try_get("city_name").unwrap_or_default(),
                enrollment: numeric_u32(&row, "enrollment"),
                average_score: numeric_f64(&row, "average_score"),
                assessment: row.try_get("assessment").ok(),
                ranking: optional_u32(&row, "school_ranking"),
                subject_requirement: row.try_get("subject_requirement").unwrap_or_default(),
            })
        })
        .filter(|candidate| subject_matches(&candidate.subject_requirement, profile))
        .collect();
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
        .filter(|candidate| candidate.average_score <= user_score + 30.0)
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
    if requirement.trim().is_empty() || requirement.contains("不限") {
        return true;
    }
    let selected = profile.subject_list();
    selected
        .iter()
        .all(|subject| requirement.contains(subject) || !requirement.contains('加'))
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

fn optional_u32(row: &sqlx::mysql::MySqlRow, name: &str) -> Option<u32> {
    let value = numeric_u32(row, name);
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

fn fallback_candidates(profile: &StudentProfile) -> Vec<Candidate> {
    let score = profile.score_value().unwrap_or(550.0);
    let wanted = normalized_optional(&profile.want_major);
    let major = if wanted.is_empty() {
        "计算机科学与技术".to_owned()
    } else {
        wanted
    };
    let seeds = [
        ("南开大学", "天津", 640.0, 20, "A", 20),
        ("天津大学", "天津", 635.0, 28, "A", 21),
        ("北京科技大学", "北京", 612.0, 24, "A-", 35),
        ("河北工业大学", "天津", 590.0, 42, "B+", 103),
        ("天津医科大学", "天津", 585.0, 30, "B+", 120),
        ("天津师范大学", "天津", 565.0, 55, "B", 150),
        ("天津工业大学", "天津", 560.0, 65, "B+", 145),
        ("天津财经大学", "天津", 552.0, 48, "B", 180),
        ("天津理工大学", "天津", 545.0, 72, "B-", 190),
        ("中国民航大学", "天津", 540.0, 80, "B", 170),
        ("天津科技大学", "天津", 532.0, 88, "B-", 210),
        ("天津商业大学", "天津", 520.0, 96, "C+", 240),
        ("天津城建大学", "天津", 510.0, 105, "C+", 260),
        ("河北科技大学", "石家庄", 505.0, 110, "C+", 250),
        ("山东科技大学", "青岛", 548.0, 75, "B", 125),
        ("燕山大学", "秦皇岛", 570.0, 60, "B+", 95),
        ("北京信息科技大学", "北京", 575.0, 45, "B", 140),
        ("大连交通大学", "大连", 530.0, 84, "B-", 230),
    ];
    seeds
        .into_iter()
        .filter(|(_, _, avg, _, _, _)| *avg <= score + 30.0)
        .map(
            |(school, city, average_score, enrollment, assessment, ranking)| Candidate {
                school: school.into(),
                major: major.clone(),
                city: city.into(),
                enrollment,
                average_score,
                assessment: Some(assessment.into()),
                ranking: Some(ranking),
                subject_requirement: "不限".into(),
            },
        )
        .collect()
}

fn fallback_score_distribution() -> Vec<HashMap<String, serde_json::Value>> {
    [
        (680, 72, 612),
        (670, 118, 1095),
        (660, 165, 1802),
        (650, 234, 2821),
        (640, 312, 4168),
        (630, 406, 5890),
        (620, 515, 8014),
        (610, 638, 10683),
        (600, 760, 13921),
        (590, 891, 17780),
        (580, 1012, 22245),
        (570, 1110, 27190),
        (560, 1204, 32510),
        (550, 1295, 38120),
        (540, 1360, 43950),
        (530, 1412, 49880),
        (520, 1450, 55870),
        (510, 1480, 61820),
        (500, 1510, 67640),
    ]
    .into_iter()
    .map(|(score, count, cumulative)| {
        HashMap::from([
            ("分数".into(), serde_json::json!(score)),
            ("本段人数".into(), serde_json::json!(count)),
            ("累计人数".into(), serde_json::json!(cumulative)),
        ])
    })
    .collect()
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
    fn produces_three_tiers() {
        let profile = profile("560", "院校优先");
        let table = rank_candidates(&profile, fallback_candidates(&profile));
        assert!(!table.is_empty());
        assert!(table.reach.iter().all(|item| item.average_score > 560.0));
        assert!(table.safe.iter().all(|item| item.average_score < 548.0));
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
