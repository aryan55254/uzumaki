#!/usr/bin/env bun

import { fileURLToPath } from 'bun';
import fs from 'node:fs';
import path from 'node:path';

function color(text: string, value: string) {
  const start = Bun.color(value, 'ansi') ?? '';
  const reset = start ? '\x1b[0m' : '';
  return `${start}${text}${reset}`;
}

function bold(text: string) {
  return `\x1b[1m${text}\x1b[22m`;
}

function dim(text: string) {
  return `\x1b[2m${text}\x1b[22m`;
}

const args = process.argv.slice(2);

function help() {
  const commands = [
    {
      name: 'run',
      desc: 'Run a JS/TS file in uzumaki runtime',
      args: './index.tsx [...args]',
    },
    {
      name: 'pack',
      desc: 'Package a bundled app into a standalone executable',
      args: '[dist]',
    },
  ];

  const nameWidth = Math.max(...commands.map((cmd) => cmd.name.length));
  const argsWidth = Math.max(...commands.map((cmd) => cmd.args?.length ?? 0));

  const commandLines = commands
    .map((cmd) => {
      const name = bold(color(cmd.name.padEnd(nameWidth), '#60a5fa'));
      const argText = dim((cmd.args ?? '').padEnd(argsWidth));
      return `  ${name}  ${argText}  ${cmd.desc}`;
    })
    .join('\n');

  console.log(
    [
      `${bold(color('Uzumaki', '#60a5fa'))} Desktop UI Framework`,
      '',
      `${bold('Usage:')} uzumaki <command> ${dim('[...flags] [...args]')}`,
      '',
      `${bold('Commands:')}`,
      commandLines,
    ].join('\n'),
  );
}

const BIN_FOLDER = path.resolve(
  path.dirname(fileURLToPath(new URL(import.meta.url))),
  '../bin',
);

function getBinaryName() {
  switch (process.platform) {
    case 'win32':
      return 'uzumaki.exe';
    default:
      return 'uzumaki';
  }
}

function resolveBinaryPath() {
  return path.join(BIN_FOLDER, getBinaryName());
}

async function run(entryPoint: string, extraArgs: string[] = []) {
  const binaryPath = resolveBinaryPath();

  if (!fs.existsSync(binaryPath)) {
    console.error(
      [
        color('error:', '#ef4444'),
        `native binary not found at ${dim(binaryPath)}`,
      ].join(' '),
    );
    return 1;
  }

  const child = Bun.spawn([binaryPath, entryPoint, ...extraArgs], {
    stdin: 'inherit',
    stdout: 'inherit',
    stderr: 'inherit',
  });

  return await child.exited;
}

async function main() {
  if (!args.length) {
    help();
    return 0;
  }

  const cmd = args[0]!;

  switch (cmd) {
    case 'run': {
      const entryPoint = args[1];
      if (!entryPoint) {
        console.error(`${color('error:', '#ef4444')} entry point not provided`);
        console.error(`usage: ${dim('uzumaki run <entrypoint> [...args]')}`);
        return 1;
      }
      return await run(entryPoint, args.slice(2));
    }

    case 'pack': {
      console.error(dim('pack command is not implemented yet'));
      return 1;
    }

    default: {
      return await run(cmd, args.slice(1));
    }
  }
}

const exitCode = await main();
if (exitCode !== 0) {
  process.exit(exitCode);
}
