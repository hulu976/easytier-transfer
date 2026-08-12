use serde::Deserialize;
use anyhow::Result;

/// `easytier-cli route --json` 输出的单条路由信息（仅取本阶段需要的字段，
/// 其余字段缺失也不报错，使用宽松反序列化）。
#[derive(Debug, Clone, Deserialize)]
pub struct RouteInfo {
    pub peer_id: Option<u32>,
    pub ipv4_addr: Option<String>,
    pub hostname: Option<String>,
    pub path_latency: Option<f64>,
}

/// 通过调用 `easytier-cli route --json` 获取在线节点列表。
///
/// 这是节点发现的"方式 1（CLI 解析）"——最简单、零耦合，初始阶段使用。
/// 注意：EasyTier 升级后 JSON 字段可能变化，故全部字段使用 `Option` 容错。
pub async fn list_online_peers() -> Result<Vec<RouteInfo>> {
    let output = tokio::process::Command::new("easytier-cli")
        .arg("route")
        .arg("--json")
        .output()
        .await?;

    if !output.status.success() {
        anyhow::bail!(
            "easytier-cli failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let routes: Vec<RouteInfo> = serde_json::from_str(&stdout).unwrap_or_default();

    // 过滤掉虚拟 IP 为空（本节点或无效条目）的条目
    Ok(routes
        .into_iter()
        .filter(|r| r.ipv4_addr.as_ref().map(|s| !s.is_empty()).unwrap_or(false))
        .collect())
}
