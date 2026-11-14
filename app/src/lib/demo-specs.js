// The demo box's answer to /specs: the processor of a second-hand office mini PC, and the
// pretend computer's disks added up (see demoComputer in demo.js).

import { demoComputer } from './demo.js';
import { summarizeDrives } from './spec-card.js';

const DEMO_PROCESSOR = 'Intel Core i5-6500T';

export function demoSpecs() {
  const computer = demoComputer();
  return { processor: DEMO_PROCESSOR, drives: summarizeDrives(computer.drives, computer.smartctl) };
}
