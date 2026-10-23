// Pictures of the web build through the registered server, in headless
// Chrome on the real GPU: Playground Kit's js/shot.mjs with this game's
// defaults (the status global __mech, the build in dist/web/, the pictures
// in shots/). For example:
//   node tools/shot.mjs --query "mode=impact" --name impact --wait 8
import { run } from '../../../crates/engine/js/shot.mjs';

process.env.GAME_GLOBAL ||= '__mech';
run({ defaults: { dist: 'web', out: 'shots' } }).catch((e) => {
  console.error(e);
  process.exit(1);
});
