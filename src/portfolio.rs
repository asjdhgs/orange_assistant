use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::models::{RecommendationTable, SchoolRecommendation, StudentProfile};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Reach,
    Match,
    Safe,
}

impl Tier {
    pub fn chinese_name(self) -> &'static str {
        match self {
            Self::Reach => "冲",
            Self::Match => "稳",
            Self::Safe => "保",
        }
    }

    fn expected_probability_range(self) -> (f64, f64) {
        match self {
            Self::Reach => (0.10, 0.60),
            Self::Match => (0.50, 0.90),
            Self::Safe => (0.75, 0.995),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PortfolioAnalysis {
    pub summary: PortfolioSummary,
    pub balance: BalanceAnalysis,
    pub simulation: SimulationReport,
    pub sensitivity: Vec<SensitivityPoint>,
    pub school_diagnostics: Vec<SchoolDiagnostic>,
    pub warnings: Vec<String>,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PortfolioSummary {
    pub total_choices: usize,
    pub reach_choices: usize,
    pub match_choices: usize,
    pub safe_choices: usize,
    pub unique_schools: usize,
    pub average_probability: f64,
    pub expected_admissions: f64,
    pub at_least_one_probability: f64,
    pub estimated_score: Option<f64>,
    pub estimated_rank: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BalanceAnalysis {
    pub reach_ratio: f64,
    pub match_ratio: f64,
    pub safe_ratio: f64,
    pub target_reach_ratio: f64,
    pub target_match_ratio: f64,
    pub target_safe_ratio: f64,
    pub balance_score: f64,
    pub concentration_score: f64,
    pub diversity_score: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SimulationReport {
    pub iterations: usize,
    pub no_admission_probability: f64,
    pub one_or_more_probability: f64,
    pub two_or_more_probability: f64,
    pub median_admissions: usize,
    pub p10_admissions: usize,
    pub p90_admissions: usize,
    pub distribution: BTreeMap<usize, f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SensitivityPoint {
    pub score_change: i32,
    pub at_least_one_probability: f64,
    pub expected_admissions: f64,
    pub risk_level: RiskLevel,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Moderate,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize)]
pub struct SchoolDiagnostic {
    pub tier: Tier,
    pub school_name: String,
    pub average_score: f64,
    pub score_gap: Option<f64>,
    pub enrollment: u32,
    pub stated_probability: String,
    pub parsed_probability: f64,
    pub adjusted_probability: f64,
    pub confidence: f64,
    pub flags: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
struct TargetRatios {
    reach: f64,
    match_ratio: f64,
    safe: f64,
}

impl TargetRatios {
    fn from_profile(profile: Option<&StudentProfile>) -> Self {
        let Some(profile) = profile else {
            return Self {
                reach: 0.30,
                match_ratio: 0.45,
                safe: 0.25,
            };
        };
        let strategy = profile.strategy.as_str();
        if strategy.starts_with("院校优先") {
            Self {
                reach: 0.40,
                match_ratio: 0.40,
                safe: 0.20,
            }
        } else if strategy.starts_with("城市优先") {
            Self {
                reach: 0.25,
                match_ratio: 0.45,
                safe: 0.30,
            }
        } else {
            Self {
                reach: 0.30,
                match_ratio: 0.45,
                safe: 0.25,
            }
        }
    }
}

pub fn analyze(table: &RecommendationTable, profile: Option<&StudentProfile>) -> PortfolioAnalysis {
    let diagnostics = collect_diagnostics(table, profile);
    let summary = summarize(table, profile, &diagnostics);
    let target = TargetRatios::from_profile(profile);
    let balance = analyze_balance(table, target);
    let simulation = simulate(&diagnostics, 20_000, 0x5EED_0529);
    let sensitivity = analyze_sensitivity(table, profile, &diagnostics);
    let warnings = build_warnings(&summary, &balance, &simulation, &diagnostics);
    let suggestions = build_suggestions(&summary, &balance, &simulation, &diagnostics);

    PortfolioAnalysis {
        summary,
        balance,
        simulation,
        sensitivity,
        school_diagnostics: diagnostics,
        warnings,
        suggestions,
    }
}

fn collect_diagnostics(
    table: &RecommendationTable,
    profile: Option<&StudentProfile>,
) -> Vec<SchoolDiagnostic> {
    let user_score = profile.and_then(StudentProfile::score_value);
    let mut result = Vec::new();
    for (tier, schools) in [
        (Tier::Reach, table.reach.as_slice()),
        (Tier::Match, table.match_list.as_slice()),
        (Tier::Safe, table.safe.as_slice()),
    ] {
        for school in schools {
            let raw_probability = parse_probability(&school.probability)
                .unwrap_or_else(|| infer_probability(tier, user_score, school));
            let confidence = confidence_score(school, user_score, tier);
            let adjusted = shrink_probability(raw_probability, confidence, tier);
            let mut flags = Vec::new();
            let expected = tier.expected_probability_range();
            if raw_probability < expected.0 {
                flags.push(format!("{}档概率偏低，建议复核分档", tier.chinese_name()));
            }
            if raw_probability > expected.1 {
                flags.push(format!("{}档概率偏高，可能过于乐观", tier.chinese_name()));
            }
            if school.enrollment < 5 {
                flags.push("招生人数很少，年度波动风险较高".into());
            } else if school.enrollment < 15 {
                flags.push("招生规模偏小".into());
            }
            if let Some(score) = user_score {
                let gap = school.average_score - score;
                if tier == Tier::Reach && gap > 25.0 {
                    flags.push("院校均分高出考生分数超过 25 分".into());
                }
                if tier == Tier::Safe && gap > -5.0 {
                    flags.push("保底梯度不够明显".into());
                }
            }
            result.push(SchoolDiagnostic {
                tier,
                school_name: school.school_name.clone(),
                average_score: school.average_score,
                score_gap: user_score.map(|score| round_two(school.average_score - score)),
                enrollment: school.enrollment,
                stated_probability: school.probability.clone(),
                parsed_probability: round_four(raw_probability),
                adjusted_probability: round_four(adjusted),
                confidence: round_four(confidence),
                flags,
            });
        }
    }
    result
}

fn summarize(
    table: &RecommendationTable,
    profile: Option<&StudentProfile>,
    diagnostics: &[SchoolDiagnostic],
) -> PortfolioSummary {
    let total = diagnostics.len();
    let expected_admissions: f64 = diagnostics
        .iter()
        .map(|item| item.adjusted_probability)
        .sum();
    let no_admission = diagnostics
        .iter()
        .map(|item| 1.0 - item.adjusted_probability)
        .product::<f64>();
    let unique_schools = diagnostics
        .iter()
        .map(|item| item.school_name.as_str())
        .collect::<HashSet<_>>()
        .len();
    PortfolioSummary {
        total_choices: total,
        reach_choices: table.reach.len(),
        match_choices: table.match_list.len(),
        safe_choices: table.safe.len(),
        unique_schools,
        average_probability: if total == 0 {
            0.0
        } else {
            round_four(expected_admissions / total as f64)
        },
        expected_admissions: round_two(expected_admissions),
        at_least_one_probability: round_four(1.0 - no_admission),
        estimated_score: profile.and_then(StudentProfile::score_value),
        estimated_rank: profile.and_then(StudentProfile::rank_value),
    }
}

fn analyze_balance(table: &RecommendationTable, target: TargetRatios) -> BalanceAnalysis {
    let total = table.reach.len() + table.match_list.len() + table.safe.len();
    let divisor = total.max(1) as f64;
    let reach = table.reach.len() as f64 / divisor;
    let match_ratio = table.match_list.len() as f64 / divisor;
    let safe = table.safe.len() as f64 / divisor;
    let distance = (reach - target.reach).abs()
        + (match_ratio - target.match_ratio).abs()
        + (safe - target.safe).abs();
    let balance_score = (1.0 - distance / 2.0).clamp(0.0, 1.0);
    let concentration = reach.max(match_ratio).max(safe);
    let entropy = normalized_entropy(&[reach, match_ratio, safe]);
    BalanceAnalysis {
        reach_ratio: round_four(reach),
        match_ratio: round_four(match_ratio),
        safe_ratio: round_four(safe),
        target_reach_ratio: target.reach,
        target_match_ratio: target.match_ratio,
        target_safe_ratio: target.safe,
        balance_score: round_four(balance_score),
        concentration_score: round_four(concentration),
        diversity_score: round_four(entropy),
    }
}

fn simulate(diagnostics: &[SchoolDiagnostic], iterations: usize, seed: u64) -> SimulationReport {
    if diagnostics.is_empty() || iterations == 0 {
        return SimulationReport {
            iterations,
            no_admission_probability: 1.0,
            one_or_more_probability: 0.0,
            two_or_more_probability: 0.0,
            median_admissions: 0,
            p10_admissions: 0,
            p90_admissions: 0,
            distribution: BTreeMap::from([(0, 1.0)]),
        };
    }
    let mut rng = XorShift64::new(seed);
    let mut counts = Vec::with_capacity(iterations);
    let mut histogram: BTreeMap<usize, usize> = BTreeMap::new();
    for _ in 0..iterations {
        let shared_market_shock = (rng.next_f64() - 0.5) * 0.12;
        let mut admissions = 0;
        for item in diagnostics {
            let tier_volatility = match item.tier {
                Tier::Reach => 0.10,
                Tier::Match => 0.07,
                Tier::Safe => 0.05,
            };
            let school_noise = (rng.next_f64() - 0.5) * tier_volatility;
            let adjusted = (item.adjusted_probability + shared_market_shock + school_noise)
                .clamp(0.005, 0.995);
            if rng.next_f64() <= adjusted {
                admissions += 1;
            }
        }
        *histogram.entry(admissions).or_default() += 1;
        counts.push(admissions);
    }
    counts.sort_unstable();
    let no_admission = histogram.get(&0).copied().unwrap_or(0) as f64 / iterations as f64;
    let one_or_more = 1.0 - no_admission;
    let zero_or_one =
        histogram.get(&0).copied().unwrap_or(0) + histogram.get(&1).copied().unwrap_or(0);
    let distribution = histogram
        .into_iter()
        .map(|(count, frequency)| (count, round_four(frequency as f64 / iterations as f64)))
        .collect();
    SimulationReport {
        iterations,
        no_admission_probability: round_four(no_admission),
        one_or_more_probability: round_four(one_or_more),
        two_or_more_probability: round_four(1.0 - zero_or_one as f64 / iterations as f64),
        median_admissions: percentile(&counts, 0.50),
        p10_admissions: percentile(&counts, 0.10),
        p90_admissions: percentile(&counts, 0.90),
        distribution,
    }
}

fn analyze_sensitivity(
    table: &RecommendationTable,
    profile: Option<&StudentProfile>,
    baseline: &[SchoolDiagnostic],
) -> Vec<SensitivityPoint> {
    let Some(profile) = profile else {
        return Vec::new();
    };
    let Some(base_score) = profile.score_value() else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for score_change in [-20, -15, -10, -5, 0, 5, 10, 15, 20] {
        let changed_score = base_score + score_change as f64;
        let probabilities: Vec<f64> = baseline
            .iter()
            .map(|diagnostic| {
                let school = find_school(table, diagnostic.tier, &diagnostic.school_name)
                    .expect("diagnostic came from recommendation table");
                let inferred = infer_probability(diagnostic.tier, Some(changed_score), school);
                let blended = inferred * 0.65 + diagnostic.parsed_probability * 0.35;
                shrink_probability(blended, diagnostic.confidence, diagnostic.tier)
            })
            .collect();
        let expected: f64 = probabilities.iter().sum();
        let at_least_one = 1.0
            - probabilities
                .iter()
                .map(|probability| 1.0 - probability)
                .product::<f64>();
        result.push(SensitivityPoint {
            score_change,
            at_least_one_probability: round_four(at_least_one),
            expected_admissions: round_two(expected),
            risk_level: classify_risk(at_least_one),
        });
    }
    result
}

fn find_school<'a>(
    table: &'a RecommendationTable,
    tier: Tier,
    name: &str,
) -> Option<&'a SchoolRecommendation> {
    let source = match tier {
        Tier::Reach => &table.reach,
        Tier::Match => &table.match_list,
        Tier::Safe => &table.safe,
    };
    source.iter().find(|school| school.school_name == name)
}

fn build_warnings(
    summary: &PortfolioSummary,
    balance: &BalanceAnalysis,
    simulation: &SimulationReport,
    diagnostics: &[SchoolDiagnostic],
) -> Vec<String> {
    let mut warnings = Vec::new();
    if summary.total_choices == 0 {
        warnings.push("当前志愿组合为空，无法进行风险评估".into());
        return warnings;
    }
    if summary.safe_choices == 0 {
        warnings.push("组合中没有保底院校，存在明显滑档风险".into());
    }
    if summary.match_choices == 0 {
        warnings.push("组合中没有稳妥院校，梯度衔接不足".into());
    }
    if balance.reach_ratio > 0.50 {
        warnings.push("冲刺院校占比超过 50%，组合偏激进".into());
    }
    if balance.safe_ratio > 0.55 {
        warnings.push("保底院校占比过高，可能牺牲了院校层次".into());
    }
    if balance.balance_score < 0.60 {
        warnings.push("冲稳保比例与当前策略的建议比例偏差较大".into());
    }
    if simulation.no_admission_probability > 0.10 {
        warnings.push(format!(
            "模拟中全部未录取概率约为 {:.1}%，建议增加可靠保底项",
            simulation.no_admission_probability * 100.0
        ));
    }
    let small_enrollment = diagnostics
        .iter()
        .filter(|item| item.enrollment < 10)
        .count();
    if small_enrollment >= 3 {
        warnings.push(format!(
            "有 {small_enrollment} 个选项招生规模小于 10 人，年度波动可能较大"
        ));
    }
    let low_confidence = diagnostics
        .iter()
        .filter(|item| item.confidence < 0.55)
        .count();
    if low_confidence > diagnostics.len() / 3 {
        warnings.push("较多院校数据置信度偏低，请结合最新官方位次表复核".into());
    }
    warnings
}

fn build_suggestions(
    summary: &PortfolioSummary,
    balance: &BalanceAnalysis,
    simulation: &SimulationReport,
    diagnostics: &[SchoolDiagnostic],
) -> Vec<String> {
    let mut suggestions = Vec::new();
    if summary.total_choices < 8 {
        suggestions.push("建议至少准备 8 个有明显梯度的院校选项".into());
    }
    if summary.safe_choices < 2 {
        suggestions.push("建议补充 2 所以上、往年位次明显低于考生位次的保底院校".into());
    }
    if balance.reach_ratio < balance.target_reach_ratio - 0.12 {
        suggestions.push("当前组合较保守，可在可接受范围内增加 1–2 所冲刺院校".into());
    }
    if balance.reach_ratio > balance.target_reach_ratio + 0.12 {
        suggestions.push("减少部分高分差冲刺项，并替换为近三年位次更稳定的院校".into());
    }
    if simulation.one_or_more_probability < 0.95 {
        suggestions.push("优先提升整体录取覆盖率，再优化院校层次".into());
    }
    if diagnostics
        .iter()
        .any(|item| item.tier == Tier::Safe && item.score_gap.unwrap_or(0.0) > -8.0)
    {
        suggestions.push("至少保留一所平均分低于考生分数 10–20 分的真实保底院校".into());
    }
    if suggestions.is_empty() {
        suggestions.push("当前组合梯度较均衡，下一步重点核对专业调剂、体检和单科限制".into());
    }
    suggestions
}

fn parse_probability(text: &str) -> Option<f64> {
    let cleaned = text.trim().trim_end_matches('%');
    let value = cleaned.parse::<f64>().ok()?;
    let normalized = if text.contains('%') || value > 1.0 {
        value / 100.0
    } else {
        value
    };
    normalized.is_finite().then(|| normalized.clamp(0.0, 1.0))
}

fn infer_probability(tier: Tier, user_score: Option<f64>, school: &SchoolRecommendation) -> f64 {
    let enrollment_bonus = (school.enrollment.max(1) as f64).ln() * 0.008;
    let base = match (user_score, tier) {
        (Some(score), Tier::Reach) => 0.42 - (school.average_score - score).max(0.0) * 0.025,
        (Some(score), Tier::Match) => 0.62 + (score - school.average_score) * 0.018,
        (Some(score), Tier::Safe) => 0.84 + (score - school.average_score) * 0.008,
        (None, Tier::Reach) => 0.36,
        (None, Tier::Match) => 0.68,
        (None, Tier::Safe) => 0.90,
    };
    (base + enrollment_bonus).clamp(0.03, 0.99)
}

fn confidence_score(school: &SchoolRecommendation, user_score: Option<f64>, tier: Tier) -> f64 {
    let enrollment_component =
        ((school.enrollment.max(1) as f64).ln() / 100.0_f64.ln()).clamp(0.0, 1.0);
    let probability_component = parse_probability(&school.probability)
        .map(|_| 1.0)
        .unwrap_or(0.35);
    let score_component = user_score
        .map(|score| {
            let gap = (school.average_score - score).abs();
            (1.0 - gap / 80.0).clamp(0.1, 1.0)
        })
        .unwrap_or(0.45);
    let tier_component = match tier {
        Tier::Reach => 0.70,
        Tier::Match => 0.85,
        Tier::Safe => 0.80,
    };
    (enrollment_component * 0.35
        + probability_component * 0.25
        + score_component * 0.25
        + tier_component * 0.15)
        .clamp(0.1, 1.0)
}

fn shrink_probability(raw: f64, confidence: f64, tier: Tier) -> f64 {
    let prior = match tier {
        Tier::Reach => 0.35,
        Tier::Match => 0.68,
        Tier::Safe => 0.88,
    };
    (raw * confidence + prior * (1.0 - confidence)).clamp(0.005, 0.995)
}

fn normalized_entropy(probabilities: &[f64]) -> f64 {
    let positive: Vec<f64> = probabilities
        .iter()
        .copied()
        .filter(|value| *value > 0.0)
        .collect();
    if positive.len() <= 1 {
        return 0.0;
    }
    let entropy = -positive.iter().map(|value| value * value.ln()).sum::<f64>();
    entropy / (positive.len() as f64).ln()
}

fn percentile(values: &[usize], probability: f64) -> usize {
    if values.is_empty() {
        return 0;
    }
    let index = ((values.len() - 1) as f64 * probability.clamp(0.0, 1.0)).round() as usize;
    values[index]
}

fn classify_risk(probability: f64) -> RiskLevel {
    if probability >= 0.97 {
        RiskLevel::Low
    } else if probability >= 0.90 {
        RiskLevel::Moderate
    } else if probability >= 0.75 {
        RiskLevel::High
    } else {
        RiskLevel::Critical
    }
}

fn round_two(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn round_four(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

#[derive(Debug, Clone)]
struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut value = self.state;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.state = value;
        value
    }

    fn next_f64(&mut self) -> f64 {
        let bits = self.next_u64() >> 11;
        bits as f64 / ((1_u64 << 53) as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn school(name: &str, score: f64, probability: &str) -> SchoolRecommendation {
        SchoolRecommendation {
            school_name: name.into(),
            enrollment: 30,
            average_score: score,
            probability: probability.into(),
        }
    }

    fn test_table() -> RecommendationTable {
        RecommendationTable {
            reach: vec![school("A大学", 575.0, "42.0%")],
            match_list: vec![
                school("B大学", 557.0, "70.0%"),
                school("C大学", 551.0, "78.0%"),
            ],
            safe: vec![school("D大学", 530.0, "94.0%")],
        }
    }

    fn test_profile() -> StudentProfile {
        StudentProfile {
            score: "560".into(),
            live_city: "天津市".into(),
            rank: "12000".into(),
            want_major: "计算机".into(),
            unwant_major: "无".into(),
            hobby: "编程".into(),
            future_goal: "工程师".into(),
            strategy: "科目优先".into(),
            subjects: "物理,化学,生物".into(),
        }
    }

    #[test]
    fn parses_percentages() {
        assert_eq!(parse_probability("75%"), Some(0.75));
        assert_eq!(parse_probability("0.75"), Some(0.75));
        assert_eq!(parse_probability("bad"), None);
    }

    #[test]
    fn analysis_has_expected_counts() {
        let table = test_table();
        let profile = test_profile();
        let analysis = analyze(&table, Some(&profile));
        assert_eq!(analysis.summary.total_choices, 4);
        assert_eq!(analysis.summary.unique_schools, 4);
        assert_eq!(analysis.school_diagnostics.len(), 4);
        assert_eq!(analysis.sensitivity.len(), 9);
    }

    #[test]
    fn simulation_is_reproducible() {
        let table = test_table();
        let profile = test_profile();
        let diagnostics = collect_diagnostics(&table, Some(&profile));
        let left = simulate(&diagnostics, 1000, 42);
        let right = simulate(&diagnostics, 1000, 42);
        assert_eq!(
            left.no_admission_probability,
            right.no_admission_probability
        );
        assert_eq!(left.distribution, right.distribution);
    }

    #[test]
    fn empty_portfolio_is_critical() {
        let analysis = analyze(&RecommendationTable::default(), None);
        assert_eq!(analysis.summary.total_choices, 0);
        assert_eq!(analysis.simulation.no_admission_probability, 1.0);
        assert!(!analysis.warnings.is_empty());
    }

    #[test]
    fn entropy_rewards_balance() {
        let balanced = normalized_entropy(&[0.33, 0.34, 0.33]);
        let concentrated = normalized_entropy(&[0.90, 0.05, 0.05]);
        assert!(balanced > concentrated);
        assert!(balanced > 0.99);
    }
}
