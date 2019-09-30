<template>
    <b-container class="p-0">
        <p>
            <b-progress class="w-100" v-if="value != max" :value="value" :max="max" show animated></b-progress>
        </p>
    </b-container>
</template>

<script>
import { sleep } from '../utils.js'

export default {
    name: 'ProgressBar',
    data: () => ({
        value: 100,
        max: 100,
        animationInProgress: false,
    }),
    methods: {
        updateProgressPercent(percent) {
            this.value = percent;
        },
        cancelAnimation() {
            this.value = this.max;
        },
        startProgressBarHalfLife(millis) {
            const animation = progress => 1 - Math.pow(2, -progress);
            this.startProgressBar(millis, animation);
        },
        startProgressBarTimer(millis) {
            const animation = progress => progress;
            this.startProgressBar(millis, animation);
        },
        async startProgressBar(duration, animation) {
            this.cancelAnimation();
            while (this.animationInProgress) {
                await sleep(50);
            }
            
            this.value = 0;
            this.animationInProgress = true;
            const start = Date.now();
            const draw = () => {
                if (this.value >= this.max) {
                    this.animationInProgress = false;
                    return;
                }

                let progress = (Date.now() - start) / duration;
                progress = animation(progress);
                progress = Math.min(1.0, progress);
                this.value = Math.round(this.max * progress);
                window.requestAnimationFrame(draw);
            };
            window.requestAnimationFrame(draw);
        },
    },
}
</script>
