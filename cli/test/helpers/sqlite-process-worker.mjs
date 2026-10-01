import { readFileSync } from 'node:fs';
import Database from 'better-sqlite3';

const operations = JSON.parse(readFileSync(0, 'utf8'));
const db = new Database(process.argv[2], { fileMustExist: true, timeout: 5000 });
try {
  const results = db.transaction(() =>
    operations.map(({ mode, sql, params = [] }) => {
      if (mode === 'get') return db.prepare(sql).get(...params);
      if (mode === 'all') return db.prepare(sql).all(...params);
      if (mode === 'exec') {
        db.exec(sql);
        return null;
      }
      throw new Error(`Unsupported SQLite inspection mode: ${mode}`);
    }),
  )();
  process.stdout.write(JSON.stringify(results));
} finally {
  db.close();
}
