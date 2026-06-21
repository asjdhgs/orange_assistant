use std::{collections::HashMap, sync::Arc};

use serde::Serialize;
use sqlx::Row;

use crate::{
    error::{AppError, AppResult},
    models::{AppState, MbtiChoiceRequest},
    recommendation::validate_readonly_sql,
};

#[derive(Debug, Clone, Serialize)]
pub struct MbtiQuestion {
    pub question: String,
    pub option_a: String,
    pub option_b: String,
    pub dimension: String,
}

impl MbtiQuestion {
    fn as_frontend_row(&self) -> Vec<String> {
        vec![
            self.question.clone(),
            self.option_a.clone(),
            self.option_b.clone(),
            self.dimension.clone(),
        ]
    }
}

pub async fn load_questions(state: &Arc<AppState>) -> AppResult<Vec<Vec<String>>> {
    let pool = state
        .databases
        .mbti
        .as_ref()
        .ok_or_else(|| AppError::Config("MBTI 数据库不可用，无法加载题目".into()))?;
    if !state.llm.is_configured() {
        return Err(AppError::Llm(
            "未配置 DEEPSEEK_API_KEY，无法由大模型生成 MBTI 题目查询 SQL".into(),
        ));
    }
    let sql = state.llm.mbti_questions_sql().await?;
    validate_readonly_sql(&sql, &["mbti_questions"])?;
    let rows = sqlx::query(&sql).fetch_all(pool).await?;
    let questions = rows
        .into_iter()
        .filter_map(|row| {
            Some(MbtiQuestion {
                question: row.try_get("question_text").ok()?,
                option_a: row.try_get("option1").ok()?,
                option_b: row.try_get("option2").ok()?,
                dimension: row.try_get("dimension").ok()?,
            })
        })
        .collect::<Vec<_>>();
    if questions.len() < 40 {
        return Err(AppError::Config("MBTI 数据库题目不足 40 条".into()));
    }
    Ok(questions
        .into_iter()
        .map(|question| question.as_frontend_row())
        .collect())
}

pub fn calculate_type(questions: &[Vec<String>], request: &MbtiChoiceRequest) -> AppResult<String> {
    if questions.len() < 4 {
        return Err(AppError::Internal("MBTI 题库未正确加载".into()));
    }
    let mut scores: HashMap<char, i32> = [('E', 0), ('S', 0), ('F', 0), ('J', 0)]
        .into_iter()
        .collect();

    for (index_text, choice) in &request.operation {
        let index = index_text
            .parse::<usize>()
            .map_err(|_| AppError::Validation(format!("无效题号：{index_text}")))?;
        let question = questions
            .get(index)
            .ok_or_else(|| AppError::Validation(format!("题号超出范围：{index}")))?;
        if !matches!(choice, 1 | 2) {
            return Err(AppError::Validation(format!(
                "第 {} 题只能选择选项 1 或 2",
                index + 1
            )));
        }
        let dimension = question
            .get(3)
            .and_then(|value| value.chars().next())
            .ok_or_else(|| AppError::Internal("题目缺少 MBTI 维度".into()))?;
        if let Some(score) = scores.get_mut(&dimension) {
            *score += if *choice == 1 { 1 } else { -1 };
        }
    }

    if request.operation.len() < questions.len() {
        return Err(AppError::Validation("请完成全部 MBTI 题目后再提交".into()));
    }
    Ok(format!(
        "{}{}{}{}",
        if scores[&'E'] > 0 { 'E' } else { 'I' },
        if scores[&'S'] > 0 { 'S' } else { 'N' },
        if scores[&'F'] > 0 { 'F' } else { 'T' },
        if scores[&'J'] > 0 { 'J' } else { 'P' },
    ))
}

pub async fn career_recommendation(state: &Arc<AppState>, raw_type: &str) -> AppResult<String> {
    let mbti_type = raw_type.trim().to_ascii_uppercase();
    if !is_valid_type(&mbti_type) {
        return Err(AppError::Validation(
            "MBTI 类型格式无效，应类似 INTJ、ENFP".into(),
        ));
    }

    let pool = state
        .databases
        .mbti
        .as_ref()
        .ok_or_else(|| AppError::Config("MBTI 数据库不可用，无法读取职业推荐".into()))?;
    if !state.llm.is_configured() {
        return Err(AppError::Llm(
            "未配置 DEEPSEEK_API_KEY，无法由大模型生成 MBTI 职业查询 SQL".into(),
        ));
    }
    let sql = state.llm.mbti_careers_sql(&mbti_type).await?;
    validate_readonly_sql(&sql, &["mbti_career_mapping", "mbti_types", "careers"])?;
    let rows = sqlx::query(&sql).fetch_all(pool).await?;
    let careers: Vec<(String, String)> = rows
        .into_iter()
        .filter_map(|row| {
            Some((
                row.try_get::<String, _>("career_name").ok()?,
                row.try_get::<String, _>("career_description")
                    .unwrap_or_default(),
            ))
        })
        .collect();
    if careers.is_empty() {
        return Err(AppError::NotFound(
            "数据库中没有找到该 MBTI 类型的职业推荐".into(),
        ));
    }
    Ok(format_careers(&mbti_type, &careers))
}

