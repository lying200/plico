# 代码与变更约束

## 格式与工具链

Rust 工具链以根目录 `rust-toolchain.toml` 为准，格式由 `rustfmt.toml` 决定。
使用 `cargo fmt --package plico`；提交前运行相同命令加 `-- --check`，CI 强制检查。
EditorConfig 统一自有文件的缩进、UTF-8 与 LF；Markdown 保留有意义的行尾双空格。
第三方 `vendor/` 保持上游格式，仅格式化修改过的补丁范围；跨路径引用 vendor 的测试模块
显式跳过递归格式化。升级工具链时同时核对 devenv 的实际 rustc/rustfmt 版本和 CI。

格式化不能代替职责拆分。大范围纯格式变化应与行为修改分开审阅；不要为压低行数重新
压缩代码。行数用于定位热点，不作架构健康度或验收门槛。

## 模块与状态所有权

- Git 查询、写操作执行及结果解析属于核心模块；视图负责收集用户意图、显示结果、
  路由事件，不在多个视图重复解释 Git 输出或文档状态。
- 异步任务的取消令牌、请求身份和结果接纳规则由同一具体模块维护。切换仓库或查询后，
  旧结果必须失效；拆文件时一起迁移这些不变量。
- 区分草稿、磁盘错误、读取进行中和写入进行中。文档模块决定保存是否完成或受阻；
  调用方使用其结果，不重新组合内部标志。领域用语见 `../CONTEXT.md`。
- 引入抽象须对应真实变化点，优先具体模块与内部测试 seam。新增通用任务框架、事件总线
  或公共 trait 前，说明两个实际调用场景及其不同需求。
- 对照 Rebased 的规则继续按 `../AGENTS.md` 执行，记录源码与保留差异。

## 验证与提交

纯格式变更运行格式检查和核心测试；模块职责变化还需通过原调用入口的回归测试。
涉及桌面状态、焦点或生命周期时运行相关 `desktop-tests`；可见交互按项目要求做
Windows 原生验收。已知失败必须记录具体用例和复现条件，不能扩大忽略范围。

核心回归：`cargo test --locked --no-default-features`。
桌面定向回归：`cargo test --locked --features desktop-tests --bin plico <filter>`。
修改 Tab、焦点、编辑器或共享文档生命周期后，完整桌面验收包含两条命令：

```sh
cargo test --locked --features desktop-tests --bin plico
cargo test --locked --config 'profile.test.debug-assertions=false' --features desktop-tests --bin plico diff_tab::editable_navigation_tests
```

Diff 导航测试包含跨文件 F7 焦点与工具栏实际鼠标点击，均在默认调试配置下执行。
此前模式按钮重复设置 hover 导致的调试断言已修复，不再忽略这两项测试。
第二条保留发布配置的导航回归覆盖；不能全局关闭调试断言运行第一条来代替，
因为其他测试依赖调试布局信息。测试不代替 Windows 原生界面验收。
CI 的 `desktop-test` 独立任务执行这两条命令，并作为 preview 发布的前置门槛。
它使用 GPUI 测试平台检查状态、焦点和生命周期，不替代 Windows 原生交互验收。
提交记录分别注明本地测试、CI 运行与原生验收结果；修改流水线但尚未运行时不能称为 CI 已通过。
Windows CI 另执行 `cargo test --locked --no-default-features --lib --target x86_64-pc-windows-msvc hosting::`，
覆盖 Windows socket 模式、分段 OAuth 请求及取消释放；Linux 通过或 Windows 仅编译均不能替代。
CI 同时执行 `cargo clippy --locked --no-default-features --all-targets`，本地使用相同命令；
现有警告尚未清零，暂不作为全仓 `-D warnings` 门槛，也不通过全局 allow 隐藏债务。
新增行为测试覆盖实际调用路径及取消、迟到结果等相关边界；不为纯格式变化编写镜像测试。

Windows CI 另运行 `credential_store::windows_tests::`（与上述 hosting 测试使用相同 Cargo 参数），
通过唯一测试键验证原生凭据读写、超限失败保留旧值、条件删除和清理；只操作测试自建凭据，
不枚举或修改真实账号。该用例不覆盖系统凭据库不可用、账号索引与 Token 的多条写入事务或跨进程竞争。

Windows CI 执行 `cargo test --locked --no-default-features --test process_tree --target x86_64-pc-windows-msvc`，
覆盖 Job Object 取消、父进程先退出时的后代清理、继承管道关闭、无关进程保留和重复启动的句柄释放。
测试只创建自有子进程；其中 parent-exits fixture 必须保留“父进程先退出”的行为，不得为消除静态警告而等待其后代。
