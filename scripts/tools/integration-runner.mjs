import { readdir } from 'node:fs/promises';
import { join } from 'node:path';
import { spawn } from 'node:child_process';

const integrationDir = join(process.cwd(), 'scripts', 'integration');
const files = (await readdir(integrationDir))
  .filter((file) => file.endsWith('.test.mjs'))
  .sort();

for (const file of files) {
  const exitCode = await new Promise((resolve) => {
    const child = spawn(
      process.execPath,
      ['--test', join(integrationDir, file)],
      { env: process.env, stdio: 'inherit' },
    );
    child.on('error', () => resolve(1));
    child.on('exit', (code, signal) => resolve(code ?? (signal ? 1 : 0)));
  });

  if (exitCode !== 0) process.exit(exitCode);
}
