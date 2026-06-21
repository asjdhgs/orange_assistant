# 架构与 Rust 特性说明

## 总体结构

项目采用 Rust/Axum 构建 Web 服务。页面、静态资源和 API 由同一个进程托管，服务统一处理页面渲染、静态资源、认证、MBTI、数据库访问、AI 推荐检索规划、知识图谱、风险分析和外部模型代理。

```text
浏览器 Rust 前端（/、/app.css、/app.js）
        │ HTTP / SSE
        ▼
Axum 路由层 routes.rs + frontend.rs
        │
        ├─ frontend.rs       页面 HTML、CSS、浏览器交互逻辑
        ├─ auth.rs           用户注册、登录、管理员查询
        ├─ mbti.rs           40 题测评、类型计算、职业匹配
        ├─ recommendation.rs AI SQL 校验、院校筛选、加权排序、冲稳保分档
        ├─ portfolio.rs      组合风险、敏感性、蒙特卡洛模拟
        ├─ knowledge.rs      知识图谱解析、匹配、BFS 遍历
        └─ llm.rs            DeepSeek 调用、SQL 生成与推荐解读
                │
                ▼
        DeepSeek / MySQL
```

## Rust 核心特性

- 所有权与借用：分析函数尽量借用 `&StudentProfile` 和 `&RecommendationTable`，只在跨异步任务共享时使用 `Arc`。
- `struct` 与 `enum`：请求、响应、志愿档位、风险等级和数据库状态均使用强类型表达。
- `trait`：Serde 的 `Serialize/Deserialize`、SQLx 的行解码、Axum 的响应转换构成接口抽象。
- 泛型：统一的 `ApiMessage<T>` 支持不同消息载荷。
- 错误处理：`AppResult<T>` 和 `AppError` 通过 `?` 传播错误并统一转换为 HTTP 响应。
- 异步并发：Tokio 驱动 HTTP、MySQL 和外部模型请求，`RwLock` 管理低冲突共享状态。
- 集合与图算法：知识图谱使用 `HashMap`、`HashSet`、`VecDeque` 完成索引和广度优先遍历。
- 数值算法：志愿组合分析使用确定性伪随机模拟，估算全部未录取风险和分数敏感性。
- AI SQL 生成：模型根据学生画像生成只读查询 SQL，推荐模块校验 SQL 安全性后读取数据库并完成排序。
- 业务查询：院校推荐、一分一段表、MBTI 题目和 MBTI 职业推荐均通过模型生成 SQL；账号注册与登录保留固定参数化 SQL。

## 安全改进

- 所有 SQL 用户参数均使用 SQLx 绑定，不再拼接手机号、密码和专业名称。
- 推荐 SQL 只执行固定的只读查询。
- 数据库连接和模型密钥通过运行环境配置读取，业务代码不直接写入敏感配置。
- 大模型生成的 SQL 只允许只读查询，并由服务端校验表名和危险关键字。
