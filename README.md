# Voice Input

面向个人使用的 macOS 语音输入工具，基于 [OpenTypeless](https://github.com/tover0314-w/opentypeless) v1.1.60（MIT）定制。使用自己的语音识别和大模型 API 密钥，将语音转成可直接输入或复制的文字。

> 本仓库维护 Voice Input 个人版，仅维护 macOS 版本，已在 Apple Silicon Mac 上构建；CI 也只验证 Apple Silicon（ARM64），不代表已验证 Intel Mac。上游保留的其他平台代码、多语言文档和历史发布记录不代表本项目已验证相应功能。本版不接入上游账户、订阅或自动更新。

## 主要功能

- **Fn 切换录音**：第一次按下开始，松开继续；第二次按下结束并处理。Esc 可取消。
- **自动输入**：录音开始时实时读取前台应用，在输出前再次核对输入位置；兼容先返回窗口、稍后才暴露编辑框的应用，确认真正聚焦的可编辑控件后写入。
- **结果编辑与复制**：没有可用输入框、输出前焦点变化或系统报告发送失败时，保留完整文字；可直接修正识别错误再一键复制，多条待处理结果分别保留编辑草稿。系统报告发送成功后，不再因浏览器回读延迟、文字格式变化或无法回读而弹窗。
- **两种 AI 润色风格**：清爽整理成自然语句和段落；结构化保留完整表达，只给实际列表编号，背景、解释、补充想法与请求自然分段。保留原意和细节，润色失败时仍保留原始转写。
- **自己的服务商与密钥**：沿用上游的 STT / LLM 服务商选择及兼容接口配置。
- 中文界面、本地历史记录和个人词典。

详细操作见 [中文使用说明](PERSONAL_USAGE.zh-CN.md)；提示词链路、优先级和实测结论见 [润色风格说明](docs/polish-styles.md)。

## 本地开发

需要 macOS、Xcode Command Line Tools、Node.js 22.12+、Rust stable 和 CMake。首次开发请先安装 [Tauri 所需依赖](https://v2.tauri.app/start/prerequisites/)。

```bash
git clone https://github.com/auorwi/voice-input.git
cd voice-input
npm ci
npm run tauri dev
```

构建日常使用的本机应用（固定本地证书签名，不含 Apple 公证）：

```bash
npm run signing:setup   # 首次在这台 Mac 上运行；重复执行会复用原身份
npm run build:mac
```

默认产物为 `src-tauri/target/release/bundle/macos/Voice Input.app`；若设置了绝对路径的 `CARGO_TARGET_DIR`，产物位于相应目录。退出旧应用后使用 `npm run signing:install -- "产物绝对路径" "固定安装绝对路径"` 安装。日常保留同一安装路径，避免构建副本同时出现在系统搜索中。

构建和安装都会校验原有证书身份；证书丢失或变化时中止，不退回临时签名。首次从旧临时签名版本迁移、证书保存及回滚方法见 [本机固定签名说明](docs/local-macos-signing.md)。`tauri dev` 仍用于开发调试，不作为日常授权版本。

## 配置与权限

1. 在应用设置中选择语音识别服务商，填写自己的 API 密钥并测试连接。
2. 如需润色，配置 LLM 服务商和模型。
3. 在 macOS「隐私与安全性」中授予 Voice Input 麦克风和辅助功能权限。
4. 在文本编辑器中点选输入位置，按 Fn 试录一句，再按 Fn 结束。

API 密钥通过 macOS 钥匙串保存。音频和相关文本会发送给你配置的 STT / LLM 服务商；费用、可用地区和数据政策由相应服务商决定。录音历史保存在本机，不应提交到 Git。

### 已授权但仍提示缺少辅助功能权限

旧版的临时签名会随重新构建改变，导致系统保留旧授权而新版本无法使用。从 1.1.61 起，日常构建使用固定证书身份；不要再使用 `signingIdentity: "-"`，也不要直接覆盖正在运行的应用。

从旧签名迁移时需要一次重新授权，以后的同身份升级应保留权限。如果仍有异常，先核对正在运行的应用路径和 `npm run signing:verify -- "应用路径"`；仅在确认是旧授权记录时处理 Voice Input 自身的条目，避免反复重置权限。详见 [迁移与排查](docs/local-macos-signing.md#首次迁移与权限)。

## 验证

```bash
npm test
npm run lint
npm run build
npm run test:signing
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo test --locked --manifest-path src-tauri/Cargo.toml
```

部分较新的 Node.js 版本会向测试环境暴露不兼容的原生 `localStorage`；在这些版本上可用 `NODE_OPTIONS=--no-experimental-webstorage npm test` 运行测试。

自动测试覆盖快捷键状态、输出目标判定、结果保留、取消与界面行为。真实语音识别、润色、麦克风及目标应用输入仍需使用自己的设备和有效凭据验证。

## 来源与许可

- 上游：[tover0314-w/opentypeless](https://github.com/tover0314-w/opentypeless)
- 定制基线：v1.1.60，提交 `842f278191c39e22fa21efec347c750245c91170`
- 技术栈：Rust、Tauri 2、React、TypeScript
- 许可证：[MIT](LICENSE)；保留 OpenTypeless Contributors 的版权声明及 [第三方声明](THIRD_PARTY_NOTICES.md)。

问题反馈请使用 [本仓库 Issues](https://github.com/auorwi/voice-input/issues)。
