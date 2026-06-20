# 项目概览

小橘助手是一个面向高考志愿填报场景的 Rust Web 应用，提供学生信息采集、院校推荐、志愿组合分析、MBTI 职业探索、专业知识图谱和智能问答服务。

## 目录内容

- `src/`：Rust 源码，包含服务入口、路由、认证、推荐算法、MBTI、知识图谱、模型问答等模块。
- `frontend/src/`：页面使用的图片资源。
- `frontend/lib/`：知识图谱页面需要的前端库。
- `database/`：MySQL 初始化脚本。
- `data/`：专业知识图谱数据文件。
- `scripts/`：服务启动和项目检查脚本。
- `docs/`：架构、模块职责和目录说明文档。
- `Cargo.toml`、`Cargo.lock`：Rust 项目配置。
- `.env.example`：环境变量模板。

## 启动方式

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\start-backend.ps1
```

浏览器访问：

```text
http://127.0.0.1:8000
```
