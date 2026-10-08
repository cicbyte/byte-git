//! config：全局配置读写。

use byte_git_core::config::Settings;
use clap::{Args, Subcommand};

#[derive(Args, Debug)]
pub struct ConfigCmd {
    #[command(subcommand)]
    pub cmd: ConfigSub,
}

#[derive(Subcommand, Debug)]
pub enum ConfigSub {
    /// 列出全部配置
    List,
    /// 读取一个配置键（如 ai.model、github.token）
    Get { key: String },
    /// 写入一个配置键（顶层分组名.字段名）
    Set { key: String, value: String },
    /// 显示配置文件路径
    Path,
}

pub fn run(cmd: ConfigCmd) -> anyhow::Result<()> {
    match cmd.cmd {
        ConfigSub::Path => {
            println!("{}", byte_git_core::util::AppPaths::new()?.settings_file().display());
        }
        ConfigSub::List => {
            let s = Settings::load()?;
            println!("{}", serde_yaml::to_string(&s)?);
        }
        ConfigSub::Get { key } => {
            let s = Settings::load()?;
            match s.get(&key) {
                Some(v) => println!("{v}"),
                None => anyhow::bail!("未知配置键: {key}"),
            }
        }
        ConfigSub::Set { key, value } => {
            let mut s = Settings::load()?;
            let mut json = serde_json::to_value(&mut s)?;
            let Some(obj) = json.as_object_mut() else {
                anyhow::bail!("配置序列化异常");
            };
            let Some((group, field)) = key.split_once('.') else {
                anyhow::bail!("键格式应为 <分组>.<字段>，如 ai.model");
            };
            let Some(section) = obj.get_mut(group).and_then(|v| v.as_object_mut()) else {
                anyhow::bail!("未知配置分组: {group}");
            };
            if !section.contains_key(field) {
                anyhow::bail!("未知配置字段: {key}");
            }
            // 数字/布尔自动识别，其余按字符串
            let parsed: serde_json::Value = if let Ok(n) = value.parse::<i64>() {
                n.into()
            } else if let Ok(b) = value.parse::<bool>() {
                b.into()
            } else {
                value.clone().into()
            };
            section.insert(field.to_string(), parsed);
            s = serde_json::from_value(json)?;
            s.save()?;
            println!("已写入 {key} = {value}");
        }
    }
    Ok(())
}
