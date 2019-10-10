import { sleep } from "./utils";

class GeneratorMultiplierSomewhatAbstract {
	constructor(start) {
        this.promises = [];
        start();
    }

	/**
	 * gencopy() can be called by everyone who wants to know how progress() is doing now.
	 */
	gencopy() {
        // I should do a talk at some conference about this.
        const self = this;
        let firstTime = true;
        return {
            [Symbol.asyncIterator]() {
                return {
                    next() {
                        if (firstTime) {
                            firstTime = false;
                            if (self.current !== undefined) {
                                return Promise.resolve(self.current);
                            }
                        }
                        return new Promise(resolve => {
                            // the resolvement will happen in start()
                            self.promises.push(resolve);
                        });
                    }
                };
            }
        };
    }
}

export class GeneratorMultiplierMinTime extends GeneratorMultiplierSomewhatAbstract {
	constructor(gen, minWaitTime = 1000) {
        const start = async () => {
            let prevEnd = Date.now();
            do {
                this.current = await gen.next();
                
                let diff = Date.now() - prevEnd;
                if (diff < minWaitTime) {
                    await sleep(minWaitTime - diff);
                }
                prevEnd = Date.now();

                this.promises.forEach(resolve => resolve(this.current));
                this.promises.length = 0;
            } while (this.current.done == false);
        };
        super(start);
    }
}

export class GeneratorMultiplier extends GeneratorMultiplierSomewhatAbstract {
	constructor(gen) {
        const start = async () => {
            do {
                this.current = await gen.next();
                this.promises.forEach(resolve => resolve(this.current));
                this.promises.length = 0;
            } while (this.current.done == false);
        };
        super(start);
    }
}
