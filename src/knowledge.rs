use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Quadruple {
    pub kind: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
}

impl Quadruple {
    pub fn as_frontend_row(&self) -> Vec<String> {
        vec![
            self.kind.clone(),
            self.subject.clone(),
            self.predicate.clone(),
            self.object.clone(),
        ]
    }
}

#[derive(Debug, Clone, Default)]
pub struct KnowledgeGraph {
    records: Vec<Quadruple>,
    entity_types: HashMap<String, HashSet<String>>,
    adjacency: HashMap<String, Vec<usize>>,
    categories: HashSet<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphAnalysis {
    pub normalized_query: String,
    pub matched_categories: Vec<String>,
    pub related_entities: Vec<Vec<String>>,
}

impl KnowledgeGraph {
    pub async fn load(path: impl AsRef<Path>) -> AppResult<Self> {
        let path = path.as_ref();
        let content = fs::read_to_string(path).await.map_err(|error| {
            AppError::Internal(format!("无法读取知识图谱 {}：{error}", path.display()))
        })?;
        let records = parse_records(&content);
        if records.is_empty() {
            return Err(AppError::Internal(format!(
                "知识图谱 {} 中没有有效记录",
                path.display()
            )));
        }
        Ok(Self::from_records(records))
    }

    pub fn from_records(records: Vec<Quadruple>) -> Self {
        let mut graph = Self {
            records,
            ..Self::default()
        };
        for (index, record) in graph.records.iter().enumerate() {
            graph
                .adjacency
                .entry(record.subject.clone())
                .or_default()
                .push(index);
            graph
                .adjacency
                .entry(record.object.clone())
                .or_default()
                .push(index);
            if record.kind == "实体" {
                graph
                    .entity_types
                    .entry(record.subject.clone())
                    .or_default()
                    .insert(record.predicate.clone());
                if record.predicate.contains("专业类") || record.subject.ends_with('类') {
                    graph.categories.insert(record.subject.clone());
                }
            }
            if record.kind == "实体关系" && record.subject.ends_with('类') {
                graph.categories.insert(record.subject.clone());
            }
        }
        graph
    }

    pub fn entity_count(&self) -> usize {
        self.entity_types.len()
    }

    pub fn relation_count(&self) -> usize {
        self.records
            .iter()
            .filter(|record| record.kind == "实体关系")
            .count()
    }

    pub fn analyze(&self, query: &str) -> GraphAnalysis {
        let normalized = normalize_query(query);
        let categories = self.match_categories(&normalized, 4);
        let related = self.focused_related_records(&categories, 56);
        GraphAnalysis {
            normalized_query: normalized,
            matched_categories: categories,
            related_entities: related
                .into_iter()
                .map(|record| record.as_frontend_row())
                .collect(),
        }
    }

    pub fn match_categories(&self, query: &str, limit: usize) -> Vec<String> {
        let parent_matches = self.parent_categories_for_query(query);
        let mut scored: Vec<(String, f64)> = self
            .categories
            .iter()
            .filter_map(|category| {
                let stem = category.trim_end_matches('类');
                let direct = query.contains(category) || (!stem.is_empty() && query.contains(stem));
                let overlap = character_similarity(query, category);
                let synonyms = synonym_score(query, category);
                let score = if parent_matches.contains(category) {
                    4.0
                } else if direct {
                    3.0
                } else if synonyms > 0.0 {
                    2.0 + synonyms
                } else {
                    overlap
                };
                (score >= 0.72).then(|| (category.clone(), score))
            })
            .collect();
        if scored.iter().any(|(_, score)| *score >= 2.0) {
            scored.retain(|(_, score)| *score >= 2.0);
        }
        scored.sort_by(|left, right| {
            right
                .1
                .total_cmp(&left.1)
                .then_with(|| left.0.cmp(&right.0))
        });
        scored
            .into_iter()
            .take(limit)
            .map(|(category, _)| category)
            .collect()
    }

    fn parent_categories_for_query(&self, query: &str) -> HashSet<String> {
        let mut parents = HashSet::new();
        for record in &self.records {
            if record.kind == "实体关系"
                && record.predicate.contains("包含")
                && self.categories.contains(&record.subject)
                && query.contains(record.object.as_str())
            {
                parents.insert(record.subject.clone());
            }
        }
        parents
    }

    pub fn focused_related_records(
        &self,
        categories: &[String],
        max_records: usize,
    ) -> Vec<Quadruple> {
        let mut result = Vec::new();
        let mut seen = HashSet::new();
        let category_set: HashSet<&str> = categories.iter().map(String::as_str).collect();
        let mut direct_children = HashSet::new();

        for category in categories {
            for child in self.direct_children(category, 10) {
                direct_children.insert(child);
            }
        }

        for record in &self.records {
            if result.len() >= max_records {
                break;
            }
            let is_category_record = category_set.contains(record.subject.as_str())
                || category_set.contains(record.object.as_str());
            let is_direct_child_record = direct_children.contains(&record.subject)
                || direct_children.contains(&record.object);
            let keep = match record.kind.as_str() {
                "实体关系" => is_category_record,
                "实体" => {
                    category_set.contains(record.subject.as_str())
                        || direct_children.contains(&record.subject)
                }
                _ => is_category_record || is_direct_child_record,
            };
            if keep && seen.insert(record.clone()) {
                result.push(record.clone());
            }
        }
        result
    }

