-- 管理员账号
CREATE TABLE IF NOT EXISTS users (
    id            TEXT PRIMARY KEY,
    username      TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    created_at    TEXT NOT NULL,
    last_login_at TEXT
);

-- 文章
CREATE TABLE IF NOT EXISTS posts (
    id         TEXT PRIMARY KEY,
    slug       TEXT UNIQUE NOT NULL,
    title      TEXT NOT NULL,
    content    TEXT NOT NULL,
    status     TEXT NOT NULL DEFAULT 'draft',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    meta       TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_posts_status     ON posts(status);
CREATE INDEX IF NOT EXISTS idx_posts_created_at ON posts(created_at DESC);

-- 独立页面
CREATE TABLE IF NOT EXISTS pages (
    id         TEXT PRIMARY KEY,
    slug       TEXT UNIQUE NOT NULL,
    title      TEXT NOT NULL,
    content    TEXT NOT NULL,
    status     TEXT NOT NULL DEFAULT 'draft',
    template   TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- 媒体文件
CREATE TABLE IF NOT EXISTS media (
    id            TEXT PRIMARY KEY,
    filename      TEXT NOT NULL,
    original_name TEXT NOT NULL,
    mime_type     TEXT NOT NULL,
    size_bytes    INTEGER NOT NULL,
    width         INTEGER,
    height        INTEGER,
    url           TEXT NOT NULL,
    thumb_url     TEXT,
    uploaded_at   TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_media_uploaded_at ON media(uploaded_at DESC);

-- 主题配置
CREATE TABLE IF NOT EXISTS theme_config (
    theme_name TEXT PRIMARY KEY,
    config     TEXT NOT NULL DEFAULT '{}'
);

-- 插件/主题 KV 存储
CREATE TABLE IF NOT EXISTS plugin_store (
    plugin_name TEXT NOT NULL,
    key         TEXT NOT NULL,
    value       TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    PRIMARY KEY (plugin_name, key)
);

-- 撤销的 JWT（即时登出）
CREATE TABLE IF NOT EXISTS revoked_tokens (
    jti        TEXT PRIMARY KEY,
    expires_at TEXT NOT NULL
);

-- 构建历史
CREATE TABLE IF NOT EXISTS build_history (
    id          TEXT PRIMARY KEY,
    trigger     TEXT NOT NULL,
    status      TEXT NOT NULL,
    total_pages INTEGER,
    rebuilt     INTEGER,
    cached      INTEGER,
    duration_ms INTEGER,
    error       TEXT,
    started_at  TEXT NOT NULL,
    finished_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_build_history_started ON build_history(started_at DESC);
