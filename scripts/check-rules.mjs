/**
 * Wrapper de compatibilidad. `npm run rules:check` y el job `rules-check` de CI
 * siguen invocando este archivo; ahora delega en el CLI (`cli.mjs rules`).
 */
import { main } from './cli.mjs';

process.exitCode = await main(['rules']);
