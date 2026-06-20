# 小橘助手（Rust 组队大作业版）

小橘助手是一个基于 Rust 的高考志愿填报与专业探索系统。项目使用 Axum 提供 Web 服务，前端页面、静态资源和后端 API 由同一个 Rust 进程托管。

## 功能

- 手机号注册、密码登录和管理员用户查看
- 40 题 MBTI 测评与职业推荐
- 学生分数、位次、选科、专业和城市偏好采集
- MySQL 院校数据查询与冲/稳/保推荐
- 志愿组合风险、比例、敏感性和蒙特卡洛模拟
- 专业知识图谱匹配与动态关系图
- DeepSeek 流式咨询；未配置密钥时提供本地说明
- 一分一段表查询

## 作业规模

- Rust 源码约 3000～6000 行，处于组队项目建议范围。
- 10 个 Rust 模块，职责明确，其中 `frontend.rs` 负责页面、样式和浏览器交互。
- 15 个单元测试。
- `cargo fmt`、`cargo clippy -D warnings`、`cargo test` 均可检查。
- 小组分工说明见 [docs/TEAMWORK.md](docs/TEAMWORK.md)。
- 架构和 Rust 特性说明见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)。
- 项目目录说明见 [docs/PROJECT_CONTENTS.md](docs/PROJECT_CONTENTS.md)。

## 环境

- Rust 1.85+（当前已在 Rust 1.95 验证）
- MySQL 8.x 或兼容 MySQL 协议的 MariaDB
- 可选：DeepSeek API Key

## 快速启动

### 1. 配置

复制 `.env.example` 为 `.env`，填写数据库连接。默认端口：

- Rust Web 服务：`127.0.0.1:8000`
- MySQL：`127.0.0.1:3306`

若暂时没有数据库，系统仍可用内置演示数据启动；注册数据只在本次进程内保存。正式演示应导入数据库。

### 2. 导入数据库

依次导入：

```text
database/backend_database/users.sql
database/backend_database/mbti.sql
database/backend_database/tianjin.sql
database/frontend_database/一分一段表.sql
```

这些脚本会创建 `users`、`mbti_careers`、`tianjin` 和 `score_distribution` 数据库。

### 3. 启动

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\start-backend.ps1
```

浏览器访问：

```text
http://127.0.0.1:8000
```

## 质量检查

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\check.ps1
```

也可分别运行：

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## 主要 API

- `/api/orange/register`
- `/api/orange/loader`
- `/api/orange/questions`
- `/api/orange/result`
- `/api/orange/seek`
- `/api/orange/student`
- `/api/orange/smart_recommend`
- `/api/orange/recommend_result`
- `/api/orange/recommend_summary`
- `/api/orange/recommend_analysis`
- `/api/orange/chat/stream`
- `/process`
- `/get_dynamic_kg`

## 前端入口

- `/`：单页前端入口
- `/app.css`：页面样式
- `/app.js`：浏览器交互逻辑
- `/assets/*`：图片资源
- `/lib/*`：知识图谱前端库

## 提交注意

- 不要提交 `target/`、`.env`、`.mysql-data/`。
- 提交前补全分工文档中的成员信息。
- 演示视频建议依次展示登录、信息填写、推荐表、MBTI、知识图谱和 Rust 代码结构。
- 推荐概率仅供辅助决策，最终以当年官方招生计划和院校章程为准。
