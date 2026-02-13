import fs from 'node:fs';
import path from 'node:path';

const TEXT_EXTENSIONS = new Set([
  '.ts', '.tsx', '.js', '.jsx', '.json', '.md',
  '.css', '.html', '.yml', '.yaml', '.mjs', '.cjs',
]);

// Files with no extension that are text
const TEXT_NAMES = new Set([
  '.gitignore', '.dockerignore', '.editorconfig',
  '.env', '.env.example', '.nvmrc', 'Dockerfile',
]);

function isTextFile(filename) {
  if (TEXT_NAMES.has(filename)) return true;
  const ext = path.extname(filename).toLowerCase();
  return TEXT_EXTENSIONS.has(ext);
}

export function copyTemplate(templateDir, targetDir, replacements) {
  const entries = fs.readdirSync(templateDir, { withFileTypes: true });

  for (const entry of entries) {
    const srcPath = path.join(templateDir, entry.name);
    const destPath = path.join(targetDir, entry.name);

    if (entry.isDirectory()) {
      fs.mkdirSync(destPath, { recursive: true });
      copyTemplate(srcPath, destPath, replacements);
    } else if (isTextFile(entry.name)) {
      let content = fs.readFileSync(srcPath, 'utf8');
      for (const [placeholder, value] of Object.entries(replacements)) {
        content = content.replaceAll(placeholder, value);
      }
      fs.writeFileSync(destPath, content, 'utf8');
    } else {
      fs.copyFileSync(srcPath, destPath);
    }
  }
}
