<template>
    <div class="w-100">
        <Alert class="w-100 mt-0 mb-1" ref="alert"></Alert>
        <ProgressBar ref="progress_bar"></ProgressBar>
    </div>
</template>

<script>
import { sleep } from '../utils.js'

import Alert from './Alert.vue'
import ProgressBar from './ProgressBar.vue'
import timeConstants from '../timeConstants'

const components = {
    Alert,
    ProgressBar,
};

export default {
    name: 'AlertWithProgressBar',
    props: ['shower'],
    async created() {
        let wait = null;
        for await (const progress of this.shower.generator.gencopy()) {
            if (progress.displayMessage) {
                this.$refs.alert.display(progress.displayMessage);
                wait = progress.displayMessage.countdown;
            }

            if (progress.startProgressBar) {
                switch (progress.startProgressBar.variant) {
                    case 'half':
                        this.$refs.progress_bar.startProgressBarHalfLife(progress.startProgressBar.duration);
                        break;
                    default: 
                        throw new Error('switch reached default state');
                }
            }

            if (progress.stopProgressBar) {
                this.$refs.progress_bar.cancelAnimation();
            }
        }

        wait && await sleep(wait * 1000);

        let idx = this.store.pendingTransactionGenerators.indexOf(this.shower);
        if (idx != -1) {
            this.store.pendingTransactionGenerators = this.store.pendingTransactionGenerators.slice(idx, 1);
        }
    },
    components,
}
</script>
