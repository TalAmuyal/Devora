-- Frozen schema from #299, before the Hub can create spaces with history.
CREATE TABLE space (
 id INTEGER PRIMARY KEY CHECK(id = 0),
 name TEXT NOT NULL,
 role TEXT NOT NULL,
 form TEXT NOT NULL,
 instructions TEXT NOT NULL,
 session_id TEXT NOT NULL,
 has_conversation INTEGER NOT NULL CHECK(has_conversation IN (0,1)),
 ready INTEGER NOT NULL CHECK(ready IN (0,1))
);
PRAGMA user_version = 1;
