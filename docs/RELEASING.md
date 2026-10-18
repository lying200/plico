# 发布流程

main提交和PR自动执行核心测试及Windows release构建，产物保留14天。
发布时更新Cargo.toml、Cargo.lock中plico版本和Windows产品版本，添加
`docs/releases/v<版本>.md`，提交并等待构建通过，再推送同名v标签。
标签必须与Cargo版本一致；流水线成功后创建带ZIP和SHA-256的Draft Pre-release。
下载草稿中的包，在Windows检查启动、打开仓库、Diff和编辑保存，再发布草稿。
不覆盖已发布标签或附件；修复发下一个preview版本。

构建/测试任务只读仓库，只有标签发布任务拥有contents:write；没有自定义访问令牌。
自动化构建不能替代Windows桌面人工验收。Actions页面可手动触发构建。