fn is_valid_type(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 4
        && matches!(bytes[0], b'I' | b'E')
        && matches!(bytes[1], b'N' | b'S')
        && matches!(bytes[2], b'F' | b'T')
        && matches!(bytes[3], b'J' | b'P')
}

fn format_careers(mbti_type: &str, careers: &[(String, String)]) -> String {
    let split = careers.len().min(6);
    let preferred = careers[..split]
        .iter()
        .map(|(name, description)| {
            format!("- **{name}**：{}", enrich_description(name, description))
        })
        .collect::<Vec<_>>()
        .join("\n");
    let interested = careers[split..]
        .iter()
        .map(|(name, description)| {
            format!("- **{name}**：{}", enrich_description(name, description))
        })
        .collect::<Vec<_>>()
        .join("\n");
    let extra = if interested.is_empty() {
        String::new()
    } else {
        format!("\n\n## 也可以进一步了解\n\n{interested}")
    };
    format!(
        "## {mbti_type} 类型职业与专业方向推荐\n\n\
        {mbti_type} 类型通常适合发挥自身认知优势与工作偏好的岗位。以下结果来自数据库，并已补充工作内容、能力要求和专业选择提示，仅作专业探索参考，建议再结合课程兴趣、学科基础和真实职业体验判断。\n\n\
        ## 优先推荐方向\n\n{preferred}{extra}\n\n\
        ## 使用建议\n\n- 先查看这些职业对应的大学专业、核心课程和实习场景。\n- 再比较自己是否喜欢日常工作内容，而不只看职业名称。\n- 关注岗位对数学、写作、沟通、编程、实验或审美能力的要求。\n- MBTI 不能决定人生方向，请把它当作辅助工具。"
    )
}

fn enrich_description(name: &str, description: &str) -> String {
    let base = clean_description(description);
    let profile = career_profile(name);
    format!(
        "{} 主要工作包括{}；适合{}。大学阶段可重点关注{}，同时通过{}积累真实体验。",
        base, profile.tasks, profile.traits, profile.majors, profile.practice
    )
}

fn clean_description(description: &str) -> String {
    let trimmed = description.trim().trim_end_matches(['。', '；', ';', '.']);
    if trimmed.is_empty() {
        "该方向需要在具体行业中运用专业知识解决问题".into()
    } else {
        trimmed.into()
    }
}

#[derive(Debug, Clone, Copy)]
struct CareerProfile {
    tasks: &'static str,
    traits: &'static str,
    majors: &'static str,
    practice: &'static str,
}

