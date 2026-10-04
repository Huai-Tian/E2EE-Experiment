# Security Policy / 安全策略

## Reporting a Vulnerability / 报告漏洞

**Report privately — do NOT open a public issue.**
**请私下报告，不要开公开 issue。**

- **Preferred / 首选**：GitHub Private Vulnerability Reporting
  （仓库 Security 标签页 → "Report a vulnerability"）
- **Also accepted / 同样接受**：加密邮件发送至 huaitian.behinder@gmail.com，
  使用 GPG 公钥
  `125F 7101 1717 0318 A318  F9C4 7FA3 DE0E 6B42 E01A`（Huai-Tian (Feedback)）

**What to include / 请附**：受影响的版本或 commit、复现步骤、影响评估
（版本号见 Releases；SHA256SUMS 用于校验二进制）。

## Scope / 范围

协议组合与实现（握手、PAKE、帧格式、文件传输、群钥匙表、流量垫）
及 FFI 表面（`libe2ee.so` / `e2ee.dll`）。

Out of scope / 不在范围：README "Known limitations / 已知限制" 已载明的
设计取舍——如电话模型的中间人缺口、无存活探测——这些是文档化的边界，
不是漏洞。

## What to expect / 预期

本项目为非商业研究项目，由个人维护：**尽力而为响应，不承诺时限**。
确认有效的问题会尽快修复并在 GitHub Security Advisories 公告。

## Safe harbor / 安全港

私下报告的善意安全研究不会被追究法律责任。
Good-faith research reported privately will not face legal action.
