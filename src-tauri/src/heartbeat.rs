// 控制端心跳租约及过期槽位清理；清理任务作为 Tokio 后台任务运行（与 discovery.rs
// 的 UDP 监听一致），不占用独立的阻塞 std::thread。
use crate::{model::MonitorTile, runtime::SharedRuntime};
use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(2 * 60);
const SWEEP_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Default)]
pub(crate) struct ClientLeases {
    last_seen: HashMap<String, Instant>,
}

impl ClientLeases {
    pub(crate) fn heartbeat(&mut self, client_id: &str) {
        self.last_seen.insert(client_id.to_owned(), Instant::now());
    }

    fn expire(&mut self, now: Instant, timeout: Duration) -> HashSet<String> {
        let mut expired = HashSet::new();
        self.last_seen.retain(|client_id, last_seen| {
            let alive = now.duration_since(*last_seen) < timeout;
            if !alive {
                expired.insert(client_id.clone());
            }
            alive
        });
        expired
    }
}

pub(crate) fn valid_client_id(value: &str) -> bool {
    value.len() <= 128
        && !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn clear_tiles_owned_by(tiles: &mut [MonitorTile], expired: &HashSet<String>) -> usize {
    let mut cleared = 0;
    for tile in tiles {
        if expired.contains(&tile.client_id) {
            *tile = MonitorTile::default();
            cleared += 1;
        }
    }
    cleared
}

pub(crate) fn start_cleanup(runtime: SharedRuntime) {
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(SWEEP_INTERVAL);
        loop {
            ticker.tick().await;
            let expired = runtime
                .client_leases
                .lock()
                .expire(Instant::now(), HEARTBEAT_TIMEOUT);
            if expired.is_empty() {
                continue;
            }
            let mut state = runtime.state.write();
            let changed = clear_tiles_owned_by(&mut state.tiles, &expired) > 0;
            drop(state);
            if changed {
                runtime.changed();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expires_only_clients_outside_the_lease_window() {
        let start = Instant::now();
        let mut leases = ClientLeases::default();
        leases.last_seen.insert("expired".into(), start);
        leases
            .last_seen
            .insert("alive".into(), start + Duration::from_secs(90));

        let expired = leases.expire(start + Duration::from_secs(121), Duration::from_secs(120));

        assert!(expired.contains("expired"));
        assert!(!expired.contains("alive"));
        assert!(leases.last_seen.contains_key("alive"));
    }

    #[test]
    fn validates_client_ids_used_in_paths() {
        assert!(valid_client_id("550e8400-e29b-41d4-a716-446655440000"));
        assert!(!valid_client_id(""));
        assert!(!valid_client_id("client/one"));
    }

    #[test]
    fn clears_only_tiles_owned_by_expired_clients() {
        let mut tiles = vec![
            MonitorTile {
                client_id: "expired".into(),
                username: "A".into(),
                ..MonitorTile::default()
            },
            MonitorTile {
                client_id: "alive".into(),
                username: "B".into(),
                ..MonitorTile::default()
            },
        ];

        let cleared = clear_tiles_owned_by(&mut tiles, &HashSet::from(["expired".into()]));

        assert_eq!(cleared, 1);
        assert!(tiles[0].username.is_empty());
        assert_eq!(tiles[1].username, "B");
    }
}
