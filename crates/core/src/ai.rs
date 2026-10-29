//! OpenAI 兼容 Chat 客户端（Ollama / DeepSeek / OpenAI / Groq …）。
//!
//! 非流式 + SSE 流式两种调用，
//! token 估算与截断，提示词文件管理（内嵌资产 → prompts 目录落盘）。

use anyhow::{bail, Context, Result};
use include_dir::{include_dir, Dir};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::time::Duration;

use crate::util::AppPaths;

static PROMPTS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets/prompts");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system".into(), content: content.into() }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".into(), content: content.into() }
    }
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    stream: bool,
}

#[derive(Deserialize, Default)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    #[serde(default)]
    message: Option<ContentField>,
    #[serde(default)]
    delta: Option<ContentField>,
}

#[derive(Deserialize)]
struct ContentField {
    #[serde(default)]
    content: String,
}

/// Chat 客户端（阻塞式，300s 超时——对齐 Go TaskContext(5min)）。
pub struct ChatClient {
    base_url: String,
    api_key: String,
    model: String,
    client: reqwest::blocking::Client,
}

impl ChatClient {
    pub fn new(base_url: &str, api_key: &str, model: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            model: model.to_string(),
            client: reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(300))
                .build()
                .expect("构建 HTTP 客户端失败"),
        }
    }

    fn post(&self, stream: bool, messages: &[ChatMessage]) -> Result<reqwest::blocking::Response> {
        let url = format!("{}/chat/completions", self.base_url);
        let body = ChatRequest { model: &self.model, messages, stream };
        let mut req = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body);
        if !self.api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", self.api_key));
        }
        let resp = req.send().context("AI 请求失败")?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let text = resp.text().unwrap_or_default();
            bail!("AI 请求失败 ({status}): {} —— {}", classify_error(status, &text), short(&text, 300));
        }
        Ok(resp)
    }

    /// 非流式调用。
    pub fn chat(&self, messages: &[ChatMessage]) -> Result<String> {
        let resp = self.post(false, messages)?;
        let chat: ChatResponse = resp.json().context("解析 AI 响应失败")?;
        let content = chat
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.map(|m| m.content))
            .unwrap_or_default();
        if content.is_empty() {
            bail!("AI 返回空响应");
        }
        Ok(content)
    }

    /// SSE 流式调用：每个 token 回调 on_token，返回完整消息。
    pub fn chat_stream(
        &self,
        messages: &[ChatMessage],
        mut on_token: impl FnMut(&str),
    ) -> Result<String> {
        let resp = self.post(true, messages)?;
        let reader = BufReader::new(resp);
        let mut full = String::new();
        for line in reader.lines() {
            let line = line.context("读取流式响应失败")?;
            let line = line.trim();
            let Some(data) = line.strip_prefix("data: ") else { continue };
            if data == "[DONE]" {
                break;
            }
            let Ok(chunk) = serde_json::from_str::<ChatResponse>(data) else { continue };
            let Some(content) = chunk.choices.into_iter().next().and_then(|c| c.delta.map(|d| d.content))
            else {
                continue;
            };
            if !content.is_empty() {
                print!("{content}");
                on_token(&content);
                full.push_str(&content);
            }
        }
        Ok(full)
    }
}

fn classify_error(status: u16, body: &str) -> &'static str {
    match status {
        401 | 402 | 403 => "认证失败（检查 api_key）",
        429 => "速率限制（稍后重试）",
        404 => {
            if body.to_lowercase().contains("model") {
                "模型不存在（检查 model 名称）"
            } else {
                "接口不存在（检查 base_url）"
            }
        }
        s if s >= 500 => "服务端错误",
        _ => "未知错误",
    }
}

fn short(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect()
    }
}

// ---------------------------------------------------------------------------
// Token 估算与截断（1 token ≈ 4 字符启发式）
// ---------------------------------------------------------------------------