fn career_profile(name: &str) -> CareerProfile {
    let rules: &[(&[&str], CareerProfile)] = &[
        (
            &[
                "软件",
                "计算机",
                "程序",
                "网络",
                "安全",
                "数据",
                "人工智能",
                "SEM",
                "内容营销",
            ],
            CareerProfile {
                tasks: "系统设计、代码实现、数据分析、产品迭代和线上问题排查",
                traits: "逻辑清晰、愿意持续学习、能把复杂问题拆成步骤的人",
                majors: "计算机科学与技术、软件工程、数据科学与大数据技术、网络空间安全、人工智能等专业",
                practice: "课程项目、开源作品、算法训练、企业实习或个人产品作品集",
            },
        ),
        (
            &[
                "医生", "护士", "药剂", "牙医", "验光", "医疗", "营养", "护理", "健康",
            ],
            CareerProfile {
                tasks: "健康评估、诊疗或护理配合、用药与康复建议、患者沟通和记录管理",
                traits: "责任感强、细致耐心、能承受规范训练和持续学习压力的人",
                majors: "临床医学、护理学、药学、口腔医学、公共卫生、营养学等专业",
                practice: "医院见习、健康科普项目、实验课程、志愿服务和资格证书准备",
            },
        ),
        (
            &["教师", "教育", "辅导员", "家庭教师", "培训", "社工", "社区"],
            CareerProfile {
                tasks: "学习支持、课程设计、个体沟通、成长陪伴和资源协调",
                traits: "表达温和、有共情力、愿意帮助他人成长的人",
                majors: "教育学、心理学、汉语言文学、社会工作、学科师范类专业",
                practice: "支教、家教、班级管理实践、心理辅导训练和社团组织经历",
            },
        ),
        (
            &[
                "会计", "审计", "银行", "财务", "保险", "经济", "精算", "合规",
            ],
            CareerProfile {
                tasks: "财务核算、风险评估、数据建模、报表分析和制度合规检查",
                traits: "数字敏感、严谨稳健、能长期处理细节和规则的人",
                majors: "会计学、财务管理、金融学、经济学、统计学、保险学等专业",
                practice: "财务实训、数据分析项目、证券/银行/会计师事务所实习和证书学习",
            },
        ),
        (
            &[
                "经理",
                "项目",
                "运营",
                "供应链",
                "销售主管",
                "销售经理",
                "客户经理",
                "人力资源",
            ],
            CareerProfile {
                tasks: "目标拆解、团队协作、资源调度、进度跟踪和业务结果复盘",
                traits: "组织能力强、沟通主动、愿意面对不确定性和结果压力的人",
                majors: "工商管理、市场营销、人力资源管理、物流管理、信息管理与信息系统等专业",
                practice: "学生组织、商业竞赛、项目管理实训、企业运营或销售实习",
            },
        ),
        (
            &["律师", "法官", "法律", "警察", "狱警", "文书"],
            CareerProfile {
                tasks: "事实梳理、证据审查、规则解释、文书写作和当事人沟通",
                traits: "原则感强、表达准确、能在压力下保持判断的人",
                majors: "法学、公安学、社会学、政治学与行政学等专业",
                practice: "模拟法庭、法律援助、法院/律所实习、案例检索和写作训练",
            },
        ),
        (
            &[
                "工程",
                "机械",
                "电工",
                "建筑",
                "质量",
                "飞行员",
                "消防",
                "后勤",
            ],
            CareerProfile {
                tasks: "方案设计、设备维护、现场实施、质量检查和安全风险控制",
                traits: "动手能力强、重视规范、喜欢把技术落到真实场景的人",
                majors: "机械工程、电气工程、土木工程、建筑学、自动化、交通运输等专业",
                practice: "工程制图、实验室项目、技能竞赛、工厂/工地/实验室实习",
            },
        ),
        (
            &[
                "艺术", "设计", "摄影", "音乐", "作家", "演员", "舞蹈", "手工", "园艺", "纹身",
            ],
            CareerProfile {
                tasks: "创意构思、作品制作、用户或观众沟通、风格打磨和项目交付",
                traits: "审美敏锐、表达欲强、愿意长期打磨作品集的人",
                majors: "视觉传达设计、数字媒体艺术、音乐表演、戏剧影视、美术学、园林等专业",
                practice: "作品集、展演比赛、商业委托、短视频/摄影项目和跨学科创作",
            },
        ),
        (
            &[
                "客服", "接待", "迎宾", "活动", "婚礼", "旅行", "导游", "调酒", "烹饪", "面包",
            ],
            CareerProfile {
                tasks: "客户接待、需求确认、现场服务、流程安排和体验优化",
                traits: "亲和力强、反应快、能在多人互动场景中保持稳定的人",
                majors: "旅游管理、酒店管理、会展经济与管理、市场营销、食品科学与工程等专业",
                practice: "服务业实习、活动执行、校园接待、餐饮实践和客户沟通训练",
            },
        ),
    ];

    for (keywords, profile) in rules {
        if keywords.iter().any(|keyword| name.contains(keyword)) {
            return *profile;
        }
    }

    CareerProfile {
        tasks: "信息收集、问题分析、方案执行、跨部门沟通和结果复盘",
        traits: "愿意学习行业知识、能稳定推进任务、重视职业长期积累的人",
        majors: "管理学、经济学、社会学、心理学、计算机或与该行业相关的应用型专业",
        practice: "职业访谈、短期实习、课程项目、竞赛活动和作品集整理",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_mbti_type() {
        assert!(is_valid_type("INTJ"));
        assert!(!is_valid_type("ABCD"));
    }

    #[test]
    fn calculates_expected_type() {
        let questions = vec![
            vec!["q1".into(), "a".into(), "b".into(), "E".into()],
            vec!["q2".into(), "a".into(), "b".into(), "S".into()],
            vec!["q3".into(), "a".into(), "b".into(), "F".into()],
            vec!["q4".into(), "a".into(), "b".into(), "J".into()],
        ];
        let request = MbtiChoiceRequest {
            operation: HashMap::from([
                ("0".into(), 1),
                ("1".into(), 2),
                ("2".into(), 1),
                ("3".into(), 2),
            ]),
        };
        assert_eq!(calculate_type(&questions, &request).unwrap(), "ENFP");
    }
}