    fn direct_children(&self, category: &str, limit: usize) -> Vec<String> {
        let Some(indices) = self.adjacency.get(category) else {
            return Vec::new();
        };
        let mut children = Vec::new();
        for index in indices {
            let record = &self.records[*index];
            if record.kind == "实体关系"
                && record.subject == category
                && record.predicate.contains("包含")
            {
                children.push(record.object.clone());
            }
        }
        children.sort();
        children.dedup();
        children.truncate(limit);
        children
    }
}

pub fn parse_records(content: &str) -> Vec<Quadruple> {
    let mut records = Vec::new();
    for raw_line in content.lines() {
        let line = raw_line.trim().trim_start_matches(['-', '*', ' ']).trim();
        if !line.starts_with('(') || !line.ends_with(')') {
            continue;
        }
        let body = line.trim_start_matches('(').trim_end_matches(')');
        let mut parts = body
            .splitn(4, ';')
            .map(|part| part.trim().trim_matches(['"', '\'', '“', '”']).to_owned());
        let Some(kind) = parts.next() else { continue };
        let Some(subject) = parts.next() else {
            continue;
        };
        let Some(predicate) = parts.next() else {
            continue;
        };
        let Some(object) = parts.next() else { continue };
        if kind.is_empty() || subject.is_empty() || predicate.is_empty() || object.is_empty() {
            continue;
        }
        records.push(Quadruple {
            kind,
            subject,
            predicate,
            object,
        });
    }
    records
}

fn normalize_query(query: &str) -> String {
    let replacements = [
        ("码农", "计算机 软件工程"),
        ("编程", "计算机 软件工程"),
        ("AI", "人工智能 计算机"),
        ("ai", "人工智能 计算机"),
        ("芯片", "电子信息 集成电路"),
        ("医生", "临床医学 医学"),
        ("老师", "教育学"),
        ("金融", "金融学 经济学"),
        ("画画", "美术学 设计学"),
        ("机器人", "自动化 机械 计算机"),
        ("心理咨询", "心理学"),
    ];
    let mut normalized = query.trim().to_owned();
    for (source, target) in replacements {
        if normalized.contains(source) {
            normalized.push(' ');
            normalized.push_str(target);
        }
    }
    normalized
}

fn character_similarity(left: &str, right: &str) -> f64 {
    let left_chars: HashSet<char> = left
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let right_chars: HashSet<char> = right
        .chars()
        .filter(|character| !character.is_whitespace() && *character != '类')
        .collect();
    if left_chars.is_empty() || right_chars.is_empty() {
        return 0.0;
    }
    let common = left_chars.intersection(&right_chars).count();
    common as f64 / right_chars.len() as f64
}

fn synonym_score(query: &str, category: &str) -> f64 {
    let groups: &[(&[&str], &[&str])] = &[
        (
            &["物理", "物理学", "应用物理", "核物理", "量子", "声学"],
            &["物理学类"],
        ),
        (
            &["计算机", "软件", "编程", "人工智能", "算法", "数据"],
            &["计算机类", "电子信息类", "自动化类"],
        ),
        (
            &["机械", "制造", "机器人", "汽车"],
            &["机械类", "自动化类", "仪器类"],
        ),
        (
            &["生物", "生命", "基因"],
            &["生物科学类", "生物工程类", "生物医学工程类"],
        ),
        (
            &["经济", "金融", "投资", "证券"],
            &["经济学类", "金融学类", "财政学类"],
        ),
        (
            &["文学", "写作", "中文"],
            &["中国语言文学类", "新闻传播学类"],
        ),
        (
            &["绘画", "美术", "设计", "动画"],
            &["美术学类", "设计学类", "戏剧与影视学类"],
        ),
        (
            &["医学", "医生", "临床"],
            &["临床医学类", "基础医学类", "医学技术类"],
        ),
        (&["法律", "律师", "法官"], &["法学类", "公安学类"]),
    ];
    for (keywords, categories) in groups {
        if keywords.iter().any(|keyword| query.contains(keyword)) && categories.contains(&category)
        {
            return 1.0;
        }
    }
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_quadruples() {
        let content =
            "(实体; 计算机类; 专业类; 包含软件工程)\n(实体关系; 计算机类; 包含; 软件工程)";
        let records = parse_records(content);
        assert_eq!(records.len(), 2);
        assert_eq!(records[1].object, "软件工程");
    }

    #[test]
    fn traverses_related_entities() {
        let graph = KnowledgeGraph::from_records(parse_records(
            "(实体; 计算机类; 专业类; 描述)\n\
             (实体; 软件工程; 专业名称; 描述)\n\
             (实体关系; 计算机类; 包含; 软件工程)",
        ));
        let analysis = graph.analyze("我喜欢编程");
        assert!(analysis.matched_categories.contains(&"计算机类".to_owned()));
        assert!(!analysis.related_entities.is_empty());
    }

    #[test]
    fn physics_query_does_not_match_broad_related_categories() {
        let graph = KnowledgeGraph::from_records(parse_records(
            "(实体; 物理学类; 专业类; 包含物理学、应用物理学等专业)\n\
             (实体; 心理学类; 专业类; 包含心理学等专业)\n\
             (实体; 护理学类; 专业类; 包含护理学、助产学等专业)\n\
             (实体; 物理学; 专业名称; 专业代码:070201)\n\
             (实体; 心理学; 专业名称; 专业代码:071101)\n\
             (实体; 护理学; 专业名称; 专业代码:101101)\n\
             (实体关系; 物理学类; 包含; 物理学)\n\
             (实体关系; 心理学类; 包含; 心理学)\n\
             (实体关系; 护理学类; 包含; 护理学)",
        ));
        let analysis = graph.analyze("我对物理学感兴趣");
        assert_eq!(analysis.matched_categories, vec!["物理学类".to_owned()]);
    }
}
