//! Platforms become labels (0.19.0 K8).
//!
//! Before 0.19 an item had platforms from a fixed set (desktop, ios, daemon,
//! infra). They become its labels in the `platform` group:
//!
//! - each platform is added to the item's labels as `platform:<name>`, after
//!   the labels it had, once; `item.platforms` is then emptied (the column
//!   stays, unread, since SQLite drops columns only by rebuilding `item`);
//! - a board gets the label group `platform: desktop, ios, daemon, infra`
//!   when its items carry a platform, or when it is release-gated and its
//!   project's templates require one;
//! - the templates' Definition of Ready fields that read platforms read the
//!   labels instead, with the same meaning: `platforms` becomes
//!   `label:platform`, and `spec_link_for_ui_or_daemon` becomes a spec link
//!   for items labelled desktop, ios or daemon;
//! - the machines that must verify a platform are keyed by its label
//!   (`platform:desktop`).
//!
//! Every value is kept. Safe to run again (see `db::migrate_columns`): an
//! item whose platforms are empty, a board with label groups, a template
//! with neither old field and a key that is already a label are left alone.

pub(super) const MIGRATION_PLATFORM_LABELS: &str = r#"
ALTER TABLE board ADD COLUMN label_groups TEXT NOT NULL DEFAULT '[]';

UPDATE item SET
    labels = (
        SELECT json_group_array(v ORDER BY g, k) FROM (
            SELECT value AS v, 0 AS g, key AS k FROM json_each(item.labels)
            UNION ALL
            SELECT 'platform:' || p.value, 1, p.key FROM json_each(item.platforms) p
            WHERE 'platform:' || p.value NOT IN (SELECT value FROM json_each(item.labels))
              AND p.key = (SELECT min(q.key) FROM json_each(item.platforms) q
                           WHERE q.value = p.value)
        )
    ),
    platforms = '[]'
WHERE platforms != '[]' AND json_valid(platforms) AND json_valid(labels);

UPDATE template SET body = json_set(body, '$.ready', json((
    SELECT json_group_array(
        CASE value
            WHEN 'platforms' THEN 'label:platform'
            WHEN 'spec_link_for_ui_or_daemon'
                THEN 'spec_link:platform:desktop,platform:ios,platform:daemon'
            ELSE value
        END ORDER BY key)
    FROM json_each(template.body, '$.ready')
)))
WHERE json_valid(body) AND EXISTS (
    SELECT 1 FROM json_each(template.body, '$.ready')
    WHERE value IN ('platforms', 'spec_link_for_ui_or_daemon')
);

UPDATE board_settings SET required_machines = (
    SELECT json_group_object(
        CASE WHEN instr(key, ':') > 0 THEN key ELSE 'platform:' || key END, json(value))
    FROM json_each(board_settings.required_machines)
)
WHERE json_valid(required_machines) AND EXISTS (
    SELECT 1 FROM json_each(board_settings.required_machines) WHERE instr(key, ':') = 0
);

UPDATE board SET label_groups = json_array(json_object(
    'name', 'platform', 'values', json_array('desktop', 'ios', 'daemon', 'infra')))
WHERE label_groups = '[]' AND (
    EXISTS (
        SELECT 1 FROM item, json_each(item.labels) l
        WHERE item.board_id = board.id AND l.value LIKE 'platform:%'
    )
    OR (flow = 'release_gated' AND EXISTS (
        SELECT 1 FROM template t, json_each(t.body, '$.ready') r
        WHERE t.project_id = board.project_id AND r.value = 'label:platform'
    ))
);
"#;
