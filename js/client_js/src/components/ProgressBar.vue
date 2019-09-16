<template>
    <b-container>
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
    }),
    methods: {
        updateProgressPercent(percent) {
            this.value = percent;
        },
        async startProgressBarTimer(millis) {
            this.value = 0;
            const num_stops = 100;
            const chunk_time = millis / num_stops;
            for (let i = 0; this.value < 100 && i < num_stops; i++) {
                this.value = i;
                await sleep(chunk_time);
            }
            this.value = 100;
        },
    },
}
</script>
