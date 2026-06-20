# 小橘助手

小橘助手是一个基于 Rust 的高考志愿填报与专业探索系统。项目使用 Axum 提供 Web 服务，前端页面、静态资源和后端 API 由同一个 Rust 进程托管，覆盖登录注册、志愿推荐、MBTI 职业探索、知识图谱和智能问答等功能。

## 项目功能

- 手机号注册、密码登录和管理员用户查看
- 学生分数、位次、选科、专业和城市偏好采集
- MySQL 院校数据查询与冲/稳/保志愿推荐
- 志愿组合风险、比例、敏感性和蒙特卡洛模拟
- 40 题 MBTI 测评与职业方向推荐
- 专业知识图谱匹配与动态关系图展示
- DeepSeek 流式咨询；未配置密钥时提供本地推荐说明
- 天津一分一段表查询

## 技术结构

- 后端框架：Rust + Axum + Tokio
- 数据访问：SQLx + MySQL
- 前端托管：Rust 输出单页界面、样式和交互脚本
- 智能问答：DeepSeek API 流式响应
- 图谱分析：基于本地专业知识图谱数据进行关键词匹配和关系遍历

更详细的架构说明见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)，项目目录说明见 [docs/PROJECT_CONTENTS.md](docs/PROJECT_CONTENTS.md)，模块职责说明见 [docs/TEAMWORK.md](docs/TEAMWORK.md)。

## 运行环境

- Rust 1.85+
- MySQL 8.x 或兼容 MySQL 协议的 MariaDB
- 可选：DeepSeek API Key

## 快速启动

### 1. 配置环境变量

复制 `.env.example` 为 `.env`，填写数据库连接和可选的模型 API Key。默认端口：

- Rust Web 服务：`127.0.0.1:8000`
- MySQL：`127.0.0.1:3306`

如果暂时没有数据库，系统仍可使用内置演示数据启动；导入数据库后可以获得完整的院校、MBTI 和一分一段表查询能力。

### 2. 导入数据库

依次导入：

```text
database/backend_database/users.sql
database/backend_database/mbti.sql
database/backend_database/tianjin.sql
database/frontend_database/一分一段表.sql
```

这些脚本会创建用户、MBTI、天津院校推荐和一分一段表相关数据。

### 3. 启动服务

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\start-backend.ps1
```

浏览器访问：

```text
http://127.0.0.1:8000
```

## 页面入口

- `/`：单页前端入口
- `/app.css`：页面样式
- `/app.js`：浏览器交互逻辑
- `/assets/*`：图片资源
- `/lib/*`：知识图谱前端库

## 主要接口

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

## 结果说明

系统会根据用户分数、位次、选科、专业偏好和城市偏好生成志愿推荐，并给出风险分析与解释。推荐概率用于辅助理解不同院校和专业组合的录取可能性，实际填报仍应结合当年官方招生计划、院校章程和个人意愿综合判断。
