# macOS 本机固定签名

旧的 ad-hoc 临时签名使用代码摘要识别应用。重新构建后摘要改变，旧的辅助功能授权和钥匙串许可可能不再匹配。1.1.61 的日常构建改用同一枚本机证书，并把应用身份固定为：

```text
identifier "com.local.voiceinput.personal" and certificate leaf = H"<本机证书 SHA-1>"
```

每次更新的代码摘要可以变化，证书及上述 designated requirement 保持一致。校验同时检查签名完整性、精确的身份表达式和证书 SHA-256；只匹配包名、临时签名、另一枚证书和放宽后的表达式都不能通过安装检查。

## 首次设置与日常更新

在仓库根目录执行，需 Python 3、macOS 自带的 `security`、`codesign`、`openssl`，以及 README 中的构建依赖：

```bash
npm run signing:setup
npm run signing:status
npm run build:mac
```

`signing:setup` 只在首次创建 **Voice Input Local Signing** 证书和私钥。再次执行会检查并复用同一身份。证书有效期十年，私钥保存在当前用户默认钥匙串中，导入时设置为不可导出，允许系统 `codesign` 使用。已有私钥不会被读取或导出，也不会修改系统信任根、Gatekeeper 或其他应用权限。

正常构建在编译前检查证书和私钥，打包后使用固定证书签名并验证。任一步失败即中止，不会回退到 ad-hoc 签名。需要离线编译且依赖已缓存时可执行 `npm run build:mac -- --offline`。设置 `CARGO_TARGET_DIR` 时必须提供绝对路径。

退出当前 Voice Input 后执行安装。以下路径需替换为你实际使用的路径；以后保持安装路径一致：

```bash
npm run signing:verify -- "/absolute/build/Voice Input.app"
npm run signing:install -- "/absolute/build/Voice Input.app" "/absolute/installed/Voice Input.app"
```

安装会先核验新旧签名、拒绝覆盖运行中的应用、备份旧应用为 ZIP、复制并验证临时安装目录，再替换及注册应用。替换后的验证或注册失败会恢复旧应用。安装器本身不会清理 TCC 权限或修改服务商密钥的钥匙串访问列表。安装成功后可删除不再需要的构建副本，避免系统搜索出现多个应用。

`npm run tauri dev` 是开发调试进程；不要把它与日常使用的固定签名应用混用。不要使用 `--sign -`、覆盖 `signingIdentity` 为 `-` 或手动修改已签名应用。

## 首次迁移与权限

从旧 ad-hoc 个人版迁移时，在已经退出旧应用的前提下执行一次：

```bash
npm run signing:install -- "/absolute/build/Voice Input.app" "/absolute/installed/Voice Input.app" --migrate-ad-hoc
```

该选项只接受同一包标识、签名完整的旧 ad-hoc 应用，不接受其他证书签名版本。旧身份变成固定身份属于一次身份迁移，macOS 可能要求重新允许麦克风、辅助功能或本应用的钥匙串项目。

如果系统辅助功能开关已开而应用仍提示未授权，先退出应用并确认只保留正在使用的那一份安装。在系统设置中仅移除 Voice Input 的旧条目，再启动当前安装的应用，通过应用内“授权”加入并开启，最后重启。必要时可由使用者主动执行 `tccutil reset Accessibility com.local.voiceinput.personal`，只重置本应用的辅助功能记录；不要在正常更新脚本中自动执行。

钥匙串系统弹窗中的“允许”只针对本次访问；要保存许可，可自行验证后点“始终允许”。固定签名保证后续版本有同一可核验身份，不会让应用获得其他钥匙串项目的访问权。

## 保存身份、恢复与回滚

状态位于 `~/Library/Application Support/com.local.voiceinput.personal-signing/`：

- `identity.json`：证书指纹、钥匙串路径等公开元数据。
- `certificate.der`：用于核验的公开证书。
- `backups/`：安装前保存的应用 ZIP。
- `identity.pending.json`：仅初始化未完成时出现；导入已成功但写入被中断时，重新运行 `signing:setup` 会核验并恢复原身份。

不要删除这些状态或钥匙串中的对应私钥。私钥设置为不可导出，不提供私钥导出备份流程；只保留 `identity.json` 不能恢复丢失的私钥。若钥匙串丢失、系统重装、证书到期或更换 Mac，需明确建立新身份并重新迁移授权，不能假装旧身份仍在。

如果初始化在私钥导入前中断，脚本会保留待恢复状态并停止。先确认钥匙串中是否存在原证书和私钥，不要直接删除状态重建，避免制造重复身份。

回滚固定签名版本：把备份 ZIP 解压至临时目录，先验证，再退出当前应用并通过同一安装命令安装。安装器仍要求匹配当前证书。最早的 ad-hoc 备份仅用于应急恢复，恢复它会改变应用身份并可能要求重新授权。

## 验证

```bash
npm run test:signing
VOICE_INPUT_SIGNED_FIXTURE="/absolute/installed/Voice Input.app" npm run test:signing
```

普通测试覆盖缺失或替换证书、放宽身份表达式、安装注册失败回滚，以及初始化中断后的恢复。可选集成测试复制一个已签名应用，修改版本后确认签名校验失败，再用同一证书重新签名并验证代码摘要改变、身份保持一致。集成测试需要本机已有身份，只对临时副本操作。

在本机已完成 1.1.60 固定签名版 → 1.1.61 固定签名版的实际覆盖升级：代码摘要变化，身份表达式不变；没有执行权限重置或切换开关，应用的辅助功能提示未再出现，两个服务商的密钥设置正常加载，没有新的钥匙串验证弹窗。该检查验证的是此设备上的升级授权连续性，不等同于完整语音识别和各应用自动输入测试。

该方案用于个人 Mac，未进行 Apple 公证。公开源码可以供其他人自行构建；向其他用户分发即装即用的 macOS 安装包，应另外配置 Apple Developer ID 与公证流程。Apple 对身份连续性、自签名及钥匙串的说明见 [Code Signing Guide](https://developer.apple.com/library/archive/documentation/Security/Conceptual/CodeSigningGuide/AboutCS/AboutCS.html) 和 [TN2206](https://developer.apple.com/library/archive/technotes/tn2206/)。
