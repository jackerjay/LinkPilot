# Conventional Branches And Commits

收到用户指出“后续的修改都需要使用 Conventional Commits 进行分支创建和提交”后，后续所有 LinkPilot 代码修改必须遵守：

- 分支名使用 Conventional Commits 的类型语义：`fix/<short-kebab-summary>`、`feat/<short-kebab-summary>`、`docs/<short-kebab-summary>`、`chore/<short-kebab-summary>` 等。
- 不再默认创建 `codex/...` 分支，除非用户当次明确要求。
- 提交信息使用 `type(scope?): subject`，例如 `fix(desktop): fall back to ask mode without default target`。
- type 优先使用 `fix`、`feat`、`docs`、`test`、`refactor`、`chore`、`ci`；scope 应匹配实际改动面。
- 这条规则存在是因为 `codex/fix-browser-support-default-target` 这类代理前缀分支不符合用户要求的 Conventional 风格，会增加后续 PR 和 release 语义整理成本。
