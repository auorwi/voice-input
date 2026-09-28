# Voice Input

面向个人使用的 macOS 语音输入工具，基于 [OpenTypeless](https://github.com/tover0314-w/opentypeless) v1.1.60（MIT）定制。使用自己的语音识别和大模型 API 密钥，将语音转成可直接输入或复制的文字。

> 本仓库维护 Voice Input 个人版，已在 Apple Silicon Mac 上构建。上游保留的其他平台代码、多语言文档和历史发布记录不代表本项目已验证相应功能。本版不接入上游账户、订阅或自动更新。

## 主要功能

- **Fn 切换录音**：第一次按下开始，松开继续；第二次按下结束并处理。Esc 可取消。
- **自动输入**：在录音开始与输出时核对当前可编辑输入位置；确认目标没有变化后尝试写入。
- **结果弹窗与复制**：没有可用输入框、焦点变化或无法确认写入成功时，保留完整文字并提供一键复制；多条待处理结果可分别查看、关闭。
- **两种 AI 润色风格**：清爽整理成自然语句和段落；结构化把独立事项分组、编号。保留原意和细节，简单通知不过度拆分，润色失败时仍保留原始转写。
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

构建本机应用（本地临时签名，不含 Apple 公证）：

```bash
npm run tauri build -- --bundles app --config '{"bundle":{"macOS":{"signingIdentity":"-"}}}'
```

默认产物为 `src-tauri/target/release/bundle/macos/Voice Input.app`；若设置了 `CARGO_TARGET_DIR`，产物位于相应目录。日常使用保留一份应用，避免构建副本同时出现在系统搜索中。

## 配置与权限

1. 在应用设置中选择语音识别服务商，填写自己的 API 密钥并测试连接。
2. 如需润色，配置 LLM 服务商和模型。
3. 在 macOS「隐私与安全性」中授予 Voice Input 麦克风和辅助功能权限。
4. 在文本编辑器中点选输入位置，按 Fn 试录一句，再按 Fn 结束。

API 密钥通过 macOS 钥匙串保存。音频和相关文本会发送给你配置的 STT / LLM 服务商；费用、可用地区和数据政策由相应服务商决定。录音历史保存在本机，不应提交到 Git。

### 已授权但仍提示缺少辅助功能权限

本地临时签名可能随重新构建而改变。如果系统开关已打开，而应用仍显示权限提示：

1. 退出 Voice Input，在系统的辅助功能列表中仅移除旧的 Voice Input 条目。
2. 打开正在使用的那一份应用，点击应用内「授权」，让当前版本重新出现在列表中。
3. 开启它的开关，按系统要求自行完成身份验证，然后重启应用。

## 验证

```bash
npm test
npm run lint
npm run build
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
