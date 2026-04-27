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

Phase 0b 只验证最小 mesh JSON 输出，`--exports` 暂时保留为 CLI 参数；STEP / STL / 3MF 导出在 Phase 0c 完整 runner 中实现。

## 输出约定

- 成功时 exit code 为 `0`，stdout 输出 JSON，`status` 为 `success`。
- build 异常时 exit code 为 `1`，stdout 输出 JSON，`status` 为 `build_error`，stderr 输出 Python traceback。
- runner 自身错误时 exit code 为 `2`，stdout 输出 JSON，`status` 为 `runner_error`，stderr 输出 Python traceback。
