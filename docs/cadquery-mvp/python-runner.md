# CadQuery Python Runner 手动环境

## 背景

`budn_cad_runner` 是 app server 调用的外部 CAD 工具，边界等同 OpenSCAD CLI。MVP 阶段不自动创建或分发 Python 环境，开发者需要先手动安装 Python 与 CadQuery。

## 安装要求

- Python：建议使用 3.11。当前验证使用 `python3.11`。
- CadQuery：当前验证使用 `cadquery==2.7.0`，对应 `cadquery-ocp==7.8.1.1.post1`。
- 安装位置不写入仓库，建议使用用户级 Python 环境或外部虚拟环境。

## 示例命令

```bash
python3.11 -m pip install --user cadquery
PYTHONPATH="$PWD" python3.11 -m budn_cad_runner \
  --script parts/top_lid.py \
  --project-root /path/to/project \
  --output-dir /tmp/budn-cad-output \
  --exports '' \
  --params '{"width":80}'
```

`python3` 如果指向系统 Python 3.9，通常无法直接使用当前 CadQuery wheel。开发和验证时应显式使用已安装 CadQuery 的解释器，例如 `python3.11`。

## 开发启动配置

`bun run web` 会启动 websocket host，host 在绑定端口前会验证 `CADQUERY_RUNNER_PYTHON` 指向的解释器能导入 `cadquery` 和 `budn_cad_runner`。验证失败时启动会直接失败，避免等到 CadQuery tool call 才暴露 `ModuleNotFoundError`。

本地 `.env` 建议显式配置：

```bash
BUDN_LLM_CONFIG=llm.toml
CADQUERY_RUNNER_PYTHON=/opt/homebrew/bin/python3.11
```

如果没有设置 `CADQUERY_RUNNER_PYTHON`，host 会回退到 `python3`。在 macOS 上这经常指向系统 Python，通常不能导入 CadQuery。

## 部署建议

- 容器或安装包应预置一个独立 Python 3.11 环境，并在该环境中安装 `cadquery` 与 budn' 的 runner 模块。
- 启动入口必须注入 `CADQUERY_RUNNER_PYTHON`，不要依赖服务器上的默认 `python3`。
- app server 启动前验证应保留为部署健康检查的一部分；失败时让进程退出，由上层 supervisor 或平台探针暴露配置问题。
- 后续可以把 `budn_cad_runner` 打成标准 Python package，并在镜像构建阶段安装，避免依赖当前工作目录进入 Python import path。

Phase 0c 起 `--exports step,stl,3mf` 会把对应格式写入 `--output-dir`，并在 stdout JSON 的 `exports` 与 `manifest.export_hashes` 中记录导出路径和内容 hash。

## 输出约定

- 成功时 exit code 为 `0`，stdout 输出 JSON，`status` 为 `success`。
- build 异常时 exit code 为 `1`，stdout 输出 JSON，`status` 为 `build_error`，stderr 输出 Python traceback。
- runner 自身错误时 exit code 为 `2`，stdout 输出 JSON，`status` 为 `runner_error`，stderr 输出 Python traceback。
