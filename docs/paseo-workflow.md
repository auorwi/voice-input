# Paseo 项目入门

在 Paseo 中使用 Worktree，可以在独立的工作目录和分支上修改本项目。操作前先确认当前 Worktree 和分支。

## 1. 新建 Worktree 后的 Setup

项目根目录的 `paseo.json` 将 `worktree.setup` 配置为 `npm ci`。新建 Worktree 后，Paseo 的 Setup 会执行：

```sh
npm ci
```

它依据 `package-lock.json` 安装锁定版本的依赖，为开发准备 `node_modules`。如果已有 `node_modules`，会先移除再安装；不会改写 `package.json` 或锁文件。若两者的依赖声明不一致，安装会报错。Setup 用于安装依赖，不会启动应用。

## 2. 检查直接依赖

在当前 Worktree 的项目根目录运行：

```sh
npm ls --depth=0
```

该命令列出项目直接依赖及其已安装版本，`--depth=0` 表示不展开更深层的依赖树。它只检查依赖，不会安装或修复依赖。

- 退出码为 `0` 且没有问题提示：本次检查未报告依赖问题。
- `UNMET DEPENDENCY` 或 `missing`：依赖缺失。
- `invalid`：已安装版本不符合要求。
- `extraneous`：发现未被项目依赖声明需要的包。

如检查失败，保留完整输出和退出码，先确认原因；不要把检查失败当作自动安装或修复的指令。

## 3. 修改后先看 Diff，再 Commit

1. 保存修改，在 Paseo 中查看当前 Worktree 的 Diff，逐项确认新增、删除和修改内容；新文件也要打开检查。
2. 点击 Paseo 顶部的 Commit 按钮前，确认当前工作区的全部改动都属于本次任务。该按钮会自动暂存全部改动、生成提交说明并提交；如果还有无关改动，应先处理好再点击。
3. 需要将本地提交同步到远程仓库时，再执行 Push，并确认目标远程和分支。

**Commit（提交）**：将暂存的改动记录到当前分支的本地 Git 历史中，不会自动上传。当前版本 Paseo 顶部的 Commit 按钮会自动暂存当前工作区的全部改动后提交。

**Push（推送）**：将本地提交上传到远程仓库，供协作者获取；未提交的文件改动不会随 Push 上传。只想保存本地进度时，完成 Commit 即可。
