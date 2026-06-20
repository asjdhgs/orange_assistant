# 项目提交说明

这个目录是小橘助手 Rust 项目的提交目录，包含运行和演示所需的源码、资源、数据脚本和文档。

## 内容

- `src/`：Rust 源码。
- `frontend/src/`：页面图片资源。
- `frontend/lib/`：知识图谱页面需要的前端库。
- `database/`：MySQL 初始化脚本。
- `data/`：知识图谱数据文件。
- `scripts/`：启动和检查脚本。
- `docs/`：架构、分工和目录说明文档。
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

