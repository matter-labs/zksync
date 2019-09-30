import { sleep } from './utils.js'

export class GeneratorMultiplier {
    constructor(gen) {
        (async () => {
            do {
                this.original = await gen.next();
            } while (this.original.done == false);
        })();
    }
    async * getGenerator() {
        while (this.original === undefined) {
            await sleep(1000);
        }

        let copy = null;
        while (true) {
            if (copy !== this.original) {
                copy = this.original;
                if (copy.value === undefined) return;
                yield copy.value;
            }
            await sleep(1000);
        }
    }
}
