# 清爽与结构化

本个人版只提供两种普通听写风格。此设置改变发给 LLM 的 system 提示词，不切换模型、不调用第二个模型，也不在模型输出后机械插入编号。

## 代码链路

```mermaid
flowchart LR
  A[设置选择并保存 polish_style] --> B[AppConfig 加载及兼容归一化]
  B --> C[语音转写文本和当前应用场景]
  C --> D[pipeline 构造 PolishRequest]
  D --> E[build_context_system_prompt]
  E --> F[DeepSeek OpenAI 兼容接口]
  F --> G[保留换行的文字结果]
  G --> H[写入当前输入框或结果弹窗]
```

- `src/components/Settings/LlmPane.tsx`：两项选择和解释文字。点击保存后配置进入后端。
- `src/stores/appStore.ts`：`PolishStyle` 只允许 `clean`、`structured`。
- `src-tauri/src/storage/mod.rs`：旧的 `minimal`、`professional` 及未知值归一为 `clean`；已有 `structured` 保留。
- `src-tauri/src/pipeline.rs`：把保存的风格、转写、上下文和场景传入 `PolishRequest`。
- `src-tauri/src/llm/prompt.rs`：组装完整 system 提示词。版本为 `voice-input-styles-v2`。
- `src-tauri/src/llm/openai.rs`：发送 system 提示词及 `<transcription>` 中的原文。DeepSeek 走这条兼容接口路径。

## 风格与优先级

| 风格 | 输出约定 |
|---|---|
| 清爽 | 去无意义口头词和偶然重复、修顺语句；自然短句和段落，不自动加标题或列表，不丢信息 |
| 结构化 | 两个及以上独立事项按编号分行；相关细节合并到对应项；较多事项可加中性主题标签；一个简单想法保持一句话 |

通用规则负责事实、否定、主观语气、条件、语言和安全边界；应用和场景负责语气及用词；最后追加所选风格的明确排版约定及配对示例。场景存在时不再跳过风格。选中文本或显式语音操作保留自身的转换要求，不套用普通听写排版。

旧版的问题有两点：场景存在时直接跳过内置风格；通用提示词同时要求不添加任何词语、为所有风格强制列点，与重写和结构化需求冲突。本次将“不得增加词语”改为“不得增加事实”，允许连接词和中性标签；列表规则归到结构化风格。

## 真实调用验证（2026-09-28）

测试使用生产 Rust 提示词函数、虚构转写、DeepSeek 已配置端点；请求 `deepseek-chat`，响应字段返回 `deepseek-flash`。temperature 0.3、max_tokens 4096；非流式记录结果，未测试麦克风或桌面输入。

最终一轮 9 组配对、18 次调用：

- 9/9 清爽输出没有自动编号。
- 8/8 多事项结构化输出逐行编号。
- 简单会议通知，两种风格均保持一句话。
- 多事项通用场景重复 3 次；另测邮件、工作聊天和要求单段落的冲突场景，所选风格均生效。
- 检查人物、日期、预算、否定、条件、主观语气以及 `API`、`QA`、`user_id` 等技术标识，均保留固定关键内容。

同一口述的实际输出示例：

清爽：

> API timeout 设成 20 秒，不要改成 30 秒，retry 还是两次。iOS 的登录按钮有重叠，Leo 今天修一下。上线要等 QA 通过，不要动 user_id 这个字段

结构化：

```text
1. API：timeout 设成 20 秒，不要改成 30 秒；retry 还是两次
2. iOS 登录按钮：有重叠，Leo 今天修一下
3. 上线：要等 QA 通过
4. user_id 字段：不要动
```

多事项复测有 4 条或 5 条的分组变化，表明具体组织方式仍由模型判断。固定样本通过不代表所有输入均能完全保真，也不代表语音识别、权限和自动写入的端到端验证。

本次回归还覆盖了场景存在时风格仍被传入、旧配置迁移、界面只有两项，以及显式语音操作不受普通听写格式影响。
