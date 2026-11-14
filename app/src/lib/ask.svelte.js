// Ask: a yes-or-no question inside Castle, in place of the browser's confirm(). The
// browser's box cannot be answered with a controller, and it looks like nothing else here.

class Asking {
  /** The open question: { question, yes, no, danger, answer }, or null. */
  current = $state(null);
}

export const asking = new Asking();

/**
 * Asks a question and waits for the answer. A second question waits for the first.
 * @param {string} question
 * @param {{ yes?: string, no?: string, danger?: boolean }} [options] the button words; danger marks the yes red
 * @returns {Promise<boolean>} true for yes
 */
export function ask(question, { yes = 'Yes', no = 'Cancel', danger = false } = {}) {
  return new Promise((resolve) => {
    const open = () => {
      asking.current = {
        question,
        yes,
        no,
        danger,
        answer(value) {
          asking.current = null;
          resolve(value);
          queue.shift()?.();
        },
      };
    };
    if (asking.current) queue.push(open);
    else open();
  });
}

const queue = [];
