CREATE TABLE servers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    working_dir TEXT,
    start_script TEXT,
    stop_script TEXT,
    restart_script TEXT,
    log_type TEXT,
    log_path TEXT,
    log_unit TEXT,
    status_type TEXT,
    status_value TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

INSERT INTO servers (
    id,
    name,
    description,
    working_dir,
    start_script,
    stop_script,
    restart_script,
    log_type,
    log_path,
    log_unit,
    status_type,
    status_value,
    created_at,
    updated_at
)
SELECT
    id,
    name,
    NULL,
    install_dir,
    NULL,
    NULL,
    NULL,
    NULL,
    NULL,
    NULL,
    'manual',
    NULL,
    created_at,
    updated_at
FROM game_servers;

DROP TABLE game_servers;
