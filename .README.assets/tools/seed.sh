#!/bin/sh
# Seed the demo history database. Runs inside the capture namespace, so the
# paths recorded are /home/dev/... (the clean paths shown in the screenshots).
set -eu
FS=/home/dev/repo/fist/fs
W=/home/dev/work
DB=$XDG_STATE_HOME/fist/record.db

"$FS" :tool bump "$W/harborlight" >/dev/null

bump() { "$FS" :tool bump --count "$1" "$2" >/dev/null; }
bump 14 "$W/harborlight"
bump 11 "$W/harborlight/src"
bump  4 "$W/harborlight/src/api"
bump  9 "$W/harborlight/docs"
bump  7 "$W/harborlight/logs"
bump  6 "$W/vault"
bump  8 "$W/vault/projects"
bump  3 "$W/vault/meetings"
bump  5 "$W/media/photos"
bump  2 "$W/data"
bump  1 "$W/archive"
bump  2 "$W/inbox"

# The history shot shows a wall of distinct filetypes. Bump this list in
# reverse so architecture.md has the newest atime and is selected at startup.
history_files='
'"$W/harborlight/docs/architecture.md"'
'"$W/harborlight/src/api/auth.rs"'
'"$W/harborlight/src/api/routes.rs"'
'"$W/harborlight/src/db.rs"'
'"$W/harborlight/src/cli/main.go"'
'"$W/harborlight/src/ui/App.tsx"'
'"$W/harborlight/scripts/deploy.py"'
'"$W/harborlight/Cargo.toml"'
'"$W/harborlight/config/app.yaml"'
'"$W/data/users.json"'
'"$W/data/metrics.csv"'
'"$W/harborlight/logs/error.log"'
'"$W/harborlight/migrations/0001_init.sql"'
'"$W/harborlight/scripts/release.sh"'
'"$W/harborlight/.env"'
'"$W/harborlight/Cargo.lock"'
'"$W/harborlight/docs/whitepaper.pdf"'
'"$W/media/audio/ambient.mp3"'
'"$W/media/video/ingest.mp4"'
'"$W/media/photos/iceland.jpg"'
'
printf '%s\n' "$history_files" | sed '/^$/d' | tac | while IFS= read -r f; do
  "$FS" :tool bump "$f" >/dev/null
done

# `path` and `cmd` are decoded as OsString bytes; insert them as BLOB or the
# whole apps pane query fails.
NOW=$(date +%s)
sqlite3 "$DB" <<SQL
DELETE FROM apps WHERE typeof(path) = 'text' OR typeof(cmd) = 'text';
INSERT OR REPLACE INTO apps (name,path,alias,cmd,atime,count,score) VALUES
 ('Visual Studio Code',CAST('/usr/share/applications/com.microsoft.VSCode.desktop' AS BLOB),'',CAST('code' AS BLOB),$((NOW-30)),6,3.2),
 ('Firefox',CAST('/usr/share/applications/firefox.desktop' AS BLOB),'',CAST('firefox' AS BLOB),$((NOW-200)),3,1.4),
 ('mpv',CAST('/usr/share/applications/mpv.desktop' AS BLOB),'',CAST('mpv' AS BLOB),$((NOW-400)),1,0.6),
 ('Obsidian',CAST('/usr/share/applications/obsidian.desktop' AS BLOB),'',CAST('obsidian' AS BLOB),$((NOW-100)),2,1.1),
 ('Alacritty',CAST('/usr/share/applications/Alacritty.desktop' AS BLOB),'',CAST('alacritty' AS BLOB),$((NOW-50)),5,2.0),
 ('Chromium',CAST('/usr/share/applications/chromium.desktop' AS BLOB),'',CAST('chromium' AS BLOB),$((NOW-300)),4,1.6);
DELETE FROM stashes;
INSERT INTO stashes (name,stash,tail,add_time) VALUES
 ('bookmark',CAST('$W/vault' AS BLOB),'vault',1),
 ('bookmark',CAST('$W/harborlight/docs' AS BLOB),'docs',2),
 ('bookmark',CAST('$W/media/photos' AS BLOB),'photos',3),
 ('bookmark',CAST('$W/data' AS BLOB),'data',4);
-- Keep the history views on the demo world only.
DELETE FROM dirs WHERE path IN (CAST('/home/dev' AS BLOB), CAST('/home/dev/.cache' AS BLOB), CAST('/home/dev/.config' AS BLOB));
SQL
echo "seeded $DB"
