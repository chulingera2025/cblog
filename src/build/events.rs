use serde::Serialize;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum BuildTask {
    FullRebuild,
    Manual,
}

/// 构建进度事件（通过 WebSocket 推送到前端）
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
pub enum BuildEvent {
    Started {
        trigger: String,
    },
    StageBegin {
        stage: String,
    },
    StageEnd {
        stage: String,
    },
    Finished {
        total_ms: u64,
        total_pages: usize,
        rebuilt: usize,
        cached: usize,
    },
    Failed {
        error: String,
    },
}
