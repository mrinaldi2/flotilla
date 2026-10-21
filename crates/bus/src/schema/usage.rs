//! Usage and budgets (0.24.0, `usage_v1`): the tokens each bot used, read
//! from its runtime's transcript, the owner's price table and each
//! project's weekly token budget.

/// `usage_day`: one row per bot, local day and model, with the tokens of
/// each kind and the turns. `project_id` is the bot's project when counted,
/// so a deleted bot's use stays with its project. `usage_seen`: what each
/// transcript entry (an assistant message, a prompt) already added, so a
/// transcript read again adds only what grew. `usage_cursor`: how far each
/// transcript file was read, and the prompts read but not yet given to a
/// reply's row. `usage_price`: USD per million tokens by model family, the
/// seeded rows marked estimates. `project.budget_weekly_tokens`: the
/// weekly budget, NULL for none; `budget_extended_until`: Extend today.
pub(super) const MIGRATION_USAGE: &str = r#"
CREATE TABLE IF NOT EXISTS usage_day (
    bot_id      TEXT NOT NULL,
    day         TEXT NOT NULL,
    model       TEXT NOT NULL DEFAULT '',
    project_id  TEXT NOT NULL,
    input       INTEGER NOT NULL DEFAULT 0,
    output      INTEGER NOT NULL DEFAULT 0,
    cache_read  INTEGER NOT NULL DEFAULT 0,
    cache_write INTEGER NOT NULL DEFAULT 0,
    turns       INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (bot_id, day, model)
);
CREATE INDEX IF NOT EXISTS usage_day_project ON usage_day(project_id, day);
CREATE INDEX IF NOT EXISTS usage_day_day ON usage_day(day);
CREATE TABLE IF NOT EXISTS usage_seen (
    bot_id      TEXT NOT NULL,
    key         TEXT NOT NULL,
    day         TEXT NOT NULL,
    input       INTEGER NOT NULL DEFAULT 0,
    output      INTEGER NOT NULL DEFAULT 0,
    cache_read  INTEGER NOT NULL DEFAULT 0,
    cache_write INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (bot_id, key)
);
CREATE TABLE IF NOT EXISTS usage_cursor (
    bot_id        TEXT NOT NULL,
    path          TEXT NOT NULL,
    offset        INTEGER NOT NULL DEFAULT 0,
    pending_turns INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (bot_id, path)
);
CREATE TABLE IF NOT EXISTS usage_price (
    model       TEXT PRIMARY KEY,
    input       REAL NOT NULL,
    output      REAL NOT NULL,
    cache_read  REAL NOT NULL,
    cache_write REAL NOT NULL,
    estimate    INTEGER NOT NULL DEFAULT 0,
    updated_at  TEXT NOT NULL
);
INSERT OR IGNORE INTO usage_price (model, input, output, cache_read, cache_write, estimate, updated_at) VALUES
    ('fable',  10.0, 50.0, 0.25, 12.5,  1, '2026-10-21T00:00:00Z'),
    ('mythos', 10.0, 50.0, 0.25, 12.5,  1, '2026-10-21T00:00:00Z'),
    ('opus',    4.0, 20.0, 0.20,  5.0,  1, '2026-10-21T00:00:00Z'),
    ('sonnet',  2.0, 10.0, 0.20,  2.5,  1, '2026-10-21T00:00:00Z'),
    ('haiku',   0.1,  0.5, 0.01,  0.125, 1, '2026-10-21T00:00:00Z'),
    ('gpt',    1.25, 10.0, 0.125, 0.0,  1, '2026-10-21T00:00:00Z'),
    ('*',       4.0, 20.0, 0.20,  5.0,  1, '2026-10-21T00:00:00Z');
ALTER TABLE project ADD COLUMN budget_weekly_tokens INTEGER;
ALTER TABLE project ADD COLUMN budget_extended_until TEXT;
"#;
