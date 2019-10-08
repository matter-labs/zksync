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
        for await (const progress of this.shower.generator.getGenerator()) {
            if (progress.message.includes(`waiting for creating new block`)) {
                this.$refs.progress_bar.startProgressBarHalfLife(10000);
            }
            if (progress.message.includes(`started proving block`)) {
                this.$refs.progress_bar.startProgressBarHalfLife(10000);
            }

            let countdown = timeConstants.countdown;
            if (progress.message.includes(`got proved!`)) {
                console.log(`got proved received`);
                this.$refs.progress_bar.cancelAnimation();
                countdown = 10;
            }

            this.$refs.alert.display({
                message: progress.message,
                variant: progress.error ? 'danger' : 'success',
                countdown,
            });
        }
        
        {            
            let elem = document.getElementById(this.shower.id);
            console.log('elem removing', elem);
            elem.parentElement.removeChild(elem);
        }
    },
    components,
}
</script>
