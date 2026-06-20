# 项目目录说明

本项目是一个 Rust Web 应用，包含页面托管、API 服务、数据库访问、推荐算法、MBTI 测评、专业知识图谱和外部模型接入。

## 核心代码

| 路径 | 作用 |
|---|---|
| `Cargo.toml`、`Cargo.lock` | Rust 项目依赖、包信息和锁定版本 |
| `src/main.rs` | 程序入口，加载配置、数据库、知识图谱并启动 Axum 服务 |
| `src/routes.rs` | Web 路由和 API 接口 |
| `src/frontend.rs` | 页面 HTML、CSS 和浏览器交互脚本 |
| `src/config.rs` | 环境变量和数据库连接配置 |
| `src/error.rs` | 统一错误类型和 HTTP 响应转换 |
| `src/models.rs` | 请求、响应、学生信息、推荐结果等数据结构 |
| `src/auth.rs` | 用户注册、登录、管理员查询 |
| `src/mbti.rs` | MBTI 题目、计分逻辑和职业推荐 |
| `src/recommendation.rs` | 志愿推荐算法、院校筛选和一分一段表查询 |
| `src/portfolio.rs` | 志愿组合风险分析、比例检查和模拟 |
| `src/knowledge.rs` | 专业知识图谱解析、匹配和前端图数据生成 |
| `src/llm.rs` | DeepSeek 接入、模型回答、本地说明 |

## 资源与数据

| 路径 | 作用 |
|---|---|
| `frontend/src/` | 页面图片资源 |
| `frontend/lib/` | 知识图谱页面使用的前端库 |
| `database/backend_database/*.sql` | 用户、MBTI、院校推荐等 MySQL 数据表 |
| `database/frontend_database/一分一段表.sql` | 天津一分一段表数据 |
| `data/knowledge_graph.txt` | 专业知识图谱文本数据 |

## 脚本与文档

| 路径 | 作用 |
|---|---|
| `scripts/start-backend.ps1` | 启动 Web 服务 |
| `scripts/check.ps1` | 运行格式化、Clippy 和测试检查 |
| `docs/ARCHITECTURE.md` | 架构和 Rust 技术点说明 |
| `docs/TEAMWORK.md` | 小组分工说明 |

## 本地文件约定

- `.env`：本地环境变量，包含数据库密码和 API Key，不提交。
- `.env.example`：环境变量模板，只保留字段名和示例值。
- `target/`：Rust 编译产物，不提交。
