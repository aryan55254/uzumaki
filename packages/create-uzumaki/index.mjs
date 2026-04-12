#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const TEMPLATE_DIR = path.join(__dirname, 'template');

const BOLD = '\u001B[1m';
const RESET = '\u001B[0m';
const BLUE = '\u001B[38;5;75m';
const GREEN = '\u001B[32m';
const RED = '\u001B[31m';
const DIM = '\u001B[2m';

function log(msg) {
  console.log(msg);
}

function copyDirSync(src, dest) {
  fs.mkdirSync(dest, { recursive: true });
  for (const entry of fs.readdirSync(src, { withFileTypes: true })) {
    const srcPath = path.join(src, entry.name);
    const destPath = path.join(dest, entry.name);
    if (entry.isDirectory()) {
      copyDirSync(srcPath, destPath);
    } else {
      fs.copyFileSync(srcPath, destPath);
    }
  }
}

function replaceVars(filePath, vars) {
  let content = fs.readFileSync(filePath, 'utf8');
  for (const [key, value] of Object.entries(vars)) {
    content = content.replaceAll(`{{${key}}}`, value);
  }
  fs.writeFileSync(filePath, content, 'utf8');
}

function main() {
  const args = process.argv.slice(2);
  const targetArg = args[0];

  if (targetArg === '--help' || targetArg === '-h') {
    log(
      `\n${BOLD}${BLUE}create-uzumaki${RESET} — Scaffold a new Uzumaki desktop app\n`,
    );
    log(`  Usage: pnpm create uzumaki ${DIM}[directory]${RESET}\n`);
    process.exit(0);
  }

  const projectDir = path.resolve(process.cwd(), targetArg ?? '.');
  const projectName = path.basename(projectDir);

  if (fs.existsSync(projectDir) && fs.readdirSync(projectDir).length > 0) {
    log(
      `${RED}error:${RESET} directory ${DIM}${projectDir}${RESET} is not empty`,
    );
    process.exit(1);
  }

  log(
    `\n${BOLD}${BLUE}Uzumaki${RESET} Creating project ${BOLD}${projectName}${RESET}...\n`,
  );

  copyDirSync(TEMPLATE_DIR, projectDir);

  const filesToTemplate = [
    path.join(projectDir, 'package.json'),
    path.join(projectDir, 'uzumaki.config.json'),
    path.join(projectDir, 'src', 'index.tsx'),
  ];

  for (const file of filesToTemplate) {
    if (fs.existsSync(file)) {
      replaceVars(file, { PROJECT_NAME: projectName });
    }
  }

  log(`  ${GREEN}created${RESET} ${projectName}/package.json`);
  log(`  ${GREEN}created${RESET} ${projectName}/tsconfig.json`);
  log(`  ${GREEN}created${RESET} ${projectName}/uzumaki.config.json`);
  log(`  ${GREEN}created${RESET} ${projectName}/src/index.tsx`);

  log(`\n${BOLD}Next steps:${RESET}\n`);
  if (targetArg) {
    log(`  cd ${projectName}`);
  }
  log('  pnpm install');
  log('  pnpm dev');
  log('');
}

main();
