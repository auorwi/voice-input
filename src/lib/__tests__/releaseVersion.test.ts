import { describe, expect, it } from 'vitest'
import ciWorkflowSource from '../../../.github/workflows/ci.yml?raw'
import appImageVerificationScriptSource from '../../../.github/scripts/verify-appimage-runtime-libraries.sh?raw'
import appImagePluginWrapperSource from '../../../.github/scripts/linuxdeploy-plugin-appimage-exclude-wrapper.sh?raw'
import linuxdeployPrepareScriptSource from '../../../.github/scripts/prepare-linuxdeploy-wrapper.sh?raw'
import linuxdeployWrapperSource from '../../../.github/scripts/linuxdeploy-exclude-wrapper.rs?raw'
import constantsSource from '../constants.ts?raw'
import linuxVerificationScriptSource from '../../../.github/scripts/upload-linux-verification-artifacts.sh?raw'

describe('release version wiring', () => {
  it('lets frontend builds read the release tag version from Vite env', () => {
    expect(constantsSource).toContain('import.meta.env.VITE_APP_VERSION')
  })

  it('limits CI Rust checks to macOS Apple Silicon and keeps Linux verification scripts intact', () => {
    // 项目仅维护 macOS 版本，CI 只验证 Apple Silicon；Linux 发布脚本保留但未接入 CI。
    expect(ciWorkflowSource).toContain('runs-on: macos-latest')
    expect(ciWorkflowSource).toContain('aarch64-apple-darwin')
    expect(ciWorkflowSource).not.toContain('ubuntu-22.04-arm')
    expect(linuxVerificationScriptSource).toContain(
      'verification_dir="release-verification/linux-${LINUX_ARCH}"',
    )
    expect(linuxVerificationScriptSource).toContain(
      'sha_file="$verification_dir/SHA256SUMS-linux-${LINUX_ARCH}.txt"',
    )
    expect(linuxVerificationScriptSource).toContain(
      'public_key_path="$verification_dir/OpenTypeless-Linux-${LINUX_ARCH}-GPG-KEY.asc"',
    )
  })

  it('keeps the Wayland exclusion wiring in the Linux AppImage packaging scripts', () => {
    expect(linuxdeployPrepareScriptSource).toContain('linuxdeploy-exclude-wrapper.rs')
    expect(linuxdeployPrepareScriptSource).toContain(
      'linuxdeploy-plugin-appimage-exclude-wrapper.sh',
    )
    expect(linuxdeployWrapperSource).toContain('--exclude-library')
    expect(linuxdeployWrapperSource).toContain('libwayland-client.so*')
    expect(appImagePluginWrapperSource).toContain("-name 'libwayland-client.so*' -delete")
    expect(linuxVerificationScriptSource).toContain('verify-appimage-runtime-libraries.sh')
  })

  it('rejects a release AppImage that still contains the Wayland client', () => {
    expect(appImageVerificationScriptSource).toContain('--appimage-extract')
    expect(appImageVerificationScriptSource).toContain("-name 'libwayland-client.so*'")
  })
})