/// 估算 token 数（中文按 1 字 ≈ 1 token，ASCII 按 4 字符 ≈ 1 token）。
pub fn estimate_tokens(s: &str) -> i64 {
    let mut cjk = 0i64;
    let mut other = 0i64;
    for ch in s.chars() {
        if ch as u32 > 0x2E80 {
            cjk += 1;
        } else {
            other += 1;
        }
    }
    cjk + other / 4
}

/// 按 token 上限截断（尾部追加截断标记）。
pub fn truncate_to_token_limit(s: &str, max_tokens: i64) -> String {
    if max_tokens <= 0 {
        return String::new();
    }
    // CJK 1:1、其它 4:1 的前缀截断
    let mut tokens = 0i64;
    for (idx, ch) in s.char_indices() {
        tokens += if ch as u32 > 0x2E80 { 1 } else { 0 };
        if tokens > max_tokens {
            let cut = s[..idx].trim_end();
            return format!("{cut}\n\n... [diff 已截断以适配 token 上限]");
        }
    }
    // ASCII 密集场景粗粒度二次校验
    if estimate_tokens(s) > max_tokens {
        let target_chars = (max_tokens * 4) as usize;
        let cut: String = s.chars().take(target_chars).collect();
        return format!("{cut}\n\n... [diff 已截断以适配 token 上限]");
    }
    s.to_string()
}

// ---------------------------------------------------------------------------
// 提示词文件管理（内嵌资产 → ~/.byte-git/prompts/ 落盘）
// ---------------------------------------------------------------------------

/// 初始化 prompts 目录：内嵌提示词在目标缺失时落盘（幂等）。
pub fn init_prompts() -> Result<PathBuf> {
    let paths = AppPaths::new()?;
    let dir = paths.prompts_dir();
    AppPaths::ensure_dir(&dir)?;
    for f in PROMPTS.files() {
        let name = f.path().file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if name.is_empty() { continue; }
        let dest = dir.join(&name);
        if !dest.exists() {
            std::fs::write(&dest, f.contents()).with_context(|| format!("写入提示词失败: {}", dest.display()))?;
        }
    }
    Ok(dir)
}

/// 读取提示词文件（prompts 目录，用户可自定义编辑）。
pub fn get_prompt_file(name: &str) -> Result<String> {
    let paths = AppPaths::new()?;
    let p = paths.prompts_dir().join(name);
    std::fs::read_to_string(&p).with_context(|| format!("读取提示词失败: {}", p.display()))
}

/// 内置提示词内容（assets/prompts 内嵌资产；未知名报错——天然白名单）。
pub fn builtin_prompt(name: &str) -> Result<&'static str> {
    let f = PROMPTS
        .files()
        .find(|f| f.path().file_name().map(|n| n == name).unwrap_or(false))
        .ok_or_else(|| anyhow::anyhow!("未知提示词: {name}"))?;
    Ok(std::str::from_utf8(f.contents())?)
}

/// 恢复提示词为内置默认（覆盖落盘文件），返回新内容。
pub fn reset_prompt_file(name: &str) -> Result<String> {
    let content = builtin_prompt(name)?.to_string();
    let paths = AppPaths::new()?;
    let dir = paths.prompts_dir();
    AppPaths::ensure_dir(&dir)?;
    let p = dir.join(name);
    std::fs::write(&p, &content).with_context(|| format!("写入提示词失败: {}", p.display()))?;
    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 内嵌提示词必须保留代码侧替换的全部占位符——提示词文案可随意改，
    /// 丢了占位符生成请求会带裸模板，此测试守住这个契约。
    #[test]
    fn builtin_prompts_keep_placeholders() {
        let aic = builtin_prompt("aicommit.md").unwrap();
        assert!(aic.contains("{{language}}"), "aicommit.md 缺 {{language}}");
        let air = builtin_prompt("airewrite.md").unwrap();
        for ph in ["{{language}}", "{{original}}", "{{files}}", "{{diff_summary}}", "{{diff}}"] {
            assert!(air.contains(ph), "airewrite.md 缺 {ph}");
        }
    }

    #[test]
    fn builtin_prompt_rejects_unknown_name() {
        assert!(builtin_prompt("no-such.md").is_err());
    }
}
