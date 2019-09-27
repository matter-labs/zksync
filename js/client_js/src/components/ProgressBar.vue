<template>
    <b-container class="p-0">
        <p>
            <b-progress class="w-100" v-if="value != max" :value="value" :max="max" show show-progress animated></b-progress>
        </p>
    </b-container>
</template>

<script>
const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms));

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
            const self = this;
            const start = Date.now();
            const draw = () => {
                if (self.value >= self.max) {
                    self.animationInProgress = false;
                    return;
                }

                let progress = (Date.now() - start) / duration;
                console.log('progress', progress);
                progress = animation(progress);
                progress = Math.min(1.0, progress);
                self.value = Math.round(self.max * progress);
                window.requestAnimationFrame(draw);
            };
            window.requestAnimationFrame(draw);
        },
    },
}
</script>
