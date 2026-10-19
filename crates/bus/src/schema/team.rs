//! The team on demand (0.23.0): whether a bot runs all the time or only
//! while there is work for it.

/// A bot's `presence`: `always_on`, or `on_demand` (asleep when idle, woken
/// by work). New bots are on demand. On upgrade every existing bot becomes on
/// demand except each project's lead, which is always on. Whichever bot
/// becomes a lead later is made always on by the trigger, so every path that
/// names a lead keeps it running.
pub(super) const MIGRATION_BOT_PRESENCE: &str = r#"
ALTER TABLE bot ADD COLUMN presence TEXT NOT NULL DEFAULT 'on_demand'
    CHECK(presence IN ('always_on', 'on_demand'));
UPDATE bot SET presence = 'always_on'
    WHERE id IN (SELECT lead_bot_id FROM project WHERE lead_bot_id IS NOT NULL);
CREATE TRIGGER IF NOT EXISTS project_lead_always_on AFTER UPDATE OF lead_bot_id ON project
    WHEN NEW.lead_bot_id IS NOT NULL
BEGIN
    UPDATE bot SET presence = 'always_on' WHERE id = NEW.lead_bot_id;
END;
"#;
